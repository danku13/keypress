//! Глобальные low-level хуки клавиатуры и мыши + горячие клавиши.
//! Всё вводо-зависимое собрано в отдельном потоке, в UI уходит готовые события.

use crate::input::UiEvent;

#[cfg(windows)]
mod imp {
    use super::UiEvent;
    use crossbeam_channel::Sender;
    use std::cell::RefCell;
    use std::collections::HashSet;
    use std::time::{Duration, Instant};

    use windows_sys::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
    use windows_sys::Win32::UI::Input::KeyboardAndMouse::{RegisterHotKey, MOD_ALT, MOD_CONTROL};
    use windows_sys::Win32::UI::WindowsAndMessaging::*;

    const LETTERS: [&str; 26] = [
        "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R",
        "S", "T", "U", "V", "W", "X", "Y", "Z",
    ];
    const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
    const FKEYS: [&str; 24] = [
        "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14",
        "F15", "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
    ];
    const NUMS: [&str; 10] = [
        "Num 0", "Num 1", "Num 2", "Num 3", "Num 4", "Num 5", "Num 6", "Num 7", "Num 8", "Num 9",
    ];

    fn mod_name(vk: u32) -> Option<&'static str> {
        match vk {
            0x10 | 0xA0 | 0xA1 => Some("Shift"),
            0x11 | 0xA2 | 0xA3 => Some("Ctrl"),
            0x12 | 0xA4 | 0xA5 => Some("Alt"),
            0x5B | 0x5C => Some("Win"),
            _ => None,
        }
    }

    fn vk_name(vk: u32) -> Option<&'static str> {
        match vk {
            0x30..=0x39 => Some(DIGITS[(vk - 0x30) as usize]),
            0x41..=0x5A => Some(LETTERS[(vk - 0x41) as usize]),
            0x70..=0x87 => Some(FKEYS[(vk - 0x70) as usize]),
            0x60..=0x69 => Some(NUMS[(vk - 0x60) as usize]),
            0x20 => Some("Space"),
            0x0D => Some("Enter"),
            0x09 => Some("Tab"),
            0x1B => Some("Esc"),
            0x08 => Some("Backspace"),
            0x2E => Some("Delete"),
            0x2D => Some("Insert"),
            0x24 => Some("Home"),
            0x23 => Some("End"),
            0x21 => Some("Page Up"),
            0x22 => Some("Page Down"),
            0x25 => Some("Left"),
            0x26 => Some("Up"),
            0x27 => Some("Right"),
            0x28 => Some("Down"),
            0x14 => Some("Caps Lock"),
            0x90 => Some("Num Lock"),
            0x91 => Some("Scroll Lock"),
            0x2C => Some("Print Screen"),
            0x6A => Some("Num *"),
            0x6B => Some("Num +"),
            0x6D => Some("Num -"),
            0x6E => Some("Num ."),
            0x6F => Some("Num /"),
            0xBA => Some(";"),
            0xBB => Some("="),
            0xBC => Some(","),
            0xBD => Some("-"),
            0xBE => Some("."),
            0xBF => Some("/"),
            0xC0 => Some("`"),
            0xDB => Some("["),
            0xDC => Some("\\"),
            0xDD => Some("]"),
            0xDE => Some("'"),
            _ => None,
        }
    }

    struct KbState {
        held: HashSet<u32>,
        /// Модификатор, нажатый «в одиночку»: покажем его, только если
        /// в течение 280 мс не пришла обычная клавиша (иначе это часть комбо).
        pending: Option<(&'static str, Instant)>,
    }

    impl KbState {
        fn new() -> Self {
            Self {
                held: HashSet::new(),
                pending: None,
            }
        }

        fn current_mods(&self) -> Vec<&'static str> {
            let mut v = Vec::new();
            if [0xA2u32, 0xA3, 0x11].iter().any(|k| self.held.contains(k)) {
                v.push("Ctrl");
            }
            if [0xA0u32, 0xA1, 0x10].iter().any(|k| self.held.contains(k)) {
                v.push("Shift");
            }
            if [0xA4u32, 0xA5, 0x12].iter().any(|k| self.held.contains(k)) {
                v.push("Alt");
            }
            if [0x5Bu32, 0x5C].iter().any(|k| self.held.contains(k)) {
                v.push("Win");
            }
            v
        }
    }

    thread_local! {
        static TX: RefCell<Option<Sender<UiEvent>>> = RefCell::new(None);
        static KB: RefCell<KbState> = RefCell::new(KbState::new());
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
            if (info.flags & LLKHF_INJECTED) == 0 {
                let msg = wparam as u32;
                let vk = info.vkCode;
                let mut ev: Option<UiEvent> = None;
                KB.with(|s| {
                    let st = &mut *s.borrow_mut();
                    match msg {
                        m if m == WM_KEYDOWN || m == WM_SYSKEYDOWN => {
                            if let Some(mname) = mod_name(vk) {
                                if !st.held.contains(&vk) {
                                    st.held.insert(vk);
                                    st.pending = Some((mname, Instant::now()));
                                }
                            } else if !st.held.contains(&vk) {
                                st.held.insert(vk);
                                if let Some(name) = vk_name(vk) {
                                    let mut parts = st.current_mods();
                                    parts.push(name);
                                    ev = Some(UiEvent::Keys {
                                        text: parts.join(" + "),
                                    });
                                }
                                // Комбо поглощает «одиночный» модификатор
                                st.pending = None;
                            }
                        }
                        m if m == WM_KEYUP || m == WM_SYSKEYUP => {
                            st.held.remove(&vk);
                        }
                        _ => {}
                    }
                });
                if let Some(e) = ev {
                    send(e);
                }
            }
        }
        // Никогда не глотаем ввод — только наблюдаем
        CallNextHookEx(0, ncode, wparam, lparam)
    }

    unsafe extern "system" fn mouse_proc(ncode: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
        if ncode >= 0 {
            let info = &*(lparam as *const MSLLHOOKSTRUCT);
            if (info.flags & LLMHF_INJECTED) == 0 {
                let msg = wparam as u32;
                let (x, y) = (info.pt.x as f32, info.pt.y as f32);
                let ev = match msg {
                    m if m == WM_LBUTTONDOWN => Some(UiEvent::Click { x, y, button: 0 }),
                    m if m == WM_RBUTTONDOWN => Some(UiEvent::Click { x, y, button: 1 }),
                    m if m == WM_MBUTTONDOWN => Some(UiEvent::Click { x, y, button: 2 }),
                    m if m == WM_MOUSEWHEEL => {
                        let d = (info.mouseData >> 16) as u16 as i16;
                        if d != 0 {
                            Some(UiEvent::Scroll { x, y, up: d > 0 })
                        } else {
                            None
                        }
                    }
                    _ => None,
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
                KB.with(|s| {
                    let st = &mut *s.borrow_mut();
                    if let Some((name, at)) = st.pending {
                        if at.elapsed() >= Duration::from_millis(280) {
                            let text = name.to_string();
                            send(UiEvent::Keys { text });
                            st.pending = None;
                        }
                    }
                });
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
    use super::UiEvent;
    use crossbeam_channel::Sender;
    pub fn spawn_hooks(tx: Sender<UiEvent>) {
        let _ = tx;
    }
}

pub use imp::spawn_hooks;
