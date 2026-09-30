//! Глобальные low-level хуки клавиатуры и мыши + горячие клавиши.
//! Всё вводо-зависимое собрано в отдельном потоке, в UI уходят готовые события.
//!
//! Чистая логика (агрегация комбо, VK-имена, фильтр инжекций, кнопки/скролл)
//! живёт в keys.rs / input.rs и покрыта unit-тестами. Здесь только Win API-клей.

#[cfg(windows)]
mod imp {
    use super::super::input::{self, UiEvent};
    use super::super::keys::KeyAggregator;
    use crossbeam_channel::Sender;
    use std::cell::RefCell;
    use std::time::Instant;

    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

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
                            ag.key_down(vk, Instant::now())
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
                let ev = if let Some(button) = input::click_button(msg) {
                    Some(UiEvent::Click { x, y, button })
                } else if msg == input::WM_MOUSEWHEEL {
                    input::wheel_up(info.mouseData).map(|up| UiEvent::Scroll { x, y, up })
                } else {
                    None
                };
                if let Some(e) = ev {
                    send(e);
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
}

pub use imp::spawn_hooks;
