//! Диагностика z-порядка: какие окна перекрывают оверлей (в т.ч. меню Пуск).
//!
//! Чистая логика (классификация shell-окон, формат отчёта) тестируется на
//! Linux через cargo test; обход цепочки окон через Win32 живёт под
//! cfg(windows). Включается переменной окружения KEYPRESS_DEBUG=1: тогда
//! рядом с keypress.toml ведётся keypress-zdebug.log со списком окон выше
//! оверлея (класс + заголовок), записывается только при изменении списка.

/// Информация об одном окне из цепочки z-порядка.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinInfo {
    /// Класс окна (GetClassNameW).
    pub class: String,
    /// Заголовок окна (GetWindowTextW), может быть пустым.
    pub title: String,
    /// Пересекается ли окно с прямоугольником оверлея.
    pub intersects: bool,
}

impl WinInfo {
    pub fn new(class: &str, title: &str, intersects: bool) -> Self {
        Self {
            class: class.to_string(),
            title: title.to_string(),
            intersects,
        }
    }
}

/// Классы окон оболочки Windows: Пуск, Поиск, центр уведомлений, панель задач.
///
/// Меню Пуск и Поиск на Win10/11 — это CoreWindow процессов
/// StartMenuExperienceHost/SearchHost; остальное — хосты XAML-полуостровов
/// оболочки Win11 и классические таскбары.
const SHELL_CLASSES: &[&str] = &[
    "Windows.UI.Core.CoreWindow",          // Пуск/Поиск/центр уведомлений
    "Shell_TrayWnd",                       // панель задач
    "Shell_SecondaryTrayWnd",              // панель задач второго монитора
    "XamlExplorerHostIslandWindow",        // Win11: XAML-хост оболочки (Task View и др.)
    "TopLevelWindowForOverflowXamlIsland", // Win11: всплывающие панели оболочки
    "Shell_ChromeWindow",                  // хост всплывающих панелей оболочки
];

/// Является ли класс окном оболочки (сравнение без учёта регистра).
pub fn is_shell_class(class: &str) -> bool {
    SHELL_CLASSES.iter().any(|c| c.eq_ignore_ascii_case(class))
}

/// Первое окно оболочки среди окон выше оверлея, пересекающееся с ним.
pub fn first_shell_above(above: &[WinInfo]) -> Option<&WinInfo> {
    above
        .iter()
        .find(|w| w.intersects && is_shell_class(&w.class))
}

/// Максимум окон при обходе цепочки z-порядка (защита от аномалий).
pub const MAX_WALK: usize = 128;

/// Обрезает строку до max символов, добавляя «…» при усечении.
fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let cut: String = s.chars().take(max).collect();
        format!("{cut}…")
    }
}

/// Текстовый отчёт для лога: найдено ли окно оверлея и что над ним.
pub fn format_report(uptime_secs: u64, overlay_found: bool, above: &[WinInfo]) -> String {
    let mut out = format!("[t+{uptime_secs}s] ");
    if !overlay_found {
        out.push_str("окно оверлея не найдено");
        return out;
    }
    if above.is_empty() {
        out.push_str("над оверлеем нет видимых окон");
        return out;
    }
    out.push_str(&format!("окон выше оверлея: {}", above.len()));
    for (i, w) in above.iter().enumerate() {
        let shell = if is_shell_class(&w.class) {
            " — SHELL (Пуск/Поиск/оболочка)"
        } else {
            ""
        };
        let cover = if w.intersects {
            " ПЕРЕКРЫВАЕТ"
        } else {
            ""
        };
        out.push_str(&format!(
            "\n  {}. [{}] \"{}\"{}{}",
            i + 1,
            w.class,
            clip(&w.title, 48),
            shell,
            cover
        ));
    }
    // Итог для быстрого чтения лога: перекрыто ли shell-окном (Пуск и т.п.).
    match first_shell_above(above) {
        Some(w) => out.push_str(&format!(
            "\nВЫВОД: оверлей перекрыт shell-окном [{}] \"{}\"",
            w.class, w.title
        )),
        None => out.push_str("\nВЫВОД: shell-окон над оверлеем нет"),
    }
    out
}

/// Вызывать раз в кадр из оверлея. Активна только при KEYPRESS_DEBUG=1:
/// раз в 500 мс собирает окна выше оверлея и при изменении списка дописывает
/// отчёт в keypress-zdebug.log рядом с keypress.toml.
pub fn debug_tick() {
    imp::tick();
}

#[cfg(windows)]
mod imp {
    use super::{format_report, WinInfo, MAX_WALK};
    use std::sync::{Mutex, OnceLock};
    use std::time::{Duration, Instant};
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED};
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetTopWindow, GetWindow, GetWindowRect, GetWindowTextW, IsWindowVisible,
        GW_HWNDNEXT,
    };

    const THROTTLE: Duration = Duration::from_millis(500);
    const LOG_NAME: &str = "keypress-zdebug.log";

    struct State {
        start: Instant,
        last_run: Instant,
        last_sig: Option<String>,
    }

    static STATE: Mutex<Option<State>> = Mutex::new(None);
    static ENABLED: OnceLock<bool> = OnceLock::new();

    pub fn tick() {
        // Флаг читаем один раз за жизнь процесса — переменная не меняется на ходу.
        let enabled =
            *ENABLED.get_or_init(|| std::env::var("KEYPRESS_DEBUG").as_deref() == Ok("1"));
        if !enabled {
            return;
        }
        let Ok(mut guard) = STATE.lock() else { return };
        let st = guard.get_or_insert_with(|| State {
            start: Instant::now(),
            last_run: Instant::now() - THROTTLE,
            last_sig: None,
        });
        if st.last_run.elapsed() < THROTTLE {
            return;
        }
        st.last_run = Instant::now();

        let hwnd = crate::winutil::overlay_hwnd();
        let (found, above) = if hwnd == 0 {
            (false, Vec::new())
        } else {
            (true, collect_above(hwnd))
        };

        // Пишем только при изменении списка перекрывающих окон — лог не растёт
        // от простоя, каждая запись = реальное изменение z-порядка.
        let sig = if found {
            format!(
                "{:?}",
                above
                    .iter()
                    .filter(|w| w.intersects)
                    .map(|w| w.class.as_str())
                    .collect::<Vec<_>>()
            )
        } else {
            String::from("no-overlay")
        };
        if Some(&sig) == st.last_sig.as_ref() {
            return;
        }
        st.last_sig = Some(sig);

        let report = format_report(st.start.elapsed().as_secs(), found, &above);
        write_log(&report);
    }

    fn log_path() -> Option<std::path::PathBuf> {
        crate::config::config_path()
            .parent()
            .map(|d| d.join(LOG_NAME))
    }

    fn write_log(text: &str) {
        use std::io::Write;
        if let Some(p) = log_path() {
            if let Ok(mut f) = std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
            {
                let _ = f.write_all(text.as_bytes());
                let _ = f.write_all(b"\n");
            }
        }
    }

    /// Окна выше оверлея в z-порядке (видимые, не cloaked), от верхнего вниз.
    /// Возвращает собранное до встречи оверлея; пусто — если оверлей сверху.
    fn collect_above(overlay: isize) -> Vec<WinInfo> {
        let mut or = RECT {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        };
        if unsafe { GetWindowRect(overlay, &mut or) } == 0 {
            return Vec::new();
        }
        let mut out = Vec::new();
        let mut cur = unsafe { GetTopWindow(0) };
        for _ in 0..MAX_WALK {
            if cur == 0 {
                break;
            }
            if cur == overlay {
                // Дошли до оверлея — всё собранное выше него.
                return out;
            }
            if unsafe { IsWindowVisible(cur) } != 0 && !is_cloaked(cur) {
                let mut r = RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                };
                if unsafe { GetWindowRect(cur, &mut r) } != 0 {
                    // Пересечение прямоугольников (частичное касание не считаем).
                    let intersects = r.left < or.right
                        && r.right > or.left
                        && r.top < or.bottom
                        && r.bottom > or.top;
                    out.push(WinInfo::new(
                        &class_name(cur),
                        &window_text(cur),
                        intersects,
                    ));
                }
            }
            cur = unsafe { GetWindow(cur, GW_HWNDNEXT) };
        }
        out
    }

    /// UWP-окна оболочки «прячутся» через cloak, а не IsWindowVisible —
    /// без этой проверки в списке висели бы невидимые CoreWindow.
    fn is_cloaked(hwnd: isize) -> bool {
        let mut cloaked: u32 = 0;
        let hr = unsafe {
            DwmGetWindowAttribute(
                hwnd,
                // В windows-sys 0.52 параметр — u32, а константа объявлена i32.
                DWMWA_CLOAKED as u32,
                &mut cloaked as *mut u32 as *mut core::ffi::c_void,
                std::mem::size_of::<u32>() as u32,
            )
        };
        hr == 0 && cloaked != 0
    }

    fn class_name(hwnd: isize) -> String {
        let mut buf = [0u16; 256];
        let n = unsafe { GetClassNameW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        if n <= 0 {
            String::new()
        } else {
            String::from_utf16_lossy(&buf[..n as usize])
        }
    }

    fn window_text(hwnd: isize) -> String {
        let mut buf = [0u16; 256];
        let n = unsafe { GetWindowTextW(hwnd, buf.as_mut_ptr(), buf.len() as i32) };
        if n <= 0 {
            String::new()
        } else {
            String::from_utf16_lossy(&buf[..n as usize])
        }
    }
}

#[cfg(not(windows))]
mod imp {
    pub fn tick() {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shell_class_matched_case_insensitively() {
        assert!(is_shell_class("Windows.UI.Core.CoreWindow"));
        assert!(is_shell_class("windows.ui.core.corewindow"));
        assert!(is_shell_class("SHELL_TRAYWND"));
    }

    #[test]
    fn ordinary_classes_are_not_shell() {
        assert!(!is_shell_class("Chrome_WidgetWin_1"));
        assert!(!is_shell_class(""));
        assert!(!is_shell_class("NotAShellClass"));
    }

    #[test]
    fn first_shell_above_skips_ordinary_windows() {
        let above = vec![
            WinInfo::new("Chrome_WidgetWin_1", "Браузер", true),
            WinInfo::new("Windows.UI.Core.CoreWindow", "Пуск", true),
            WinInfo::new("Shell_TrayWnd", "", true),
        ];
        let first = first_shell_above(&above).unwrap();
        assert_eq!(first.class, "Windows.UI.Core.CoreWindow");
        assert_eq!(first.title, "Пуск");
    }

    #[test]
    fn nonintersecting_shell_is_ignored() {
        let above = vec![WinInfo::new("Windows.UI.Core.CoreWindow", "Пуск", false)];
        assert!(first_shell_above(&above).is_none());
        assert!(first_shell_above(&[]).is_none());
    }

    #[test]
    fn report_marks_shell_and_cover() {
        let above = vec![
            WinInfo::new("Windows.UI.Core.CoreWindow", "Search", true),
            WinInfo::new("Chrome_WidgetWin_1", "Telegram", false),
        ];
        let r = format_report(7, true, &above);
        assert!(r.contains("[t+7s]"), "нет метки времени: {r}");
        assert!(r.contains("Windows.UI.Core.CoreWindow"));
        assert!(r.contains("SHELL"), "нет пометки shell: {r}");
        assert!(r.contains("ПЕРЕКРЫВАЕТ"), "нет пометки пересечения: {r}");
        assert!(r.contains("окон выше оверлея: 2"));
        assert!(r.contains("ВЫВОД: оверлей перекрыт shell-окном [Windows.UI.Core.CoreWindow]"));
    }

    #[test]
    fn report_without_shell_says_so() {
        let above = vec![WinInfo::new("Chrome_WidgetWin_1", "Браузер", true)];
        let r = format_report(3, true, &above);
        assert!(r.contains("ВЫВОД: shell-окон над оверлеем нет"), "{r}");
    }

    #[test]
    fn report_handles_no_overlay_and_empty() {
        let r = format_report(1, false, &[]);
        assert!(r.contains("не найдено"), "{r}");
        let r = format_report(2, true, &[]);
        assert!(r.contains("нет видимых окон"), "{r}");
    }
}
