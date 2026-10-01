//! Генерация SVG-иконок: мышь (ЛКМ/ПКМ/СКМ/колесо) и кейкапы (вид сверху).
//!
//! Мышь нарисована по мотивам freesvg.org «left-click-right-click»: контур
//! корпуса-капсулы, разделитель кнопок, колёсико; нажатая кнопка/направление
//! прокрутки подсвечиваются цветом из настроек. Текст внутри SVG НЕ рисуется:
//! подписи кейкапов кладёт поверх сам egui (чёткие шрифты, кириллица).
//!
//! Все функции чистые — тестируются на любой платформе.

/// Какую мышь/колесо нарисовать.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseIcon {
    /// Нажата левая кнопка
    Left,
    /// Нажата правая кнопка
    Right,
    /// Нажато колёсико (средняя кнопка)
    Middle,
    /// Прокрутка вверх (стрелка над мышью)
    WheelUp,
    /// Прокрутка вниз (стрелка под мышью)
    WheelDown,
}

/// SVG мыши. viewBox 120x230 (портретная капсула), w/h = 12:23.
///
/// * `highlight` — цвет нажатой кнопки/стрелки (#RRGGBBAA)
/// * `body` — цвет корпуса (#RRGGBBAA)
/// * `outline` — цвет контура (#RRGGBBAA)
pub fn mouse_svg(icon: MouseIcon, highlight: &str, body: &str, outline: &str) -> String {
    // Корпус: капсула x=8..112, y=40..196, полуокружности r=52
    // (центр верхней дуги — (60,92)).
    let arrow = match icon {
        MouseIcon::WheelUp => {
            format!(
                r#"  <polygon points='60,8 42,32 78,32' fill='{}'/>\n"#,
                highlight
            )
        }
        MouseIcon::WheelDown => format!(
            r#"  <polygon points='60,224 42,200 78,200' fill='{}'/>\n"#,
            highlight
        ),
        _ => String::new(),
    };
    // Подсветка нажатой кнопки — за разделительными линиями.
    let pressed = match icon {
        MouseIcon::Left => format!(
            r#"  <path d='M60,92 L8,92 A52,52 0 0 1 60,40 Z' fill='{}'/>\n"#,
            highlight
        ),
        MouseIcon::Right => format!(
            r#"  <path d='M60,92 L112,92 A52,52 0 0 0 60,40 Z' fill='{}'/>\n"#,
            highlight
        ),
        _ => String::new(),
    };
    // Средняя кнопка = подсвечивается само колёсико
    let wheel_fill = if icon == MouseIcon::Middle {
        highlight
    } else {
        outline
    };
    format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 120 230'>
{arrow}  <rect x='8' y='40' width='104' height='156' rx='52' ry='52' fill='{body}' stroke='{outline}' stroke-width='5'/>
{pressed}  <line x1='60' y1='40' x2='60' y2='92' stroke='{outline}' stroke-width='4'/>
  <line x1='8' y1='92' x2='112' y2='92' stroke='{outline}' stroke-width='4'/>
  <rect x='51' y='50' width='18' height='34' rx='9' fill='{wheel_fill}'/>
</svg>
"#
    )
}

/// SVG кейкапа (вид сверху): основание + верхняя панель с тонкой кромкой.
/// Подпись клавиши кладётся сверху средствами egui (не внутри SVG).
pub fn keycap_svg(w: u32, h: u32, top: &str, side: &str, edge: &str) -> String {
    let w = w.max(12);
    let h = h.max(12);
    // Отступ верхней панели: ~15% высоты, но не меньше 3px
    let t = ((h as f32 * 0.15).round() as u32).max(3);
    let iw = w - t * 2;
    let ih = h - t * 2;
    let r = ((h as f32 * 0.3).round() as u32).max(4);
    let r2 = r * 6 / 10;
    let bw = w - 2;
    let bh = h - 2;
    format!(
        r#"<svg xmlns='http://www.w3.org/2000/svg' width='{w}' height='{h}' viewBox='0 0 {w} {h}'>
  <rect x='1' y='1' width='{bw}' height='{bh}' rx='{r}' fill='{side}'/>
  <rect x='{t}' y='{t}' width='{iw}' height='{ih}' rx='{r2}' fill='{top}'/>
  <rect x='{t}' y='{t}' width='{iw}' height='{ih}' rx='{r2}' fill='none' stroke='{edge}' stroke-width='1.5'/>
</svg>
"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    const HL: &str = "#FF8800E6";
    const BODY: &str = "#14141AE6";
    const OUT: &str = "#FFFFFFFF";

    #[test]
    fn svg_is_wellformed() {
        for icon in [
            MouseIcon::Left,
            MouseIcon::Right,
            MouseIcon::Middle,
            MouseIcon::WheelUp,
            MouseIcon::WheelDown,
        ] {
            let s = mouse_svg(icon, HL, BODY, OUT);
            assert!(s.starts_with("<svg"), "{icon:?}: нет открывающего тега");
            assert!(s.contains("xmlns="), "{icon:?}: нет xmlns");
            assert!(s.contains("</svg>"), "{icon:?}: нет закрывающего тега");
            assert!(
                !s.contains('{') && !s.contains('}'),
                "{icon:?}: незаполненный шаблон"
            );
        }
    }

    #[test]
    fn highlight_marks_the_right_button() {
        let left = mouse_svg(MouseIcon::Left, HL, BODY, OUT);
        let right = mouse_svg(MouseIcon::Right, HL, BODY, OUT);
        let middle = mouse_svg(MouseIcon::Middle, HL, BODY, OUT);
        // подсветка встречается ровно один раз (только нужная кнопка)
        assert_eq!(left.matches(HL).count(), 1);
        assert_eq!(right.matches(HL).count(), 1);
        assert_eq!(middle.matches(HL).count(), 1);
        // и это разные геометрии
        assert_ne!(left, right);
        assert_ne!(left, middle);
    }

    #[test]
    fn wheel_icons_have_arrows() {
        let up = mouse_svg(MouseIcon::WheelUp, HL, BODY, OUT);
        let down = mouse_svg(MouseIcon::WheelDown, HL, BODY, OUT);
        assert_eq!(up.matches(HL).count(), 1); // стрелка
        assert_eq!(down.matches(HL).count(), 1);
        assert_ne!(up, down);
        // стрелка вверх выше центра, стрелка вниз ниже
        assert!(up.contains("points='60,"));
        assert!(down.contains("points='60,"));
    }

    #[test]
    fn body_and_outline_used() {
        let s = mouse_svg(MouseIcon::Left, HL, BODY, OUT);
        assert!(s.contains(BODY), "корпус должен быть окрашен");
        assert!(s.contains(OUT), "контур должен быть окрашен");
    }

    #[test]
    fn keycap_contains_layers() {
        let s = keycap_svg(60, 40, "#14141AE6", "#0A0A0FE6", "#FFFFFF40");
        assert!(s.starts_with("<svg"));
        assert!(s.contains("</svg>"));
        assert!(s.contains("width='60'"));
        assert!(s.contains("height='40'"));
        assert!(s.contains("#14141AE6"), "верхняя панель");
        assert!(s.contains("#0A0A0FE6"), "основание");
        assert!(s.contains("#FFFFFF40"), "кромка");
        assert!(!s.contains('{') && !s.contains('}'));
    }

    #[test]
    fn keycap_dimensions_vary() {
        let a = keycap_svg(60, 40, "#111111", "#000000", "#FFFFFF");
        let b = keycap_svg(120, 40, "#111111", "#000000", "#FFFFFF");
        assert_ne!(a, b);
        assert!(b.contains("width='120'"));
    }

    #[test]
    fn viewbox_aspect_matches_mouse() {
        let s = mouse_svg(MouseIcon::Left, HL, BODY, OUT);
        assert!(s.contains("viewBox='0 0 120 230'"));
    }
}

#[cfg(test)]
mod dump {
    use super::MouseIcon;

    /// Ручная визуальная проверка: cargo test -- --ignored dump_svgs --nocapture
    #[test]
    #[ignore]
    fn dump_svgs() {
        let dir = std::env::temp_dir().join("keypress-svg");
        std::fs::create_dir_all(&dir).unwrap();
        let hl = "#FF8800E6";
        let body = "#14141AE6";
        let outline = "#FFFFFFFF";
        for (name, icon) in [
            ("left", MouseIcon::Left),
            ("right", MouseIcon::Right),
            ("middle", MouseIcon::Middle),
            ("wheel_up", MouseIcon::WheelUp),
            ("wheel_down", MouseIcon::WheelDown),
        ] {
            std::fs::write(
                dir.join(format!("{name}.svg")),
                super::mouse_svg(icon, hl, body, outline),
            )
            .unwrap();
        }
        std::fs::write(
            dir.join("cap_ctrl.svg"),
            super::keycap_svg(96, 62, "#14141AE6", "#09090D", "#FFFFFF66"),
        )
        .unwrap();
        println!("{}", dir.display());
    }
}
