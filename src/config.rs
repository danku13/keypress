use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// Форма индикатора клика мыши.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ClickShape {
    Circle,
    Ring,
    Square,
}

/// Конфигурация. Хранится в keypress.toml рядом с exe.
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
    /// Показывать символы кириллицы, когда активна русская (или другая
    /// кириллическая) раскладка. Иначе — всегда английские имена VK.
    pub show_cyrillic: bool,
    /// Режим «кейкапы»: клавиши рисуются SVG-кейкапами (вид сверху) по одной
    /// на клавишу, через «+». По умолчанию ВЫКЛЮЧЕН — основной вариант
    /// (текст на фоновом бабле) не трогается.
    pub keycap_style: bool,
    /// Показывать мышь/колесо SVG-иконкой (с подсветкой нажатой кнопки).
    /// НЕЗАВИСИМО от режима кейкапов: работает и в классике (иконка на
    /// бабле вместо текста «ЛКМ»), и в кейкапах. Цвета — mouse_body/mouse_outline,
    /// подсветка — click_color/scroll_color.
    pub mouse_icons: bool,
    /// Цвет корпуса SVG-мыши (RGBA 0..1, прямая альфа).
    pub mouse_body: [f32; 4],
    /// Цвет контура SVG-мыши (RGBA 0..1, прямая альфа).
    pub mouse_outline: [f32; 4],
    /// Множитель высоты кейкапов (0.5 .. 2.5). 1.0 — как в v0.3.
    pub keycap_height: f32,
    /// Картинка-кейкап по умолчанию (PNG/JPG/GIF/BMP/ICO/SVG). Пусто —
    /// стандартный SVG-кейкап из настроек. Относительный путь — от папки
    /// с keypress.toml, абсолютный — как есть.
    pub keycap_image: String,
    /// Картинка для конкретной клавиши (перекрывает keycap_image):
    /// ключ — имя как в виджете («A», «Ctrl», «Пробел»), значение — путь.
    pub keycap_images: BTreeMap<String, String>,
    /// Сдвиг подписи на кейкапе по X (в точках, умножается на масштаб виджета)
    pub keycap_text_dx: f32,
    /// Сдвиг подписи на кейкапе по Y (в точках, умножается на масштаб виджета)
    pub keycap_text_dy: f32,
    /// Масштаб подписи на кейкапе (0.3 .. 3.0)
    pub keycap_text_scale: f32,
    /// Сколько последних значений держать в виджете клавиш (1..10)
    pub max_keys: usize,
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
            show_cyrillic: true,
            keycap_style: false,
            mouse_icons: true,
            mouse_body: [0.08, 0.08, 0.10, 0.85],
            mouse_outline: [1.00, 1.00, 1.00, 1.00],
            keycap_height: 1.0,
            keycap_image: String::new(),
            keycap_images: BTreeMap::new(),
            keycap_text_dx: 0.0,
            keycap_text_dy: 0.0,
            keycap_text_scale: 1.0,
            max_keys: 4,
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

/// Путь к конфигу. Порядок выбора:
///
/// 1. Переменная окружения KEYPRESS_CONFIG — явное переопределение.
/// 2. Папка exe, если в неё можно писать (портативная установка).
/// 3. %APPDATA%\keypress — UIAccess-установка: exe лежит в Program Files
///    (требование uiAccess=true), писать в эту папку без прав администратора
///    нельзя, а панель настроек должна сохранять конфиг из-под обычного
///    пользователя.
///
/// Результат кешируется: положение exe и окружение не меняются за жизнь
/// процесса, а перечитывание конфига по mtime вызывает функцию постоянно.
#[cfg_attr(not(windows), allow(dead_code))] // используется Windows-кодом
pub fn config_path() -> PathBuf {
    static CACHED: OnceLock<PathBuf> = OnceLock::new();
    CACHED
        .get_or_init(|| {
            let exe_dir = std::env::current_exe()
                .ok()
                .and_then(|exe| exe.parent().map(Path::to_path_buf));
            resolve_config_path(
                exe_dir.as_deref(),
                std::env::var("KEYPRESS_CONFIG")
                    .ok()
                    .filter(|s| !s.is_empty())
                    .as_deref(),
                exe_dir.as_deref().map(dir_writable).unwrap_or(false),
                std::env::var("APPDATA").ok().as_deref(),
            )
        })
        .clone()
}

/// Чистая логика выбора пути конфига (покрыта тестами).
fn resolve_config_path(
    exe_dir: Option<&Path>,
    env_override: Option<&str>,
    exe_dir_writable: bool,
    appdata: Option<&str>,
) -> PathBuf {
    if let Some(p) = env_override {
        if !p.is_empty() {
            return PathBuf::from(p);
        }
    }
    if let Some(dir) = exe_dir {
        if exe_dir_writable {
            return dir.join("keypress.toml");
        }
    }
    if let Some(ad) = appdata {
        return Path::new(ad).join("keypress").join("keypress.toml");
    }
    PathBuf::from("keypress.toml")
}

/// Можно ли создавать файлы в папке (пробная запись + удаление).
fn dir_writable(dir: &Path) -> bool {
    let probe = dir.join(".keypress-write-probe");
    match std::fs::File::create(&probe) {
        Ok(_) => {
            let _ = std::fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

/// Ограничение значений разумными пределами (совпадает с лимитами слайдеров панели).
pub fn normalized(mut cfg: Config) -> Config {
    let clamp = |v: f32, lo: f32, hi: f32| v.clamp(lo, hi);
    cfg.scale = clamp(cfg.scale, 0.5, 3.0);
    cfg.pos_x = clamp(cfg.pos_x, 0.0, 1.0);
    cfg.pos_y = clamp(cfg.pos_y, 0.0, 1.0);
    cfg.key_duration = clamp(cfg.key_duration, 0.5, 5.0);
    cfg.key_radius = clamp(cfg.key_radius, 0.0, 40.0);
    cfg.max_keys = cfg.max_keys.clamp(1, 10);
    cfg.keycap_height = clamp(cfg.keycap_height, 0.5, 2.5);
    cfg.keycap_text_dx = clamp(cfg.keycap_text_dx, -50.0, 50.0);
    cfg.keycap_text_dy = clamp(cfg.keycap_text_dy, -50.0, 50.0);
    cfg.keycap_text_scale = clamp(cfg.keycap_text_scale, 0.3, 3.0);
    // Пустые значения картинок выкидываем, остальные тримим
    cfg.keycap_image = cfg.keycap_image.trim().to_string();
    cfg.keycap_images.retain(|_, v| !v.trim().is_empty());
    for v in cfg.keycap_images.values_mut() {
        *v = v.trim().to_string();
    }
    for c in [
        &mut cfg.key_bg,
        &mut cfg.key_text,
        &mut cfg.click_color,
        &mut cfg.scroll_color,
        &mut cfg.mouse_body,
        &mut cfg.mouse_outline,
    ] {
        for comp in c {
            *comp = clamp(*comp, 0.0, 1.0);
        }
    }
    cfg
}

/// FNV-1a 64: короткая хеш-функция без зависимостей (ключ кеша текстур
/// кастомных кейкапов: путь + mtime + размер).
pub fn fnv1a(parts: &[&[u8]]) -> u64 {
    let mut h: u64 = 0xcbf2_9ce4_8422_2325;
    for chunk in parts {
        for &b in *chunk {
            h ^= b as u64;
            h = h.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    h
}

/// Путь к картинке кейкапа для подписи `label`: точное совпадение в
/// keycap_images, затем без учёта регистра, затем общий keycap_image.
/// Пустые значения пропускаются. None — использовать стандартный кейкап.
pub fn keycap_image_for<'a>(
    global: &'a str,
    per_key: &'a BTreeMap<String, String>,
    label: &str,
) -> Option<&'a str> {
    if let Some(p) = per_key.get(label) {
        if !p.trim().is_empty() {
            return Some(p.as_str());
        }
    }
    if let Some((_, p)) = per_key
        .iter()
        .find(|(k, _)| k.to_lowercase() == label.to_lowercase())
    {
        if !p.trim().is_empty() {
            return Some(p.as_str());
        }
    }
    if !global.is_empty() {
        return Some(global);
    }
    None
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
        std::env::temp_dir().join(format!("keypress-test-{}-{}.toml", tag, std::process::id()))
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
    fn new_options_defaults() {
        let c = Config::default();
        assert!(c.show_cyrillic);
        assert_eq!(c.max_keys, 4);
        // кейкапы — включаемый режим, основной вариант по умолчанию
        assert!(!c.keycap_style);
    }

    #[test]
    fn partial_file_fills_new_option_defaults() {
        let path = tmp_path("partial-new");
        std::fs::write(&path, "scale = 1.5\n").unwrap();
        let c = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert!(c.show_cyrillic);
        assert_eq!(c.max_keys, 4);
        assert!(!c.keycap_style);
    }

    #[test]
    fn keycap_style_roundtrip() {
        let path = tmp_path("keycap");
        let mut c = Config::default();
        c.keycap_style = true;
        save_to(&path, &c).expect("save failed");
        let loaded = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert!(loaded.keycap_style);
    }

    #[test]
    fn normalize_clamps_max_keys() {
        let c = normalized(Config {
            max_keys: 0,
            ..Config::default()
        });
        assert_eq!(c.max_keys, 1);
        let c = normalized(Config {
            max_keys: 99,
            ..Config::default()
        });
        assert_eq!(c.max_keys, 10);
    }

    #[test]
    fn load_applies_normalize() {
        let path = tmp_path("normload");
        std::fs::write(&path, "scale = 77.0\n").unwrap();
        let c = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(c.scale, 3.0);
    }

    #[test]
    fn mouse_settings_defaults_match_key_colors() {
        let c = Config::default();
        assert!(c.mouse_icons, "иконки мыши включены по умолчанию");
        // Дефолты совпадают с цветами клавиш — вид как в v0.3
        assert_eq!(c.mouse_body, c.key_bg);
        assert_eq!(c.mouse_outline, c.key_text);
    }

    #[test]
    fn mouse_settings_roundtrip() {
        let path = tmp_path("mouse-rt");
        let mut c = Config::default();
        c.mouse_icons = false;
        c.mouse_body = [0.9, 0.2, 0.3, 0.7];
        c.mouse_outline = [0.1, 0.9, 0.2, 1.0];
        save_to(&path, &c).expect("save failed");
        let loaded = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert!(!loaded.mouse_icons);
        assert_eq!(loaded.mouse_body, [0.9, 0.2, 0.3, 0.7]);
        assert_eq!(loaded.mouse_outline, [0.1, 0.9, 0.2, 1.0]);
    }

    #[test]
    fn mouse_settings_clamped() {
        let mut c = Config::default();
        c.mouse_body = [2.0, -1.0, 0.5, 1.5];
        let c = normalized(c);
        assert_eq!(c.mouse_body, [1.0, 0.0, 0.5, 1.0]);
    }

    #[test]
    fn partial_file_fills_mouse_defaults() {
        let path = tmp_path("partial-mouse");
        std::fs::write(&path, "scale = 1.5\n").unwrap();
        let c = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert!(c.mouse_icons);
        assert_eq!(c.mouse_body, Config::default().mouse_body);
    }

    #[test]
    fn keycap_custom_defaults() {
        let c = Config::default();
        assert_eq!(c.keycap_height, 1.0);
        assert_eq!(c.keycap_text_dx, 0.0);
        assert_eq!(c.keycap_text_dy, 0.0);
        assert_eq!(c.keycap_text_scale, 1.0);
        assert!(c.keycap_image.is_empty());
        assert!(c.keycap_images.is_empty());
        // Пустая настройка = стандартный кейкап
        assert!(keycap_image_for("", &c.keycap_images, "A").is_none());
    }

    #[test]
    fn keycap_image_for_priority() {
        let mut map = BTreeMap::new();
        map.insert("A".to_string(), "a.png".to_string());
        map.insert("ctrl".to_string(), "ctrl.png".to_string());
        map.insert("Empty".to_string(), "   ".to_string());
        // точное совпадение
        assert_eq!(keycap_image_for("all.png", &map, "A"), Some("a.png"));
        // без учёта регистра
        assert_eq!(keycap_image_for("all.png", &map, "CTRL"), Some("ctrl.png"));
        assert_eq!(keycap_image_for("all.png", &map, "Ctrl"), Some("ctrl.png"));
        // пустая по-клавишная запись пропускается -> общий
        assert_eq!(keycap_image_for("all.png", &map, "empty"), Some("all.png"));
        // нет нигде — общий
        assert_eq!(keycap_image_for("all.png", &map, "Shift"), Some("all.png"));
        // нет нигде и общий пуст — стандартный
        assert!(keycap_image_for("", &map, "Shift").is_none());
    }

    #[test]
    fn keycap_custom_roundtrip() {
        let path = tmp_path("cap-rt");
        let mut c = Config::default();
        c.keycap_height = 1.4;
        c.keycap_image = "caps/base.png".to_string();
        c.keycap_images
            .insert("A".to_string(), "caps/a.png".to_string());
        c.keycap_images
            .insert("Пробел".to_string(), "caps/space.png".to_string());
        c.keycap_text_dx = -3.5;
        c.keycap_text_dy = 2.0;
        c.keycap_text_scale = 0.8;
        save_to(&path, &c).expect("save failed");
        let loaded = load_from(&path);
        let _ = std::fs::remove_file(&path);
        assert_eq!(loaded.keycap_height, 1.4);
        assert_eq!(loaded.keycap_image, "caps/base.png");
        assert_eq!(
            loaded.keycap_images.get("Пробел").map(String::as_str),
            Some("caps/space.png")
        );
        assert_eq!(loaded.keycap_text_dx, -3.5);
        assert_eq!(loaded.keycap_text_scale, 0.8);
    }

    #[test]
    fn keycap_custom_clamped() {
        let mut c = Config::default();
        c.keycap_height = 99.0;
        c.keycap_text_dx = 100.0;
        c.keycap_text_dy = -100.0;
        c.keycap_text_scale = 0.01;
        let c = normalized(c);
        assert_eq!(c.keycap_height, 2.5);
        assert_eq!(c.keycap_text_dx, 50.0);
        assert_eq!(c.keycap_text_dy, -50.0);
        assert_eq!(c.keycap_text_scale, 0.3);
    }

    #[test]
    fn fnv1a_is_stable_and_depends_on_input() {
        let a = fnv1a(&[b"caps/a.png"]);
        let b = fnv1a(&[b"caps/a.png"]);
        let c = fnv1a(&[b"caps/b.png"]);
        assert_eq!(a, b, "хеш детерминирован");
        assert_ne!(a, c);
        // склейка частей эквивалентна одной строке
        assert_eq!(fnv1a(&[b"caps/", b"a.png"]), fnv1a(&[b"caps/a.png"]));
    }

    #[test]
    fn config_path_env_override_wins() {
        let p = resolve_config_path(
            Some(Path::new("C:\\app")),
            Some("D:\\cfg\\k.toml"),
            true,
            Some("C:\\AD"),
        );
        assert_eq!(p, PathBuf::from("D:\\cfg\\k.toml"));
        // Пустое переопределение игнорируется
        let p = resolve_config_path(Some(Path::new("C:\\app")), Some(""), true, Some("C:\\AD"));
        assert_eq!(p, Path::new("C:\\app").join("keypress.toml"));
    }

    #[test]
    fn config_path_prefers_writable_exe_dir() {
        let p = resolve_config_path(Some(Path::new("/opt/app")), None, true, Some("/ad"));
        assert_eq!(p, PathBuf::from("/opt/app/keypress.toml"));
    }

    #[test]
    fn config_path_falls_back_to_appdata_when_locked() {
        // UIAccess-установка: exe в Program Files, писать туда нельзя
        let p = resolve_config_path(
            Some(Path::new("C:\\Program Files\\keypress")),
            None,
            false,
            Some("C:\\Users\\u\\AppData\\Roaming"),
        );
        assert_eq!(
            p,
            Path::new("C:\\Users\\u\\AppData\\Roaming")
                .join("keypress")
                .join("keypress.toml")
        );
    }

    #[test]
    fn config_path_no_exe_dir_legacy_relative() {
        let p = resolve_config_path(None, None, false, None);
        assert_eq!(p, PathBuf::from("keypress.toml"));
    }
}
