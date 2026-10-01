//! Конвертации цвета: конфиг `[f32;4]` (прямой альфа-канал) <-> пикер/SVG.
//!
//! РЕГРЕССИЯ «цвет съезжает в чёрный» (v0.2): старый код упаковывал цвет в
//! egui::Color32 как unmultiplied, а читал байты обратно как прямые. egui
//! хранит Color32 в premultiplied виде, поэтому каждый кадр RGB ещё раз
//! умножался на альфу -> цвет экспоненциально затухал в чёрный (для a=0.85
//! за ~30 кадров). Фикс: RGB редактируется пикером как НЕПРОЗРАЧНЫЙ
//! (color_edit_button_srgb), альфа вынесена в отдельный слайдер; конверсии
//! этой модуля лишены потери (доказано тестом стабильности ниже).

/// Разбор цвета конфига на RGB для пикера (непрозрачный) и альфа-байт.
pub fn split(c: [f32; 4]) -> ([u8; 3], u8) {
    let f = |v: f32| -> u8 { (v.clamp(0.0, 1.0) * 255.0).round() as u8 };
    ([f(c[0]), f(c[1]), f(c[2])], f(c[3]))
}

/// Сборка цвета конфига из RGB пикера и альфа-байта.
pub fn join(rgb: [u8; 3], a: u8) -> [f32; 4] {
    let f = |v: u8| v as f32 / 255.0;
    [f(rgb[0]), f(rgb[1]), f(rgb[2]), f(a)]
}

/// Hex цвета для SVG: "#RRGGBBAA" (прямой альфа-канал — так понимает resvg).
pub fn hex_argb(c: [f32; 4]) -> String {
    let (rgb, a) = split(c);
    format!("#{:02X}{:02X}{:02X}{:02X}", rgb[0], rgb[1], rgb[2], a)
}

/// Затемнение/осветление hex-цвета "#RRGGBB"/"#RRGGBBAA" умножением RGB на k
/// (альфа сохраняется). Для теней и боковин кейкапов.
pub fn hex_shade(hex: &str, k: f32) -> String {
    let h = hex.trim_start_matches('#');
    let n = h.len();
    if n < 6 {
        return hex.to_string();
    }
    let comp = |i: usize| -> u8 {
        h.get(i * 2..i * 2 + 2)
            .and_then(|s| u8::from_str_radix(s, 16).ok())
            .unwrap_or(0)
    };
    let f = |v: u8| -> u8 { ((v as f32 * k).round().clamp(0.0, 255.0)) as u8 };
    let r = f(comp(0));
    let g = f(comp(1));
    let b = f(comp(2));
    if n >= 8 {
        format!("#{:02X}{:02X}{:02X}{:02X}", r, g, b, comp(3))
    } else {
        format!("#{:02X}{:02X}{:02X}", r, g, b)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round(v: f32) -> f32 {
        (v * 255.0).round() / 255.0
    }

    #[test]
    fn split_join_roundtrip() {
        let c = [0.2, 0.45, 0.8, 0.85];
        let (rgb, a) = split(c);
        let back = join(rgb, a);
        for i in 0..4 {
            assert!((back[i] - c[i]).abs() <= 1.0 / 255.0);
        }
    }

    #[test]
    fn opaque_color_is_exact() {
        let c = [1.0, 0.0, 0.5, 1.0];
        let (rgb, a) = split(c);
        assert_eq!(a, 255);
        let back = join(rgb, a);
        for i in 0..4 {
            assert!((back[i] - c[i]).abs() <= 1.0 / 255.0);
        }
    }

    /// ГЛАВНЫЙ регрессионный тест: много кадров split->join подряд не меняют
    /// цвет. Прежний код (premultiplied/unmultiplied-смешение) каждый кадр
    /// умножал RGB на альфу — цвет за секунду «съезжал в чёрный».
    #[test]
    fn no_drift_over_100_edit_cycles() {
        let mut c = [1.0, 0.45, 0.2, 0.9];
        for _ in 0..100 {
            let (rgb, a) = split(c);
            c = join(rgb, a);
            for comp in c {
                assert!(comp >= 0.0 && comp <= 1.0);
            }
        }
        assert_eq!(c[0], round(c[0]));
        // После первой стабилизации значение вообще не меняется
        let stable = c;
        for _ in 0..50 {
            let (rgb, a) = split(stable);
            assert_eq!(join(rgb, a), stable);
        }
        // И главное — RGB не затух: красный канал остаётся красным
        assert!(c[0] > 0.9);
        assert!(c[1] > 0.4);
    }

    #[test]
    fn alpha_zero_is_safe() {
        let c = [0.5, 0.5, 0.5, 0.0];
        let (rgb, a) = split(c);
        assert_eq!(a, 0);
        let back = join(rgb, a);
        assert_eq!(back[3], 0.0);
        // RGB сохраняется, а не обнуляется (допуск квантования 1/255)
        assert!((back[0] - c[0]).abs() <= 1.0 / 255.0);
    }

    #[test]
    fn hex_format() {
        assert_eq!(hex_argb([1.0, 0.0, 0.0, 1.0]), "#FF0000FF");
        assert_eq!(hex_argb([0.0, 1.0, 0.0, 0.5]), "#00FF0080");
        assert_eq!(hex_argb([0.5, 0.5, 0.5, 0.0]), "#80808000");
    }

    #[test]
    fn shade_multiplies_rgb_keeps_alpha() {
        assert_eq!(hex_shade("#FF0000FF", 0.5), "#800000FF");
        assert_eq!(hex_shade("#FFFFFF", 0.25), "#404040");
        // осветление с клампом
        assert_eq!(hex_shade("#C0C0C0", 2.0), "#FFFFFF");
    }
}
