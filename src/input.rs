/// События, которые поток хуков отправляет окну-оверлею.
#[cfg_attr(not(windows), allow(dead_code))] // используется Windows-кодом
#[derive(Debug, Clone)]
pub enum UiEvent {
    /// Показать комбо. Текст уже собран, например "Ctrl + C".
    Keys { text: String },
    /// Клик мышью. Координаты — физические пиксели экрана.
    /// button: 0 — левая, 1 — правая, 2 — средняя (задел на разные цвета/формы).
    Click {
        x: f32,
        y: f32,
        #[allow(dead_code)]
        button: u8,
    },
    /// Прокрутка колесиком. Координаты — физические пиксели экрана.
    Scroll { x: f32, y: f32, up: bool },
    /// Ctrl+Alt+K — пауза / продолжить показ.
    TogglePause,
    /// Ctrl+Alt+Q — выход.
    Quit,
}

// ---- Чистые хелперы для хуков (тестируемые, без windows-sys) ----

/// Сообщения мыши, интересующие оверлей (значения Win32).
pub const WM_LBUTTONDOWN: u32 = 0x0201;
pub const WM_RBUTTONDOWN: u32 = 0x0204;
pub const WM_MBUTTONDOWN: u32 = 0x0207;
pub const WM_MOUSEWHEEL: u32 = 0x020A;

/// Флаг KBDLLHOOKSTRUCT: событие сгенерировано программно (SendInput и т.п.).
pub const LLKHF_INJECTED: u32 = 0x10;
/// Флаг MSLLHOOKSTRUCT: событие сгенерировано программно.
pub const LLMHF_INJECTED: u32 = 0x01;

/// Событие клавиатуры инжектировано? Такое игнорируем (иначе зацикливание).
pub fn kb_injected(flags: u32) -> bool {
    flags & LLKHF_INJECTED != 0
}

/// Событие мыши инжектировано?
pub fn mouse_injected(flags: u32) -> bool {
    flags & LLMHF_INJECTED != 0
}

/// Сообщение мыши -> номер кнопки (0 левая, 1 правая, 2 средняя).
pub fn click_button(msg: u32) -> Option<u8> {
    match msg {
        WM_LBUTTONDOWN => Some(0),
        WM_RBUTTONDOWN => Some(1),
        WM_MBUTTONDOWN => Some(2),
        _ => None,
    }
}

/// Сырое колесико (mouseData) -> Some(up?) или None, если дельта нулевая.
pub fn wheel_up(mouse_data: u32) -> Option<bool> {
    let delta = (mouse_data >> 16) as u16 as i16;
    match delta {
        0 => None,
        d => Some(d > 0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keyboard_injection_flag() {
        assert!(!kb_injected(0));
        assert!(kb_injected(LLKHF_INJECTED));
        assert!(kb_injected(LLKHF_INJECTED | 0x20)); // вместе с другими флагами
        assert!(!kb_injected(0x20)); // посторонний флаг — не инжекция
    }

    #[test]
    fn mouse_injection_flag() {
        assert!(!mouse_injected(0));
        assert!(mouse_injected(LLMHF_INJECTED));
        assert!(!mouse_injected(0x02));
    }

    #[test]
    fn button_mapping() {
        assert_eq!(click_button(WM_LBUTTONDOWN), Some(0));
        assert_eq!(click_button(WM_RBUTTONDOWN), Some(1));
        assert_eq!(click_button(WM_MBUTTONDOWN), Some(2));
        assert_eq!(click_button(WM_MOUSEWHEEL), None); // колесо — не кнопка
        assert_eq!(click_button(0x0202), None); // WM_LBUTTONUP — не кнопка
        assert_eq!(click_button(0), None);
    }

    #[test]
    fn wheel_direction() {
        assert_eq!(wheel_up(0x0001_0000), Some(true)); // +1 в старшем слове
        assert_eq!(wheel_up(0xFFFF_0000), Some(false)); // -1
        assert_eq!(wheel_up(0xFF88_0000), Some(false)); // -120 (стандартный тик)
        assert_eq!(wheel_up(0x0078_0000), Some(true)); // +120
        assert_eq!(wheel_up(0), None); // нулевая дельта — игнорируем
    }
}
