use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// Форма индикатора клика мыши.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClickShape {
    Circle,
    Ring,
    Square,
}

/// Конфигурация. Хранится в keyviz-lite.toml рядом с exe.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Общий масштаб виджета и эффектов (0.5 .. 3.0)
    pub scale: f32,
    /// Позиция виджета клавиш: доля ширины экрана (0..1)
    pub pos_x: f32,
    /// Позиция виджета клавиш: доля высоты экрана (0..1)
    pub pos_y: f32,
    pub show_keys: bool,
    pub show_clicks: bool,
    pub show_scroll: bool,
    /// Сколько секунд висит комбо на экране
    pub key_duration: f32,
    /// Скругление углов клавиш (0..40)
    pub key_radius: f32,
    /// Форма индикатора клика
    pub click_shape: ClickShape,
    /// Цвета в RGBA, компоненты 0..1 (прямой альфа-канал)
    pub key_bg: [f32; 4],
    pub key_text: [f32; 4],
    pub click_color: [f32; 4],
    pub scroll_color: [f32; 4],
}

impl Default for Config {
    fn default() -> Self {
        Self {
            scale: 1.0,
            pos_x: 0.5,
            pos_y: 0.92,
            show_keys: true,
            show_clicks: true,
            show_scroll: true,
            key_duration: 1.2,
            key_radius: 10.0,
            click_shape: ClickShape::Ring,
            key_bg: [0.08, 0.08, 0.10, 0.85],
            key_text: [1.00, 1.00, 1.00, 1.00],
            click_color: [1.00, 0.45, 0.20, 0.90],
            scroll_color: [0.30, 0.70, 1.00, 0.90],
        }
    }
}

/// Путь к конфигу: рядом с exe (чтобы оверлей и настройки видели один файл
/// независимо от рабочей папки запуска).
pub fn config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("keyviz-lite.toml");
        }
    }
    PathBuf::from("keyviz-lite.toml")
}

pub fn load() -> Config {
    let path = config_path();
    match std::fs::read_to_string(&path) {
        Ok(s) => toml::from_str(&s).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save(cfg: &Config) {
    if let Ok(s) = toml::to_string_pretty(cfg) {
        let _ = std::fs::write(config_path(), s);
    }
}
