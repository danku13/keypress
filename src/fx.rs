//! Чистая математика эффектов оверлея: альфа-конверты, пружинка, радиусы, цвета.
//! Вынесено из overlay.rs для TDD — не зависит от egui/eframe.

/// Прямая альфа (как в конфиге) -> premultiplied RGBA-байты (как любит egui).
/// `mul` — дополнительный множитель альфы (анимация затухания).
pub fn premultiply(c: [f32; 4], mul: f32) -> [u8; 4] {
    let a = (c[3] * mul).clamp(0.0, 1.0);
    let f = |v: f32| -> u8 { (v * a * 255.0) as u8 };
    [f(c[0]), f(c[1]), f(c[2]), (a * 255.0) as u8]
}

/// Альфа комбо-клавиши по нормализованному времени жизни (t = age / duration).
/// Держится полной до 70% жизни, затем линейно гаснет до 0.
pub fn bubble_alpha(t: f32) -> f32 {
    if t < 0.7 {
        1.0
    } else {
        (1.0 - (t - 0.7) / 0.3).clamp(0.0, 1.0)
    }
}

/// «Пружинка» появления: масштаб 0.85 -> 1.0 за первые 80 мс.
/// `age` — возраст в секундах.
pub fn pop_zoom(age: f32) -> f32 {
    0.85 + 0.15 * (age / 0.08).min(1.0)
}

/// Радиус ряби клика: базовый радиус + рост за время анимации.
pub fn ripple_radius(scale: f32, t: f32) -> f32 {
    10.0 * scale + 34.0 * scale * t
}

/// Смещение треугольника прокрутки от точки колеса (вверх — против оси Y).
pub fn scroll_shift(scale: f32, t: f32) -> f32 {
    26.0 * scale + 10.0 * scale * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_close(a: f32, b: f32, eps: f32) {
        assert!((a - b).abs() <= eps, "left={a} right={b}");
    }

    #[test]
    fn premultiply_white_opaque() {
        assert_eq!(premultiply([1.0, 1.0, 1.0, 1.0], 1.0), [255, 255, 255, 255]);
    }

    #[test]
    fn premultiply_multiplies_rgb_by_alpha() {
        // a=0.5 -> rgb 255*0.5=127.5 -> 127, alpha 127
        assert_eq!(premultiply([1.0, 0.0, 0.0, 0.5], 1.0), [127, 0, 0, 127]);
    }

    #[test]
    fn premultiply_fade_multiplier() {
        // mul=0.5 поверх a=1: rgb и альфа делятся пополам
        assert_eq!(premultiply([1.0, 1.0, 1.0, 1.0], 0.5), [127, 127, 127, 127]);
    }

    #[test]
    fn premultiply_clamps() {
        assert_eq!(premultiply([2.0, -1.0, 0.5, 2.0], 1.0), [255, 0, 127, 255]);
        // отрицательный mul -> всё в 0
        assert_eq!(premultiply([1.0, 1.0, 1.0, 1.0], -1.0), [0, 0, 0, 0]);
    }

    #[test]
    fn bubble_alpha_full_until_70_percent() {
        assert_eq!(bubble_alpha(0.0), 1.0);
        assert_eq!(bubble_alpha(0.5), 1.0);
        assert_eq!(bubble_alpha(0.7), 1.0);
    }

    #[test]
    fn bubble_alpha_fades_linearly_to_zero() {
        assert_close(bubble_alpha(0.85), 0.5, 0.001);
        assert_eq!(bubble_alpha(1.0), 0.0);
        assert_eq!(bubble_alpha(1.5), 0.0); // перелив времени — не ярче 0
    }

    #[test]
    fn pop_zoom_grows_to_one() {
        assert_close(pop_zoom(0.0), 0.85, 0.001);
        assert_close(pop_zoom(0.04), 0.925, 0.001);
        assert_close(pop_zoom(0.08), 1.0, 0.001);
        assert_close(pop_zoom(10.0), 1.0, 0.001); // после 80 мс — фикс
    }

    #[test]
    fn ripple_radius_grows_linearly() {
        assert_close(ripple_radius(1.0, 0.0), 10.0, 0.001);
        assert_close(ripple_radius(1.0, 1.0), 44.0, 0.001);
        assert_close(ripple_radius(2.0, 0.5), 54.0, 0.001); // масштаб умножает всё
    }

    #[test]
    fn scroll_shift_moves_away_from_cursor() {
        assert_close(scroll_shift(1.0, 0.0), 26.0, 0.001);
        assert_close(scroll_shift(1.0, 1.0), 36.0, 0.001);
    }
}
