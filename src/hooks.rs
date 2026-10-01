//! Глобальные low-level хуки клавиатуры и мыши + горячие клавиши.
//! Всё вводо-зависимое собрано в отдельном потоке, в UI уходят готовые события.
//!
//! Чистая логика (агрегация комбо, VK-имена, фильтр инжекций, кнопки/скролл)
//! живёт в keys.rs / input.rs и покрыта unit-тестами. Здесь только Win API-клей.

#[cfg(windows)]
mod imp {
    use super::super::input::{self, UiEvent};
    use super::super::keys::{display_name, vk_name, KeyAggregator};
    use crossbeam_channel::Sender;
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, Ordering};
    use std::time::Instant;

    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, GetKeyState, GetKeyboardLayout, RegisterHotKey, ToUnicodeEx, MOD_ALT,
        MOD_CONTROL, VK_CAPITAL, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    /// Показывать кириллицу активной раскладки (управляется из панели настроек).
    static SHOW_CYRILLIC: AtomicBool = AtomicBool::new(true);

    pub fn set_show_cyrillic(v: bool) {
        SHOW_CYRILLIC.store(v, Ordering::Relaxed);
    }

    thread_local! {
        static TX: RefCell<Option<Sender<UiEvent>>> = RefCell::new(None);
        static KB: RefCell<KeyAggregator> = RefCell::new(KeyAggregator::new());
    }

    fn send(ev: UiEvent) {
        TX.with(|t| {
            if let Some(tx) = t.borrow().as_ref() {
                let _ = tx.send(ev);
            }
        });
    }

    /// Символ, соответствующий клавише в АКТИВНОЙ раскладке (раскладку берём
    /// у окна переднего плана, состояние модификаторов — через GetAsyncKeyState).
    /// None — если символа нет (мёртвая клавиша, неизвестная раскладка и т.п.).
    unsafe fn unicode_char(vk: u32, scan: u32) -> Option<char> {
        let hwnd = GetForegroundWindow();
        let tid = if hwnd != 0 {
            GetWindowThreadProcessId(hwnd, std::ptr::null_mut())
        } else {
            0
        };
        let hkl = if tid != 0 {
            GetKeyboardLayout(tid)
        } else {
            GetKeyboardLayout(0)
        };

        // Состояние модификаторов для ToUnicodeEx (регистр букв и т.п.)
        let mut state = [0u8; 256];
        let down = |v: u16| (GetAsyncKeyState(v as i32) as u16) & 0x8000 != 0;
        if down(VK_SHIFT) {
            state[VK_SHIFT as usize] = 0x80;
        }
        if down(VK_CONTROL) {
            state[VK_CONTROL as usize] = 0x80;
        }
        if down(VK_MENU) {
            state[VK_MENU as usize] = 0x80;
        }
        if down(VK_LWIN) || down(VK_RWIN) {
            state[VK_LWIN as usize] = 0x80;
        }
        if GetKeyState(VK_CAPITAL as i32) & 1 != 0 {
            state[VK_CAPITAL as usize] = 1; // Caps Lock — не зажат, а включён
        }

        let mut buf = [0u16; 8];
        let n = ToUnicodeEx(
            vk,
            scan,
            state.as_ptr(),
            buf.as_mut_ptr(),
            buf.len() as i32,
            0,
            hkl,
        );
        if n == 1 {
            char::from_u32(buf[0] as u32)
        } else {
            None
        }
    }

    unsafe extern "system" fn kb_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if ncode >= 0 {
            let info = &*(lparam as *const KBDLLHOOKSTRUCT);
            if !input::kb_injected(info.flags) {
                let msg = wparam as u32;
                let vk = info.vkCode;
                let ev = KB.with(|s| {
                    let ag = &mut *s.borrow_mut();
                    match msg {
                        m if m == WM_KEYDOWN || m == WM_SYSKEYDOWN => {
                            // Символ активной раскладки (кириллица, если включено)
                            let uni = unsafe { unicode_char(vk, info.scanCode) };
                            let name = vk_name(vk).map(|base| {
                                display_name(base, uni, SHOW_CYRILLIC.load(Ordering::Relaxed))
                            });
                            ag.key_down_named(vk, name.as_deref(), Instant::now())
                        }
                        m if m == WM_KEYUP || m == WM_SYSKEYUP => {
                            ag.key_up(vk);
                            None
                        }
                        _ => None,
                    }
                });
                if let Some(text) = ev {
                    send(UiEvent::Keys { text });
                }
            }
        }
        // Никогда не глотаем ввод — только наблюдаем
        CallNextHookEx(0, ncode, wparam, lparam)
    }

    unsafe extern "system" fn mouse_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if ncode >= 0 {
            let info = &*(lparam as *const MSLLHOOKSTRUCT);
            if !input::mouse_injected(info.flags) {
                let msg = wparam as u32;
                let (x, y) = (info.pt.x as f32, info.pt.y as f32);
                if let Some(button) = input::click_button(msg) {
                    send(UiEvent::Click { x, y, button });
                    // Кнопка мыши появляется и в виджете клавиш: «ЛКМ»,
                    // «Ctrl + ЛКМ»… СКМ — это нажатие на колёсико.
                    let text = KB.with(|s| {
                        let ag = &mut *s.borrow_mut();
                        ag.mouse_down(button, Instant::now())
                    });
                    if let Some(text) = text {
                        send(UiEvent::Keys { text });
                    }
                } else if msg == input::WM_MOUSEWHEEL {
                    if let Some(up) = input::wheel_up(info.mouseData) {
                        send(UiEvent::Scroll { x, y, up });
                        // Прокрутка дублируется в виджет: «Колесо ↑/↓»
                        let text = KB.with(|s| {
                            let ag = &mut *s.borrow_mut();
                            ag.wheel(up, Instant::now())
                        });
                        if let Some(text) = text {
                            send(UiEvent::Keys { text });
                        }
                    }
                }
            }
        }
        CallNextHookEx(0, ncode, wparam, lparam)
    }

    pub fn spawn_hooks(tx: Sender<UiEvent>) {
        std::thread::Builder::new()
            .name("input-hooks".into())
            .spawn(move || unsafe { run(tx) })
            .expect("failed to spawn hook thread");
    }

    unsafe fn run(tx: Sender<UiEvent>) {
        TX.with(|t| *t.borrow_mut() = Some(tx));

        // Горячие клавиши регистрируются на поток цикла сообщений
        let _ = RegisterHotKey(0, 1, MOD_CONTROL | MOD_ALT, 0x4B); // Ctrl+Alt+K
        let _ = RegisterHotKey(0, 2, MOD_CONTROL | MOD_ALT, 0x51); // Ctrl+Alt+Q
        let _ = SetTimer(0, 1, 60, None); // тик для сброса pending-модификатора

        let kb = SetWindowsHookExW(WH_KEYBOARD_LL, Some(kb_proc), 0, 0);
        let ms = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), 0, 0);

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            if msg.message == WM_HOTKEY {
                if msg.wParam == 1 {
                    send(UiEvent::TogglePause);
                } else if msg.wParam == 2 {
                    send(UiEvent::Quit);
                }
            } else if msg.message == WM_TIMER {
                let ev = KB.with(|s| {
                    let ag = &mut *s.borrow_mut();
                    ag.tick(Instant::now())
                });
                if let Some(text) = ev {
                    send(UiEvent::Keys { text });
                }
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        if kb != 0 {
            UnhookWindowsHookEx(kb);
        }
        if ms != 0 {
            UnhookWindowsHookEx(ms);
        }
    }
}

#[cfg(not(windows))]
mod imp {
    use super::super::input::UiEvent;
    use crossbeam_channel::Sender;
    pub fn spawn_hooks(tx: Sender<UiEvent>) {
        let _ = tx;
    }

    pub fn set_show_cyrillic(_v: bool) {}
}

pub use imp::{set_show_cyrillic, spawn_hooks};
