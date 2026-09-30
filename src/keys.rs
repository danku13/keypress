//! Чистая логика клавиш: таблицы VK-имен и агрегатор комбо.
//! Без зависимости от Windows — тестируется на любой платформе.
//!
//! Правила агрегации (как в hooks.rs, вынесено для TDD):
//! - модификаторы трекаются в held-set;
//! - «Ctrl + C» собирается при нажатии обычной клавиши;
//! - одиночный модификатор показывается через 280 мс (tick),
//!   если за это время не пришла обычная клавиша;
//! - повторный keydown той же клавиши (авто-повтор) игнорируется.

/// Через сколько мс одиночный модификатор считается «нажатым сам по себе».
pub const COMBO_TIMEOUT_MS: u64 = 280;

pub const LETTERS: [&str; 26] = [
    "A", "B", "C", "D", "E", "F", "G", "H", "I", "J", "K", "L", "M", "N", "O", "P", "Q", "R", "S",
    "T", "U", "V", "W", "X", "Y", "Z",
];
pub const DIGITS: [&str; 10] = ["0", "1", "2", "3", "4", "5", "6", "7", "8", "9"];
pub const FKEYS: [&str; 24] = [
    "F1", "F2", "F3", "F4", "F5", "F6", "F7", "F8", "F9", "F10", "F11", "F12", "F13", "F14", "F15",
    "F16", "F17", "F18", "F19", "F20", "F21", "F22", "F23", "F24",
];
pub const NUMS: [&str; 10] = [
    "Num 0", "Num 1", "Num 2", "Num 3", "Num 4", "Num 5", "Num 6", "Num 7", "Num 8", "Num 9",
];

/// VK-код модификатора -> человекочитаемое имя.
pub fn mod_name(vk: u32) -> Option<&'static str> {
    match vk {
        0x10 | 0xA0 | 0xA1 => Some("Shift"),
        0x11 | 0xA2 | 0xA3 => Some("Ctrl"),
        0x12 | 0xA4 | 0xA5 => Some("Alt"),
        0x5B | 0x5C => Some("Win"),
        _ => None,
    }
}

/// VK-код обычной клавиши -> человекочитаемое имя (английская раскладка).
pub fn vk_name(vk: u32) -> Option<&'static str> {
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

/// Агрегатор нажатий в комбо-строки вида "Ctrl + Shift + C".
#[derive(Debug, Default)]
pub struct KeyAggregator {
    held: std::collections::HashSet<u32>,
    pending: Option<(&'static str, std::time::Instant)>,
}

impl KeyAggregator {
    pub fn new() -> Self {
        Self::default()
    }

    /// Все зажатые сейчас модификаторы в каноническом порядке.
    fn current_mods(&self) -> Vec<&'static str> {
        let held = |vks: &[u32]| vks.iter().any(|k| self.held.contains(k));
        let mut v = Vec::new();
        if held(&[0xA2, 0xA3, 0x11]) {
            v.push("Ctrl");
        }
        if held(&[0xA0, 0xA1, 0x10]) {
            v.push("Shift");
        }
        if held(&[0xA4, 0xA5, 0x12]) {
            v.push("Alt");
        }
        if held(&[0x5B, 0x5C]) {
            v.push("Win");
        }
        v
    }

    /// Нажата клавиша (WM_KEYDOWN/WM_SYSKEYDOWN). Возвращает готовый текст комбо.
    pub fn key_down(&mut self, vk: u32, now: std::time::Instant) -> Option<String> {
        if let Some(mname) = mod_name(vk) {
            if !self.held.contains(&vk) {
                self.held.insert(vk);
                self.pending = Some((mname, now));
            }
            return None;
        }
        if self.held.contains(&vk) {
            return None; // авто-повтор
        }
        self.held.insert(vk);
        // Комбо поглощает «одиночный» модификатор
        self.pending = None;
        vk_name(vk).map(|name| {
            let mut parts = self.current_mods();
            parts.push(name);
            parts.join(" + ")
        })
    }

    /// Отпущена клавиша (WM_KEYUP/WM_SYSKEYUP).
    pub fn key_up(&mut self, vk: u32) {
        self.held.remove(&vk);
    }

    /// Периодический тик (~60 мс): показывает одиночный модификатор по таймауту.
    /// Если зажато несколько модификаторов — показывает весь комбо.
    pub fn tick(&mut self, now: std::time::Instant) -> Option<String> {
        let (name, at) = self.pending?;
        if now.duration_since(at) < std::time::Duration::from_millis(COMBO_TIMEOUT_MS) {
            return None;
        }
        self.pending = None;
        // Модификатор всё ещё зажат — показываем все зажатые (Ctrl + Shift),
        // иначе — только тот, который успели тапнуть и отпустить.
        let text = if self.current_mods().contains(&name) {
            self.current_mods().join(" + ")
        } else {
            name.to_string()
        };
        Some(text)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn t0() -> Instant {
        Instant::now()
    }

    #[test]
    fn mod_names() {
        assert_eq!(mod_name(0x10), Some("Shift"));
        assert_eq!(mod_name(0xA0), Some("Shift"));
        assert_eq!(mod_name(0xA1), Some("Shift"));
        assert_eq!(mod_name(0x11), Some("Ctrl"));
        assert_eq!(mod_name(0xA2), Some("Ctrl"));
        assert_eq!(mod_name(0xA3), Some("Ctrl"));
        assert_eq!(mod_name(0x12), Some("Alt"));
        assert_eq!(mod_name(0xA4), Some("Alt"));
        assert_eq!(mod_name(0x5B), Some("Win"));
        assert_eq!(mod_name(0x5C), Some("Win"));
        assert_eq!(mod_name(0x42), None); // B — не модификатор
    }

    #[test]
    fn vk_names_tables() {
        assert_eq!(vk_name(0x41), Some("A"));
        assert_eq!(vk_name(0x5A), Some("Z"));
        assert_eq!(vk_name(0x30), Some("0"));
        assert_eq!(vk_name(0x39), Some("9"));
        assert_eq!(vk_name(0x70), Some("F1"));
        assert_eq!(vk_name(0x87), Some("F24"));
        assert_eq!(vk_name(0x65), Some("Num 5"));
        assert_eq!(vk_name(0x20), Some("Space"));
        assert_eq!(vk_name(0x0D), Some("Enter"));
        assert_eq!(vk_name(0x1B), Some("Esc"));
        assert_eq!(vk_name(0x25), Some("Left"));
        assert_eq!(vk_name(0x28), Some("Down"));
        assert_eq!(vk_name(0x2C), Some("Print Screen"));
        assert_eq!(vk_name(0xBA), Some(";"));
        assert_eq!(vk_name(0xC0), Some("`"));
        assert_eq!(vk_name(0xDE), Some("'"));
        assert_eq!(vk_name(0xFF), None); // неизвестный код
        assert_eq!(vk_name(0x6A), Some("Num *"));
    }

    #[test]
    fn plain_key_emits_name() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        assert_eq!(ag.key_down(0x41, now), Some("A".into()));
    }

    #[test]
    fn combo_ctrl_c() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        assert_eq!(ag.key_down(0xA2, now), None); // Ctrl down — пока ничего
        assert_eq!(ag.key_down(0x43, now), Some("Ctrl + C".into())); // C
    }

    #[test]
    fn combo_order_is_ctrl_shift_alt_win() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        ag.key_down(0xA0, now); // Shift
        ag.key_down(0xA2, now); // Ctrl
        ag.key_down(0xA4, now); // Alt
        assert_eq!(
            ag.key_down(0x56, now),
            Some("Ctrl + Shift + Alt + V".into())
        );
    }

    #[test]
    fn auto_repeat_ignored_for_plain_key() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        assert_eq!(ag.key_down(0x41, now), Some("A".into()));
        assert_eq!(ag.key_down(0x41, now), None); // авто-повтор
        assert_eq!(ag.key_down(0x41, now), None);
    }

    #[test]
    fn auto_repeat_ignored_for_modifier() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        ag.key_down(0xA2, now); // Ctrl down -> pending
        ag.key_down(0xA2, now); // повтор — pending не сбрасывается
                                // тик до таймаута — тишина
        assert_eq!(ag.tick(now + Duration::from_millis(100)), None);
        assert_eq!(
            ag.key_down(0x43, now + Duration::from_millis(100)),
            Some("Ctrl + C".into())
        );
    }

    #[test]
    fn key_reemit_after_release() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        assert_eq!(ag.key_down(0x41, now), Some("A".into()));
        ag.key_up(0x41);
        assert_eq!(ag.key_down(0x41, now), Some("A".into()));
    }

    #[test]
    fn modifier_released_then_plain_key() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        ag.key_down(0xA2, now); // Ctrl down
        ag.key_up(0xA2); // Ctrl up
        assert_eq!(ag.key_down(0x43, now), Some("C".into()));
    }

    #[test]
    fn lone_modifier_shown_after_timeout() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA0, t); // Shift down
        assert_eq!(
            ag.tick(t + Duration::from_millis(COMBO_TIMEOUT_MS - 1)),
            None
        );
        assert_eq!(
            ag.tick(t + Duration::from_millis(COMBO_TIMEOUT_MS)),
            Some("Shift".into())
        );
        // показывается один раз
        assert_eq!(
            ag.tick(t + Duration::from_millis(COMBO_TIMEOUT_MS + 100)),
            None
        );
    }

    #[test]
    fn lone_modifier_absorbed_by_plain_key() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA0, t); // Shift down
        assert_eq!(ag.key_down(0x41, t), Some("Shift + A".into()));
        // после комбо таймер не должен показать одиночный модификатор
        assert_eq!(ag.tick(t + Duration::from_millis(1000)), None);
    }

    #[test]
    fn two_mods_alone_show_full_combo_on_timeout() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA2, t); // Ctrl
        ag.key_down(0xA0, t); // Shift — pending сместился, но Ctrl всё ещё зажат
        assert_eq!(
            ag.tick(t + Duration::from_millis(COMBO_TIMEOUT_MS)),
            Some("Ctrl + Shift".into())
        );
    }

    #[test]
    fn released_pending_modifier_still_shown_once() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA2, t); // Ctrl down
        ag.key_up(0xA2); // быстрый тап и отпускание до тика
        assert_eq!(
            ag.tick(t + Duration::from_millis(COMBO_TIMEOUT_MS)),
            Some("Ctrl".into())
        );
    }

    #[test]
    fn unknown_key_clears_pending_modifier() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA2, t); // Ctrl down
        assert_eq!(ag.key_down(0xFF, t), None); // безымянная клавиша
        assert_eq!(ag.tick(t + Duration::from_millis(1000)), None); // pending сброшен
    }

    #[test]
    fn win_key_combo() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0x5B, t); // Win
        assert_eq!(ag.key_down(0x44, t), Some("Win + D".into()));
    }
}
