//! Чистая логика клавиш: таблицы VK-имен и агрегатор комбо.
//! Без зависимости от Windows — тестируется на любой платформе.
//!
//! Правила агрегации (как в hooks.rs, вынесено для TDD):
//! - модификаторы трекаются в held-set;
//! - «Ctrl + C» собирается при нажатии обычной клавиши;
//! - одиночный модификатор показывается через 280 мс (tick),
//!   если за это время не пришла обычная клавиша;
//! - повторный keydown той же клавиши (авто-повтор) игнорируется;
//! - мышь/колесо возвращаются отдельными частями комбо (Part::Mouse /
//!   Part::Wheel) — виджет рисует их SVG-иконками, а не текстом.

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

/// Часть комбо: обычная клавиша (текст), кнопка мыши (0 левая / 1 правая /
/// 2 средняя) или прокрутка колеса (true — вверх).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Key(String),
    Mouse(u8),
    Wheel(bool),
}

/// Результат прокрутки колеса.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WheelEv {
    /// Новая прокрутка — показать комбо.
    New(Vec<Part>),
    /// Колесо продолжает крутиться в ту же сторону — продлить показ
    /// последней строки (не создавать новую).
    Extend,
}

impl Part {
    pub fn key(s: impl Into<String>) -> Self {
        Part::Key(s.into())
    }
}

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

// ---- Кириллица и мышь в виджете клавиш ----

/// Это кириллический символ (включая Ё/ё)?
pub fn is_cyrillic(c: char) -> bool {
    matches!(c, '\u{0400}'..='\u{04FF}')
}

/// Имя для показа: при включённой настройке и кириллическом символе активной
/// раскладки — сам символ (в верхнем регистре), иначе английское имя VK.
pub fn display_name(base: &str, unicode: Option<char>, show_cyrillic: bool) -> String {
    match unicode {
        Some(c) if show_cyrillic && is_cyrillic(c) => c.to_uppercase().to_string(),
        _ => base.to_string(),
    }
}

/// Кнопка мыши (0 — левая, 1 — правая, 2 — средняя) -> имя.
/// Средняя кнопка — это и есть нажатие на колёсико.
/// Текстовый фолбэк, когда SVG-иконка ещё не готова.
pub fn mouse_button_name(button: u8) -> &'static str {
    match button {
        0 => "ЛКМ",
        1 => "ПКМ",
        _ => "СКМ",
    }
}

/// Прокрутка колесика -> имя (текстовый фолбэк).
pub fn wheel_name(up: bool) -> &'static str {
    if up {
        "Колесо ↑"
    } else {
        "Колесо ↓"
    }
}

/// Текст одной части комбо (иконки мыши/колеса — их текстовыми именами).
pub fn part_text(p: &Part) -> String {
    match p {
        Part::Key(s) => s.clone(),
        Part::Mouse(b) => mouse_button_name(*b).to_string(),
        Part::Wheel(up) => wheel_name(*up).to_string(),
    }
}

/// Комбо одной строкой «Ctrl + ЛКМ» (классический режим виджета и фолбэк).
pub fn combo_text(parts: &[Part]) -> String {
    parts.iter().map(part_text).collect::<Vec<_>>().join(" + ")
}

/// Окно троттлинга прокрутки: повторная прокрутка в ту же сторону в пределах
/// этого времени не создаёт новую строку, а продлевает текущую (иначе сплошной
/// скролл зальёт стопку или, наоборот, строка исчезнет посреди прокрутки).
pub const WHEEL_THROTTLE_MS: u64 = 250;

/// Агрегатор нажатий в комбо из частей (клавиши + мышь + колесо).
#[derive(Debug, Default)]
pub struct KeyAggregator {
    held: std::collections::HashSet<u32>,
    pending: Option<(&'static str, std::time::Instant)>,
    last_wheel: Option<(bool, std::time::Instant)>,
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

    fn mods_as_parts(&self) -> Vec<Part> {
        self.current_mods().into_iter().map(Part::key).collect()
    }

    /// Нажата клавиша (WM_KEYDOWN/WM_SYSKEYDOWN). Возвращает готовое комбо.
    /// На Windows хук зовёт key_down_named (имя зависит от раскладки);
    /// этот вариант — прямое использование VK-таблицы.
    #[cfg_attr(not(test), allow(dead_code))]
    pub fn key_down(&mut self, vk: u32, now: std::time::Instant) -> Option<Vec<Part>> {
        self.key_down_named(vk, vk_name(vk), now)
    }

    /// Как key_down, но имя клавиши уже разрешено хуком (раскладка/кириллица).
    pub fn key_down_named(
        &mut self,
        vk: u32,
        name: Option<&str>,
        now: std::time::Instant,
    ) -> Option<Vec<Part>> {
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
        // Имя не передано хуком -> берём таблицу VK (английская раскладка)
        let name = name.or_else(|| vk_name(vk))?;
        let mut parts = self.mods_as_parts();
        parts.push(Part::key(name));
        Some(parts)
    }

    /// Отпущена клавиша (WM_KEYUP/WM_SYSKEYUP).
    pub fn key_up(&mut self, vk: u32) {
        self.held.remove(&vk);
    }

    /// Нажата кнопка мыши: ЛКМ/ПКМ/СКМ (СКМ = нажатие на колёсико).
    /// Комбинируется с зажатыми модификаторами: «Ctrl + ЛКМ».
    pub fn mouse_down(&mut self, button: u8, _now: std::time::Instant) -> Option<Vec<Part>> {
        // Кнопки не держим в held-наборе: авто-повтора у мыши нет,
        // и кнопка не выступает модификатором для последующих событий.
        self.pending = None;
        let mut parts = self.mods_as_parts();
        parts.push(Part::Mouse(button));
        Some(parts)
    }

    /// Прокрутка колесика: новое комбо (New) либо продление существующей
    /// строки (Extend), пока скролл продолжается в ту же сторону.
    pub fn wheel(&mut self, up: bool, now: std::time::Instant) -> Option<WheelEv> {
        if let Some((last_up, at)) = self.last_wheel {
            if last_up == up
                && now.duration_since(at) < std::time::Duration::from_millis(WHEEL_THROTTLE_MS)
            {
                self.last_wheel = Some((up, now));
                return Some(WheelEv::Extend);
            }
        }
        self.last_wheel = Some((up, now));
        self.pending = None;
        let mut parts = self.mods_as_parts();
        parts.push(Part::Wheel(up));
        Some(WheelEv::New(parts))
    }

    /// Периодический тик (~60 мс): показывает одиночный модификатор по таймауту.
    /// Если зажато несколько модификаторов — показывает весь комбо.
    pub fn tick(&mut self, now: std::time::Instant) -> Option<Vec<Part>> {
        let (name, at) = self.pending?;
        if now.duration_since(at) < std::time::Duration::from_millis(COMBO_TIMEOUT_MS) {
            return None;
        }
        self.pending = None;
        // Модификатор всё ещё зажат — показываем все зажатые (Ctrl + Shift),
        // иначе — только тот, который успели тапнуть и отпустить.
        if self.current_mods().contains(&name) {
            Some(self.mods_as_parts())
        } else {
            Some(vec![Part::key(name)])
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::{Duration, Instant};

    fn t0() -> Instant {
        Instant::now()
    }

    /// Комбо (Option<Vec<Part>>) -> текст для удобных ассертов.
    fn txt(r: Option<Vec<Part>>) -> String {
        r.map(|p| combo_text(&p)).unwrap_or_default()
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
    fn part_and_combo_text() {
        assert_eq!(part_text(&Part::key("Ctrl")), "Ctrl");
        assert_eq!(part_text(&Part::Mouse(0)), "ЛКМ");
        assert_eq!(part_text(&Part::Mouse(1)), "ПКМ");
        assert_eq!(part_text(&Part::Mouse(2)), "СКМ");
        assert_eq!(part_text(&Part::Wheel(true)), "Колесо ↑");
        assert_eq!(part_text(&Part::Wheel(false)), "Колесо ↓");
        assert_eq!(combo_text(&[Part::key("Ctrl"), Part::key("C")]), "Ctrl + C");
    }

    #[test]
    fn plain_key_emits_name() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        assert_eq!(txt(ag.key_down(0x41, now)), "A");
        // структура: ровно одна часть-клавиша
        assert_eq!(ag.key_down(0x41, now + Duration::from_secs(1)), None); // авто-повтор
    }

    #[test]
    fn combo_ctrl_c() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        assert_eq!(ag.key_down(0xA2, now), None); // Ctrl down — пока ничего
        assert_eq!(txt(ag.key_down(0x43, now)), "Ctrl + C");
        // и части именно структурные
        assert_eq!(
            ag.key_down_named(0x44, Some("D"), now + Duration::from_millis(500)),
            Some(vec![Part::key("Ctrl"), Part::key("D")])
        );
    }

    #[test]
    fn combo_order_is_ctrl_shift_alt_win() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        ag.key_down(0xA0, now); // Shift
        ag.key_down(0xA2, now); // Ctrl
        ag.key_down(0xA4, now); // Alt
        assert_eq!(txt(ag.key_down(0x56, now)), "Ctrl + Shift + Alt + V");
    }

    #[test]
    fn auto_repeat_ignored_for_plain_key() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        assert_eq!(txt(ag.key_down(0x41, now)), "A");
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
            txt(ag.key_down(0x43, now + Duration::from_millis(100))),
            "Ctrl + C"
        );
    }

    #[test]
    fn key_reemit_after_release() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        assert_eq!(txt(ag.key_down(0x41, now)), "A");
        ag.key_up(0x41);
        assert_eq!(txt(ag.key_down(0x41, now)), "A");
    }

    #[test]
    fn modifier_released_then_plain_key() {
        let mut ag = KeyAggregator::new();
        let now = t0();
        ag.key_down(0xA2, now); // Ctrl down
        ag.key_up(0xA2); // Ctrl up
        assert_eq!(txt(ag.key_down(0x43, now)), "C");
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
            txt(ag.tick(t + Duration::from_millis(COMBO_TIMEOUT_MS))),
            "Shift"
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
        assert_eq!(txt(ag.key_down(0x41, t)), "Shift + A");
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
            txt(ag.tick(t + Duration::from_millis(COMBO_TIMEOUT_MS))),
            "Ctrl + Shift"
        );
    }

    #[test]
    fn released_pending_modifier_still_shown_once() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA2, t); // Ctrl down
        ag.key_up(0xA2); // быстрый тап и отпускание до тика
        assert_eq!(
            txt(ag.tick(t + Duration::from_millis(COMBO_TIMEOUT_MS))),
            "Ctrl"
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
        assert_eq!(txt(ag.key_down(0x44, t)), "Win + D");
    }

    #[test]
    fn cyrillic_detection() {
        assert!(is_cyrillic('ф'));
        assert!(is_cyrillic('Ё'));
        assert!(is_cyrillic('Я'));
        assert!(!is_cyrillic('F'));
        assert!(!is_cyrillic('1'));
        assert!(!is_cyrillic(';'));
    }

    #[test]
    fn display_name_prefers_cyrillic_when_enabled() {
        assert_eq!(display_name("A", Some('ф'), true), "Ф");
        assert_eq!(display_name("A", Some('Ж'), true), "Ж");
        assert_eq!(display_name("A", Some('ё'), true), "Ё");
        assert_eq!(display_name("A", Some('ф'), false), "A"); // настройка выключена
        assert_eq!(display_name("A", None, true), "A"); // символ не определён
        assert_eq!(display_name(";", Some('a'), true), ";"); // латиница не подменяет имя
    }

    #[test]
    fn mouse_button_names() {
        assert_eq!(mouse_button_name(0), "ЛКМ");
        assert_eq!(mouse_button_name(1), "ПКМ");
        assert_eq!(mouse_button_name(2), "СКМ"); // СКМ = нажатие на колёсико
    }

    #[test]
    fn wheel_names() {
        assert_eq!(wheel_name(true), "Колесо ↑");
        assert_eq!(wheel_name(false), "Колесо ↓");
    }

    #[test]
    fn mouse_down_plain() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        assert_eq!(ag.mouse_down(0, t), Some(vec![Part::Mouse(0)]));
    }

    #[test]
    fn mouse_down_combo_with_ctrl() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA2, t); // Ctrl
        assert_eq!(txt(ag.mouse_down(0, t)), "Ctrl + ЛКМ");
        assert_eq!(
            ag.mouse_down(0, t),
            Some(vec![Part::key("Ctrl"), Part::Mouse(0)])
        );
    }

    #[test]
    fn mouse_down_absorbs_pending_modifier() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA0, t); // Shift (pending)
        assert_eq!(txt(ag.mouse_down(0, t)), "Shift + ЛКМ");
        // одиночный модификатор после этого больше не показывается
        assert_eq!(ag.tick(t + Duration::from_millis(400)), None);
    }

    #[test]
    fn mouse_down_multi_mods() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA2, t); // Ctrl
        ag.key_down(0xA0, t); // Shift
        assert_eq!(txt(ag.mouse_down(1, t)), "Ctrl + Shift + ПКМ");
    }

    #[test]
    fn wheel_new_and_extend_while_spinning() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        // первая прокрутка — новая строка
        assert_eq!(
            ag.wheel(false, t),
            Some(WheelEv::New(vec![Part::Wheel(false)]))
        );
        // крутится дальше в ту же сторону — продление, НЕ новая строка
        assert_eq!(
            ag.wheel(false, t + Duration::from_millis(200)),
            Some(WheelEv::Extend)
        );
        assert_eq!(
            ag.wheel(false, t + Duration::from_millis(400)),
            Some(WheelEv::Extend)
        );
        // долго крутилась и остановилась — снова новая строка
        assert_eq!(
            ag.wheel(false, t + Duration::from_millis(900)),
            Some(WheelEv::New(vec![Part::Wheel(false)]))
        );
    }

    #[test]
    fn wheel_opposite_direction_is_new() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.wheel(false, t);
        assert_eq!(
            ag.wheel(true, t + Duration::from_millis(50)),
            Some(WheelEv::New(vec![Part::Wheel(true)]))
        );
    }

    #[test]
    fn wheel_combo_with_ctrl() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA2, t); // Ctrl
        assert_eq!(
            ag.wheel(false, t),
            Some(WheelEv::New(vec![Part::key("Ctrl"), Part::Wheel(false)]))
        );
    }

    #[test]
    fn key_down_named_uses_layout_name() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        assert_eq!(
            ag.key_down_named(0x41, Some("Ф"), t),
            Some(vec![Part::key("Ф")])
        );
        // имя не определено -> берём таблицу VK
        assert_eq!(ag.key_down_named(0x42, None, t), Some(vec![Part::key("B")]));
        // авто-повтор по VK по-прежнему глушится
        assert_eq!(ag.key_down_named(0x41, Some("Ф"), t), None);
    }

    #[test]
    fn key_down_named_combo_with_modifier() {
        let mut ag = KeyAggregator::new();
        let t = t0();
        ag.key_down(0xA2, t); // Ctrl
        assert_eq!(
            ag.key_down_named(0x41, Some("Ф"), t),
            Some(vec![Part::key("Ctrl"), Part::key("Ф")])
        );
    }
}
