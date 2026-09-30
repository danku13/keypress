//! keyviz-lite — показывает нажатия клавиш, клики мыши и прокрутку на экране.
//!
//! Режимы запуска:
//!   keyviz-lite.exe              — оверлей
//!   keyviz-lite.exe --settings   — панель настроек

mod config;
mod hooks;
mod input;
mod overlay;
mod settings;
mod winutil;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--settings" || a == "settings") {
        settings::run();
    } else {
        overlay::run();
    }
}
