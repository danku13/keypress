//! keypress — показывает нажатия клавиш, клики мыши и прокрутку на экране.
//!
//! Режимы запуска:
//!   keypress.exe              — оверлей + панель настроек (если ещё не открыта)
//!   keypress.exe --settings   — только панель настроек
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

// Чистые кроссплатформенные модули (тестируются на Linux через cargo test)
mod config;
mod fx;
mod input;
mod keys;

// Windows-глацинирование: GUI, хуки и Win API живут только под Windows
#[cfg(windows)]
mod hooks;
#[cfg(windows)]
mod overlay;
#[cfg(windows)]
mod settings;
#[cfg(windows)]
mod winutil;

/// Что запускать.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LaunchMode {
    /// Оверлей (+ панель настроек, если ещё не открыта).
    Overlay,
    /// Только панель настроек.
    Settings,
}

/// Разбор аргументов командной строки (args[0] — путь к exe, игнорируется).
pub fn decide_mode(args: &[String]) -> LaunchMode {
    let flag_set = args
        .iter()
        .skip(1)
        .any(|a| a == "--settings" || a == "settings");
    if flag_set {
        LaunchMode::Settings
    } else {
        LaunchMode::Overlay
    }
}

#[cfg(windows)]
fn spawn_panel_if_needed() {
    if winutil::window_exists(winutil::SETTINGS_TITLE) {
        return;
    }
    let exe = std::env::current_exe().unwrap_or_default();
    let _ = std::process::Command::new(exe).arg("--settings").spawn();
}

#[cfg(windows)]
fn main() {
    let args: Vec<String> = std::env::args().collect();
    match decide_mode(&args) {
        LaunchMode::Settings => settings::run(),
        LaunchMode::Overlay => {
            if winutil::overlay_running() {
                // Оверлей уже работает — просто показать панель.
                spawn_panel_if_needed();
                return;
            }
            // Панель настроек видна сразу после запуска, оверлей рядом с ней.
            spawn_panel_if_needed();
            overlay::run();
        }
    }
}

#[cfg(not(windows))]
fn main() {
    println!("keypress: графический режим доступен только на Windows.");
    println!("Сборка: cargo build --release --target x86_64-pc-windows-gnu");
}

#[cfg(test)]
mod tests {
    use super::*;

    fn args(list: &[&str]) -> Vec<String> {
        list.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn no_args_launches_overlay() {
        assert_eq!(decide_mode(&args(&["keypress.exe"])), LaunchMode::Overlay);
    }

    #[test]
    fn settings_flag_opens_panel() {
        assert_eq!(
            decide_mode(&args(&["keypress.exe", "--settings"])),
            LaunchMode::Settings
        );
    }

    #[test]
    fn bare_word_settings_also_opens_panel() {
        assert_eq!(
            decide_mode(&args(&["keypress.exe", "settings"])),
            LaunchMode::Settings
        );
    }

    #[test]
    fn unknown_args_default_to_overlay() {
        assert_eq!(
            decide_mode(&args(&["keypress.exe", "--whatever", "x"])),
            LaunchMode::Overlay
        );
    }
}
