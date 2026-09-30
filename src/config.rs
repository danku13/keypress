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
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
#[cfg_attr(not(windows), allow(dead_code))] // используется Windows-кодом
pub fn config_path() -> PathBuf {
    if let Ok(exe) = std::env::current_exe() {
        if let Some(dir) = exe.parent() {
            return dir.join("keyviz-lite.toml");
        }
    }
    PathBuf::from("keyviz-lite.toml")
}

/// Ограничение значений разумными пределами (совпадает с лимитами слайдеров панели).
pub fn normalized(mut cfg: Config) -> Config {
    let clamp = |v: f32, lo: f32, hi: f32| v.clamp(lo, hi);
    cfg.scale = clamp(cfg.scale, 0.5, 3.0);
    cfg.pos_x = clamp(cfg.pos_x, 0.0, 1.0);
    cfg.pos_y = clamp(cfg.pos_y, 0.0, 1.0);
    cfg.key_duration = clamp(cfg.key_duration, 0.5, 5.0);
    cfg.key_radius = clamp(cfg.key_radius, 0.0, 40.0);
    for c in [
        &mut cfg.key_bg,
        &mut cfg.key_text,
        &mut cfg.click_color,
        &mut cfg.scroll_color,
    ] {
        for comp in c {
            *comp = clamp(*comp, 0.0, 1.0);
        }
    }
    cfg
}

pub fn load_from(path: &std::path::Path) -> Config {
    match std::fs::read_to_string(path) {
        // битый файл -> дефолт; недостающие поля serde(default) добирает из дефолта
        Ok(s) => normalized(toml::from_str(&s).unwrap_or_default()),
        Err(_) => Config::default(),
    }
}

pub fn save_to(path: &std::path::Path, cfg: &Config) -> std::io::Result<()> {
    let s = toml::to_string_pretty(cfg).map_err(std::io::Error::other)?;
    std::fs::write(path, s)
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn load() -> Config {
    load_from(&config_path())
}

#[cfg_attr(not(windows), allow(dead_code))]
pub fn save(cfg: &Config) {
    let _ = save_to(&config_path(), cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tmp_path(tag: &str) -> PathBuf {
        std::env::temp_dir().join(format!("keyviz-test-{}-{}.toml", tag, std::process::id()))
    }

    #[test]
    fn defaults_are_sane() {
        let c = Config::default();
        assert_eq!(c.scale, 1.0);
        assert_eq!(c.pos_x, 0.5);
        assert_eq!(c.pos_y, 0.92);
        assert!(c.show_keys && c.show_clicks && c.show_scroll);
        assert_eq!(c.key_duration, 1.2);
        assert_eq!(c.click_shape, ClickShape::Ring);
        assert_eq!(c.key_text[3], 1.0); // непрозрачный текст
        assert!(c.key_bg[3] < 1.0); // полупрозрачный фон
    }

    #[test]
    fn save_then_load_roundtrip() {
        let path = tmp_path("roundtrip");
        let mut c = Config::default();
        c.scale = 1.75;
        c.pos_x = 0.25;
        c.click_shape = ClickShape::Square;
        c.key_bg = [0.9, 0.1, 0.2, 0.5];
        save_to(&path, &c).expect("save failed");
        let loaded = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.scale, 1.75);
        assert_eq!(loaded.pos_x, 0.25);
        assert_eq!(loaded.click_shape, ClickShape::Square);
        assert_eq!(loaded.key_bg, [0.9, 0.1, 0.2, 0.5]);
    }

    #[test]
    fn missing_file_gives_defaults() {
        let path = tmp_path("missing");
        let _ = std::fs::remove_file(&path);
        assert_eq!(load_from(&path), Config::default());
    }

    #[test]
    fn corrupt_file_gives_defaults() {
        let path = tmp_path("corrupt");
        std::fs::write(&path, "это не toml {{{{").unwrap();
        assert_eq!(load_from(&path), Config::default());
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn partial_file_fills_defaults() {
        let path = tmp_path("partial");
        std::fs::write(&path, "scale = 2.0\n").unwrap();
        let c = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(c.scale, 2.0);
        assert_eq!(c.pos_x, Config::default().pos_x); // остальное — из дефолта
        assert_eq!(c.click_shape, Config::default().click_shape);
    }

    #[test]
    fn unknown_fields_ignored() {
        let path = tmp_path("unknown");
        std::fs::write(&path, "scale = 1.5\nfuture_option = 42\n").unwrap();
        let c = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(c.scale, 1.5);
    }

    #[test]
    fn normalize_clamps_ranges() {
        let mut c = Config::default();
        c.scale = 100.0;
        c.pos_x = 2.0;
        c.pos_y = -1.0;
        c.key_duration = 99.0;
        c.key_radius = 500.0;
        c.key_bg = [1.5, -0.2, 0.5, 2.0];
        let c = normalized(c);
        assert_eq!(c.scale, 3.0);
        assert_eq!(c.pos_x, 1.0);
        assert_eq!(c.pos_y, 0.0);
        assert_eq!(c.key_duration, 5.0);
        assert_eq!(c.key_radius, 40.0);
        assert_eq!(c.key_bg, [1.0, 0.0, 0.5, 1.0]);
    }

    #[test]
    fn normalize_clamps_bottom() {
        let mut c = Config::default();
        c.scale = 0.05;
        c.key_duration = 0.0;
        let c = normalized(c);
        assert_eq!(c.scale, 0.5);
        assert_eq!(c.key_duration, 0.5);
    }

    #[test]
    fn load_applies_normalize() {
        let path = tmp_path("normload");
        std::fs::write(&path, "scale = 77.0\n").unwrap();
        let c = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(c.scale, 3.0);
    }
}
