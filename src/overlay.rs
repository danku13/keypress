//! Оверлей: прозрачное, always-on-top, click-through окно на весь экран.
//! Рисует клавиши, клики и прокрутку. Ничего не перехватывает, только показывает.
//!
//! Виджет клавиш — два режима:
//! - классический (по умолчанию): комбо текстом на фоновом бабле;
//! - кейкапы (cfg.keycap_style): каждая клавиша — SVG-кейкап «вид сверху»,
//!   мышь/колесо — SVG-иконки, части разделены «+».
//! Иконки генерируются из шаблонов (icons.rs), растятся через egui_extras
//! svg-лоадер и рисуются painter-ом с tint-фейдом.

use crate::config::{config_path, load, ClickShape, Config};
use crate::hooks::spawn_hooks;
use crate::icons::{keycap_svg, mouse_svg, MouseIcon};
use crate::input::UiEvent;
use crate::keys::{combo_text, Part};
use crate::winutil::apply_overlay_styles;
use crossbeam_channel::{Receiver, Sender};
use eframe::egui;
use std::collections::HashMap;
use std::time::{Duration, Instant};

const CLICK_LIFE: f32 = 0.55;
const SCROLL_LIFE: f32 = 0.5;

/// UV-рект всей текстуры для painter.image
const UV: egui::Rect = egui::Rect::from_min_max(egui::Pos2::ZERO, egui::pos2(1.0, 1.0));

/// Пропорции SVG-иконки мыши (viewBox 120x230)
const MOUSE_W: f32 = 120.0;
const MOUSE_H: f32 = 230.0;

struct Bubble {
    parts: Vec<Part>,
    born: Instant,
}

impl Bubble {
    fn is_wheel(&self, up: bool) -> bool {
        self.parts.iter().any(|p| *p == Part::Wheel(up))
    }
}

struct Ripple {
    born: Instant,
    x: f32,
    y: f32,
}

struct ScrollFx {
    born: Instant,
    x: f32,
    y: f32,
    up: bool,
}

pub struct OverlayApp {
    cfg: Config,
    rx: Receiver<UiEvent>,
    /// Копия отправителя: трей может быть включён на лету из панели настроек
    /// (config перечитывается по mtime) — нужен Sender для spawn().
    tx: Sender<UiEvent>,
    /// Трей сейчас запущен (синхронизирован с cfg.tray_icon через sync_tray).
    tray_active: bool,
    bubbles: Vec<Bubble>,
    ripples: Vec<Ripple>,
    scrolls: Vec<ScrollFx>,
    paused: bool,
    started: bool,
    cfg_last_check: Instant,
    cfg_mtime: Option<std::time::SystemTime>,
    /// Готовые текстуры иконок по URI (bytes://...)
    textures: HashMap<String, egui::TextureHandle>,
}

/// Прямой альфа-канал (как в конфиге) -> premultiplied (как любит egui).
/// Конвертация считается в fx::premultiply (покрыта тестами).
fn col(c: [f32; 4], mul: f32) -> egui::Color32 {
    let [r, g, b, a] = crate::fx::premultiply(c, mul);
    egui::Color32::from_rgba_premultiplied(r, g, b, a)
}

/// Центрированный текст в точке (фолбэк «иконки ещё не готовы» и текст
/// вместо иконки при выключенных SVG-мышах).
fn draw_label(
    painter: &egui::Painter,
    label: &str,
    font_size: f32,
    center: egui::Pos2,
    color: egui::Color32,
) {
    let galley = painter.layout_no_wrap(
        label.to_string(),
        egui::FontId::proportional(font_size),
        egui::Color32::WHITE,
    );
    painter.galley(
        egui::Pos2::new(
            center.x - galley.size().x / 2.0,
            center.y - galley.size().y / 2.0,
        ),
        galley,
        color,
    );
}

/// Путь к картинке кастомного кейкапа: абсолютный — как есть, относительный —
/// от папки с keypress.toml.
fn resolve_cap_path(rel: &str) -> Option<std::path::PathBuf> {
    let p = std::path::Path::new(rel);
    if p.is_absolute() {
        return Some(p.to_path_buf());
    }
    let base = crate::config::config_path().parent()?.to_path_buf();
    Some(base.join(rel))
}

impl OverlayApp {
    pub fn new(cc: &eframe::CreationContext<'_>, cfg: Config) -> Self {
        // Лоадеры картинок (в т.ч. SVG) ставятся до создания App
        egui_extras::install_image_loaders(&cc.egui_ctx);
        crate::hooks::set_show_cyrillic(cfg.show_cyrillic);
        let (tx, rx) = crossbeam_channel::unbounded();
        spawn_hooks(tx.clone());
        let mut app = Self {
            cfg,
            rx,
            tx,
            tray_active: false,
            bubbles: Vec::new(),
            ripples: Vec::new(),
            scrolls: Vec::new(),
            paused: false,
            started: false,
            cfg_last_check: Instant::now(),
            cfg_mtime: None,
            textures: HashMap::new(),
        };
        app.sync_tray();
        app
    }

    /// Включает/выключает иконку трея в соответствии с cfg.tray_icon.
    /// Зовётся при старте и после перечитывания keypress.toml (галочка в
    /// панели настроек применяется на лету).
    fn sync_tray(&mut self) {
        if self.cfg.tray_icon && !self.tray_active {
            self.tray_active = true;
            crate::tray::spawn(self.tx.clone());
            crate::tray::set_paused(self.paused);
        } else if !self.cfg.tray_icon && self.tray_active {
            self.tray_active = false;
            crate::tray::shutdown();
        }
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                UiEvent::Keys { parts } => {
                    if self.paused || !self.cfg.show_keys || parts.is_empty() {
                        continue;
                    }
                    self.bubbles.push(Bubble {
                        parts,
                        born: Instant::now(),
                    });
                    // Количество строк в стопке настраивается (cfg.max_keys)
                    while self.bubbles.len() > self.cfg.max_keys {
                        self.bubbles.remove(0);
                    }
                }
                UiEvent::WheelPulse { up } => {
                    if self.paused || !self.cfg.show_keys {
                        continue;
                    }
                    // Колесо крутится в ту же сторону: продлеваем жизнь
                    // последней строки прокрутки, чтобы строка не гасла
                    // посреди долгого скролла.
                    let mut alive = false;
                    for b in self.bubbles.iter_mut().rev() {
                        if b.is_wheel(up) {
                            b.born = Instant::now();
                            alive = true;
                            break;
                        }
                    }
                    if !alive {
                        self.bubbles.push(Bubble {
                            parts: vec![Part::Wheel(up)],
                            born: Instant::now(),
                        });
                        while self.bubbles.len() > self.cfg.max_keys {
                            self.bubbles.remove(0);
                        }
                    }
                }
                UiEvent::Click { x, y, .. } => {
                    if self.paused || !self.cfg.show_clicks {
                        continue;
                    }
                    self.ripples.push(Ripple {
                        born: Instant::now(),
                        x,
                        y,
                    });
                }
                UiEvent::Scroll { x, y, up } => {
                    if self.paused || !self.cfg.show_scroll {
                        continue;
                    }
                    self.scrolls.push(ScrollFx {
                        born: Instant::now(),
                        x,
                        y,
                        up,
                    });
                }
                UiEvent::TogglePause => {
                    self.paused = !self.paused;
                    // Галочка «Пауза» в меню трея (если иконка включена;
                    // вызов безопасен и до её создания).
                    crate::tray::set_paused(self.paused);
                }
                UiEvent::Quit => {
                    // Панель настроек — ОТДЕЛЬНЫЙ процесс: без этого после
                    // «Выход» из трея и Ctrl+Alt+Q оставалось висеть её окно.
                    // WM_CLOSE — то же, что крестик; панель не открыта — no-op.
                    crate::winutil::stop_settings();
                    ctx.send_viewport_cmd(egui::ViewportCommand::Close);
                }
            }
        }
    }

    /// Конфиг перечитываем, если файл изменился (панель настроек пишет его сама).
    fn reload_config_if_needed(&mut self) {
        if self.cfg_last_check.elapsed() < Duration::from_millis(300) {
            return;
        }
        self.cfg_last_check = Instant::now();
        let path = config_path();
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if mtime != self.cfg_mtime {
            self.cfg_mtime = mtime;
            self.cfg = load();
            crate::hooks::set_show_cyrillic(self.cfg.show_cyrillic);
            // Галочка «Иконка в трее» применяется на лету
            self.sync_tray();
        }
    }

    fn draw_clicks(&self, painter: &egui::Painter, ppp: f32, now: Instant) {
        for r in &self.ripples {
            let t = now.duration_since(r.born).as_secs_f32() / CLICK_LIFE;
            let alpha = (1.0 - t).max(0.0);
            let c = egui::Pos2::new(r.x / ppp, r.y / ppp);
            let radius = crate::fx::ripple_radius(self.cfg.scale, t);
            match self.cfg.click_shape {
                ClickShape::Circle => {
                    painter.circle_filled(c, radius, col(self.cfg.click_color, alpha));
                }
                ClickShape::Ring => {
                    painter.add(egui::epaint::CircleShape {
                        center: c,
                        radius,
                        fill: egui::Color32::TRANSPARENT,
                        stroke: egui::Stroke::new(
                            3.0 * self.cfg.scale,
                            col(self.cfg.click_color, alpha),
                        ),
                    });
                }
                ClickShape::Square => {
                    let size = radius * 1.6;
                    painter.rect_filled(
                        egui::Rect::from_center_size(c, egui::vec2(size, size)),
                        egui::Rounding::same(6.0 * self.cfg.scale),
                        col(self.cfg.click_color, alpha),
                    );
                }
            }
        }
    }

    fn draw_scroll(&self, painter: &egui::Painter, ppp: f32, now: Instant) {
        for s in &self.scrolls {
            let t = now.duration_since(s.born).as_secs_f32() / SCROLL_LIFE;
            let alpha = (1.0 - t).max(0.0);
            let c = egui::Pos2::new(s.x / ppp, s.y / ppp);
            let shift = crate::fx::scroll_shift(self.cfg.scale, t);
            let tri = 8.0 * self.cfg.scale;
            let cy = if s.up { c.y - shift } else { c.y + shift };
            let pts = if s.up {
                vec![
                    egui::Pos2::new(c.x, cy - tri),
                    egui::Pos2::new(c.x - tri, cy + tri),
                    egui::Pos2::new(c.x + tri, cy + tri),
                ]
            } else {
                vec![
                    egui::Pos2::new(c.x, cy + tri),
                    egui::Pos2::new(c.x - tri, cy - tri),
                    egui::Pos2::new(c.x + tri, cy - tri),
                ]
            };
            painter.add(egui::Shape::convex_polygon(
                pts,
                col(self.cfg.scroll_color, alpha),
                egui::Stroke::NONE,
            ));
        }
    }

    /// Растеризованная SVG-иконка нужного размера (в физических пикселях).
    /// SVG генерируется с подставленными цветами, URI содержит размер и
    /// цвета (по нему кешируются и байты, и текстура).
    fn icon_texture(
        &mut self,
        ctx: &egui::Context,
        uri: String,
        svg: String,
        px: egui::Vec2,
    ) -> Option<egui::TextureHandle> {
        if let Some(h) = self.textures.get(&uri) {
            return Some(h.clone());
        }
        ctx.include_bytes(uri.clone(), svg.into_bytes());
        let options = egui::TextureOptions::LINEAR;
        match ctx.try_load_image(&uri, egui::load::SizeHint::Size(px.x as u32, px.y as u32)) {
            Ok(egui::load::ImagePoll::Ready { image }) => {
                let handle = ctx.load_texture(uri.clone(), (*image).clone(), options);
                self.textures.insert(uri, handle.clone());
                Some(handle)
            }
            _ => None, // ещё растеризуется — кадр рисуем фолбэком (текст)
        }
    }

    /// Иконка мыши для части комбо (Мouse/Wheel). Размер в точках.
    /// Корпус/контур — из настроек мыши (mouse_body/mouse_outline),
    /// независимых от цветов клавиш.
    fn mouse_icon_texture(
        &mut self,
        ctx: &egui::Context,
        icon: MouseIcon,
        h_pt: f32,
        ppp: f32,
    ) -> Option<egui::TextureHandle> {
        let (highlight, name) = match icon {
            MouseIcon::Left => (self.cfg.click_color, "mouse_l"),
            MouseIcon::Right => (self.cfg.click_color, "mouse_r"),
            MouseIcon::Middle => (self.cfg.click_color, "mouse_m"),
            MouseIcon::WheelUp => (self.cfg.scroll_color, "wheel_u"),
            MouseIcon::WheelDown => (self.cfg.scroll_color, "wheel_d"),
        };
        let body = crate::color::hex_argb(self.cfg.mouse_body);
        let outline = crate::color::hex_argb(self.cfg.mouse_outline);
        let hl = crate::color::hex_argb(highlight);
        let w_pt = h_pt * MOUSE_W / MOUSE_H;
        // Размер растра — в физических пикселях, чтобы было чётко при любом ppp
        let px = egui::vec2((w_pt * ppp).round().max(8.0), (h_pt * ppp).round().max(8.0));
        let uri = format!("bytes://{name}_{}_{}.svg", px.x as u32, &hl[1..]);
        let svg = mouse_svg(icon, &hl, &body, &outline);
        self.icon_texture(ctx, uri, svg, px)
    }

    /// Текстура кейкапа под ширину подписи. Ширина квантуется, чтобы не плодить текстуры.
    fn keycap_texture(
        &mut self,
        ctx: &egui::Context,
        w_pt: f32,
        h_pt: f32,
        ppp: f32,
    ) -> Option<egui::TextureHandle> {
        let top = crate::color::hex_argb(self.cfg.key_bg);
        let side = crate::color::hex_shade(&top, 0.45);
        let edge = crate::color::hex_shade(&crate::color::hex_argb(self.cfg.key_text), 0.45);
        let px = egui::vec2((w_pt * ppp).round().max(8.0), (h_pt * ppp).round().max(8.0));
        let uri = format!(
            "bytes://cap_{}_{}_{}.svg",
            px.x as u32,
            px.y as u32,
            &top[1..]
        );
        let svg = keycap_svg(px.x as u32, px.y as u32, &top, &side, &edge);
        self.icon_texture(ctx, uri, svg, px)
    }

    /// Текстура кастомного кейкапа для подписи label (кастомные картинки из
    /// настроек). Raster (PNG/JPG/GIF/BMP/ICO) декодим сами; SVG — через
    /// svg-лоадер egui_extras. Кеш — по хешу (путь+mtime+размер)+размер растра:
    /// файл заменили — текстура перестроится.
    fn custom_cap_texture(
        &mut self,
        ctx: &egui::Context,
        label: &str,
        w_pt: f32,
        h_pt: f32,
        ppp: f32,
    ) -> Option<egui::TextureHandle> {
        let global = self.cfg.keycap_image.clone();
        let per = self.cfg.keycap_images.clone();
        let rel = crate::config::keycap_image_for(&global, &per, label)?.to_string();
        let path = resolve_cap_path(&rel)?;

        let meta = std::fs::metadata(&path).ok()?;
        let mtime = meta
            .modified()
            .ok()
            .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
            .map(|d| d.as_nanos() as u64)
            .unwrap_or(0);
        let px = egui::vec2((w_pt * ppp).round().max(8.0), (h_pt * ppp).round().max(8.0));
        let path_bytes = path.as_os_str().to_string_lossy();
        let hash = crate::config::fnv1a(&[
            path_bytes.as_bytes(),
            &mtime.to_le_bytes(),
            &meta.len().to_le_bytes(),
        ]);
        let is_svg = path
            .extension()
            .and_then(|e| e.to_str())
            .map(|e| e.eq_ignore_ascii_case("svg"))
            .unwrap_or(false);
        let uri = format!(
            "bytes://cap_custom_{:016x}_{}x{}{}",
            hash,
            px.x as u32,
            px.y as u32,
            if is_svg { ".svg" } else { ".img" }
        );
        if let Some(h) = self.textures.get(&uri) {
            return Some(h.clone());
        }
        let bytes = std::fs::read(&path).ok()?;
        if is_svg {
            // SVG-лоадер растеризует под SizeHint, uri обязан кончаться .svg
            return self.icon_texture(ctx, uri, String::from_utf8_lossy(&bytes).into_owned(), px);
        }
        let img = image::load_from_memory(&bytes).ok()?;
        let rgba = img.to_rgba8();
        let (iw, ih) = rgba.dimensions();
        let color =
            egui::ColorImage::from_rgba_unmultiplied([iw as usize, ih as usize], rgba.as_raw());
        let handle = ctx.load_texture(uri.clone(), color, egui::TextureOptions::LINEAR);
        self.textures.insert(uri, handle.clone());
        Some(handle)
    }

    /// Подпись клавиши на кейкапе: масштаб/сдвиги из настроек текста.
    fn cap_label_galley(
        &self,
        painter: &egui::Painter,
        label: &str,
        font_size: f32,
        center: egui::Pos2,
        alpha: f32,
    ) {
        let font_size = font_size * self.cfg.keycap_text_scale;
        let galley = painter.layout_no_wrap(
            label.to_string(),
            egui::FontId::proportional(font_size),
            egui::Color32::WHITE,
        );
        let gs = galley.size();
        let pos = egui::Pos2::new(
            center.x - gs.x / 2.0 + self.cfg.keycap_text_dx * self.cfg.scale,
            center.y - gs.y / 2.0 + self.cfg.keycap_text_dy * self.cfg.scale,
        );
        painter.galley(pos, galley, col(self.cfg.key_text, alpha));
    }

    fn draw_keys(
        &mut self,
        painter: &egui::Painter,
        screen: &egui::Rect,
        ctx: &egui::Context,
        now: Instant,
    ) {
        let life = self.cfg.key_duration;
        let ax = screen.min.x + screen.width() * self.cfg.pos_x;
        let ay = screen.min.y + screen.height() * self.cfg.pos_y;
        let font_size = 20.0 * self.cfg.scale;
        let pad = 10.0 * self.cfg.scale;
        let gap = 6.0 * self.cfg.scale;
        let ppp = ctx.pixels_per_point();

        // Новый комбо появляется внизу (у якоря), предыдущие уезжают вверх
        let mut y_bottom = ay;
        for i in (0..self.bubbles.len()).rev() {
            let b = &self.bubbles[i];
            let age = now.duration_since(b.born).as_secs_f32();
            let t = age / life;
            let alpha = crate::fx::bubble_alpha(t);
            // лёгкая «пружинка» при появлении
            let zoom = crate::fx::pop_zoom(age);

            // parts клонируем: оба draw_* берут &mut self (кеш текстур)
            let parts = b.parts.clone();
            if self.cfg.keycap_style {
                y_bottom = self.draw_keycap_row(
                    painter, ctx, &parts, ax, y_bottom, font_size, gap, alpha, zoom, ppp,
                );
            } else {
                y_bottom = self.draw_classic_bubble(
                    painter, ctx, &parts, ax, y_bottom, font_size, pad, gap, alpha, zoom, ppp,
                );
            }
        }
    }

    /// Классический режим. mouse_icons=false — чистый текст комбо на бабле
    /// (как раньше); mouse_icons=true — текст клавиш + SVG-иконки мыши/колеса
    /// на том же бабле. Иконки управляются независимо от режима кейкапов.
    #[allow(clippy::too_many_arguments)]
    fn draw_classic_bubble(
        &mut self,
        painter: &egui::Painter,
        ctx: &egui::Context,
        parts: &[Part],
        ax: f32,
        y_bottom: f32,
        font_size: f32,
        pad: f32,
        gap: f32,
        alpha: f32,
        zoom: f32,
        ppp: f32,
    ) -> f32 {
        if !self.cfg.mouse_icons {
            return self.draw_classic_text(
                painter, parts, ax, y_bottom, font_size, pad, gap, alpha, zoom,
            );
        }

        let icon_h = font_size * 1.15;
        let sep_gap = font_size * 0.38;

        // 1) Измеряем сегменты (текст клавиш / иконки мыши)
        let mut widths: Vec<f32> = Vec::new();
        let mut content_h = 0.0f32;
        for p in parts {
            match p {
                Part::Key(label) => {
                    let g = painter.layout_no_wrap(
                        label.clone(),
                        egui::FontId::proportional(font_size),
                        egui::Color32::WHITE,
                    );
                    content_h = content_h.max(g.size().y);
                    widths.push(g.size().x);
                }
                Part::Mouse(_) | Part::Wheel(_) => {
                    content_h = content_h.max(icon_h);
                    widths.push(icon_h * MOUSE_W / MOUSE_H);
                }
            }
        }
        let plus_w = {
            let g = painter.layout_no_wrap(
                "+".to_string(),
                egui::FontId::proportional(font_size * 0.8),
                egui::Color32::WHITE,
            );
            g.size().x
        };
        let n = widths.len();
        let total_gap = sep_gap * 2.0 * n.saturating_sub(1) as f32;
        let cw = widths.iter().sum::<f32>() + plus_w * n.saturating_sub(1) as f32 + total_gap;
        let bw = (cw + pad * 2.0) * zoom;
        let bh = (content_h + pad * 2.0) * zoom;
        let center = egui::Pos2::new(ax, y_bottom - bh / 2.0);

        painter.rect_filled(
            egui::Rect::from_center_size(center, egui::vec2(bw, bh)),
            egui::Rounding::same(self.cfg.key_radius * self.cfg.scale),
            col(self.cfg.key_bg, alpha),
        );

        // 2) Рисуем сегменты слева направо, вертикальный центр бабла
        let cy = center.y;
        let au8 = (alpha * 255.0) as u8;
        let fade = egui::Color32::from_rgba_premultiplied(au8, au8, au8, au8);
        let mut x = center.x - cw * zoom / 2.0;
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                let plus = painter.layout_no_wrap(
                    "+".to_string(),
                    egui::FontId::proportional(font_size * 0.8),
                    col(self.cfg.key_text, alpha),
                );
                let ps = plus.size();
                painter.galley(
                    egui::Pos2::new(x + sep_gap * zoom, cy - ps.y / 2.0),
                    plus,
                    col(self.cfg.key_text, alpha),
                );
                x += (sep_gap + plus_w + sep_gap) * zoom;
            }
            let w = widths[i] * zoom;
            match p {
                Part::Key(label) => {
                    let g = painter.layout_no_wrap(
                        label.clone(),
                        egui::FontId::proportional(font_size),
                        egui::Color32::WHITE,
                    );
                    painter.galley(
                        egui::Pos2::new(x, cy - g.size().y / 2.0),
                        g,
                        col(self.cfg.key_text, alpha),
                    );
                }
                Part::Mouse(button) => {
                    let icon = match *button {
                        0 => MouseIcon::Left,
                        1 => MouseIcon::Right,
                        _ => MouseIcon::Middle,
                    };
                    let rect = egui::Rect::from_min_size(
                        egui::Pos2::new(x, cy - icon_h * zoom / 2.0),
                        egui::vec2(w, icon_h * zoom),
                    );
                    let mut drew_icon = false;
                    if self.cfg.mouse_icons {
                        if let Some(tex) = self.mouse_icon_texture(ctx, icon, icon_h, ppp) {
                            painter.image(tex.id(), rect, UV, fade);
                            drew_icon = true;
                        }
                    }
                    if !drew_icon {
                        draw_label(
                            painter,
                            crate::keys::mouse_button_name(*button),
                            font_size,
                            rect.center(),
                            col(self.cfg.key_text, alpha),
                        );
                    }
                }
                Part::Wheel(up) => {
                    let icon = if *up {
                        MouseIcon::WheelUp
                    } else {
                        MouseIcon::WheelDown
                    };
                    let rect = egui::Rect::from_min_size(
                        egui::Pos2::new(x, cy - icon_h * zoom / 2.0),
                        egui::vec2(w, icon_h * zoom),
                    );
                    let mut drew_icon = false;
                    if self.cfg.mouse_icons {
                        if let Some(tex) = self.mouse_icon_texture(ctx, icon, icon_h, ppp) {
                            painter.image(tex.id(), rect, UV, fade);
                            drew_icon = true;
                        }
                    }
                    if !drew_icon {
                        draw_label(
                            painter,
                            crate::keys::wheel_name(*up),
                            font_size,
                            rect.center(),
                            col(self.cfg.key_text, alpha),
                        );
                    }
                }
            }
            x += w;
        }

        y_bottom - bh - gap
    }

    /// Чистый текст комбо на бабле (классика без иконок мыши).
    #[allow(clippy::too_many_arguments)]
    fn draw_classic_text(
        &self,
        painter: &egui::Painter,
        parts: &[Part],
        ax: f32,
        y_bottom: f32,
        font_size: f32,
        pad: f32,
        gap: f32,
        alpha: f32,
        zoom: f32,
    ) -> f32 {
        let text = combo_text(parts);
        let galley = painter.layout_no_wrap(
            text,
            egui::FontId::proportional(font_size),
            egui::Color32::WHITE,
        );
        let gsize = galley.size();
        let bw = (gsize.x + pad * 2.0) * zoom;
        let bh = (gsize.y + pad * 2.0) * zoom;
        let center = egui::Pos2::new(ax, y_bottom - bh / 2.0);
        let rect = egui::Rect::from_center_size(center, egui::vec2(bw, bh));

        painter.rect_filled(
            rect,
            egui::Rounding::same(self.cfg.key_radius * self.cfg.scale),
            col(self.cfg.key_bg, alpha),
        );
        let text_pos = egui::Pos2::new(center.x - gsize.x / 2.0, center.y - gsize.y / 2.0);
        painter.galley(text_pos, galley, col(self.cfg.key_text, alpha));

        y_bottom - bh - gap
    }

    /// Режим кейкапов: каждая часть — свой кейкап/иконка, между ними «+».
    #[allow(clippy::too_many_arguments)]
    fn draw_keycap_row(
        &mut self,
        painter: &egui::Painter,
        ctx: &egui::Context,
        parts: &[Part],
        ax: f32,
        y_bottom: f32,
        font_size: f32,
        gap: f32,
        alpha: f32,
        zoom: f32,
        ppp: f32,
    ) -> f32 {
        let cap_h = font_size * 1.75 * self.cfg.keycap_height;
        let icon_h = font_size * 1.6;
        let sep_gap = font_size * 0.38;

        // 1) Считаем размеры всех частей (в точках)
        let mut widths: Vec<f32> = Vec::new();
        for p in parts {
            match p {
                Part::Key(label) => {
                    let galley = painter.layout_no_wrap(
                        label.clone(),
                        egui::FontId::proportional(font_size * 0.92),
                        egui::Color32::WHITE,
                    );
                    // ширина кейкапа: подпись + поля, квантование 2pt (кеш текстур)
                    let w = (galley.size().x * 0.92 + font_size * 0.9 + 2.0) / 2.0 * 2.0;
                    widths.push(w.max(cap_h * 0.8));
                }
                Part::Mouse(_) | Part::Wheel(_) => {
                    if self.cfg.mouse_icons {
                        widths.push(icon_h * MOUSE_W / MOUSE_H);
                    } else {
                        // иконки выключены — текст вместо мыши/колеса
                        let label = match p {
                            Part::Mouse(btn) => crate::keys::mouse_button_name(*btn),
                            Part::Wheel(up) => crate::keys::wheel_name(*up),
                            _ => unreachable!(),
                        };
                        let g = painter.layout_no_wrap(
                            label.to_string(),
                            egui::FontId::proportional(font_size * 0.92),
                            egui::Color32::WHITE,
                        );
                        widths.push((g.size().x + font_size * 0.6 + 2.0) / 2.0 * 2.0);
                    }
                }
            }
        }
        let plus_w = {
            let g = painter.layout_no_wrap(
                "+".to_string(),
                egui::FontId::proportional(font_size * 0.8),
                egui::Color32::WHITE,
            );
            g.size().x
        };
        let n = widths.len();
        let total_gap = sep_gap * 2.0 * n.saturating_sub(1) as f32;
        let total_w =
            (widths.iter().sum::<f32>() + plus_w * n.saturating_sub(1) as f32 + total_gap) * zoom;
        let row_h = cap_h.max(icon_h) * zoom;

        // 2) Рисуем слева направо, центрируя по ax
        let mut x = ax - total_w / 2.0;
        let cy = y_bottom - row_h / 2.0;
        let au8 = (alpha * 255.0) as u8;
        let fade = egui::Color32::from_rgba_premultiplied(au8, au8, au8, au8);
        for (i, p) in parts.iter().enumerate() {
            if i > 0 {
                // разделитель «+»
                let plus = painter.layout_no_wrap(
                    "+".to_string(),
                    egui::FontId::proportional(font_size * 0.8),
                    col(self.cfg.key_text, alpha),
                );
                let ps = plus.size();
                painter.galley(
                    egui::Pos2::new(x + sep_gap * zoom, cy - ps.y / 2.0),
                    plus,
                    col(self.cfg.key_text, alpha),
                );
                x += (sep_gap + plus_w + sep_gap) * zoom;
            }
            let w = widths[i] * zoom;
            match p {
                Part::Key(label) => {
                    let rect = egui::Rect::from_min_size(
                        egui::Pos2::new(x, cy - cap_h * zoom / 2.0),
                        egui::vec2(w, cap_h * zoom),
                    );
                    // Сначала кастомная картинка клавиши, затем сгенерированный кейкап
                    let tex = self
                        .custom_cap_texture(ctx, label, widths[i], cap_h, ppp)
                        .or_else(|| self.keycap_texture(ctx, widths[i], cap_h, ppp));
                    match tex {
                        Some(tex) => {
                            painter.image(tex.id(), rect, UV, fade);
                        }
                        None => {
                            // фолбэк, пока SVG растеризуется: обычный фон бабла
                            painter.rect_filled(
                                rect,
                                egui::Rounding::same(6.0),
                                col(self.cfg.key_bg, alpha),
                            );
                        }
                    }
                    self.cap_label_galley(painter, label, font_size * 0.92, rect.center(), alpha);
                }
                Part::Mouse(button) => {
                    let icon = match *button {
                        0 => MouseIcon::Left,
                        1 => MouseIcon::Right,
                        _ => MouseIcon::Middle,
                    };
                    let rect = egui::Rect::from_min_size(
                        egui::Pos2::new(x, cy - icon_h * zoom / 2.0),
                        egui::vec2(w, icon_h * zoom),
                    );
                    let mut drew_icon = false;
                    if self.cfg.mouse_icons {
                        if let Some(tex) = self.mouse_icon_texture(ctx, icon, icon_h, ppp) {
                            painter.image(tex.id(), rect, UV, fade);
                            drew_icon = true;
                        }
                    }
                    if !drew_icon {
                        draw_label(
                            painter,
                            crate::keys::mouse_button_name(*button),
                            font_size,
                            rect.center(),
                            col(self.cfg.key_text, alpha),
                        );
                    }
                }
                Part::Wheel(up) => {
                    let icon = if *up {
                        MouseIcon::WheelUp
                    } else {
                        MouseIcon::WheelDown
                    };
                    // под стрелку оставляем чуть больше высоты
                    let wh = icon_h * 1.25;
                    let rect = egui::Rect::from_min_size(
                        egui::Pos2::new(x, cy - wh * zoom / 2.0),
                        egui::vec2(w, wh * zoom),
                    );
                    let mut drew_icon = false;
                    if self.cfg.mouse_icons {
                        if let Some(tex) = self.mouse_icon_texture(ctx, icon, icon_h, ppp) {
                            painter.image(tex.id(), rect, UV, fade);
                            drew_icon = true;
                        }
                    }
                    if !drew_icon {
                        draw_label(
                            painter,
                            crate::keys::wheel_name(*up),
                            font_size,
                            rect.center(),
                            col(self.cfg.key_text, alpha),
                        );
                    }
                }
            }
            x += w;
        }

        y_bottom - row_h - gap
    }

    fn draw_paused_hint(&self, painter: &egui::Painter, screen: &egui::Rect) {
        let ax = screen.min.x + screen.width() * self.cfg.pos_x;
        let ay = screen.min.y + screen.height() * self.cfg.pos_y;
        painter.text(
            egui::Pos2::new(ax, ay + 20.0 * self.cfg.scale),
            egui::Align2::CENTER_TOP,
            "Пауза — Ctrl+Alt+K",
            egui::FontId::proportional(13.0 * self.cfg.scale),
            col(self.cfg.key_text, 0.4),
        );
    }
}

impl eframe::App for OverlayApp {
    fn clear_color(&self, _visuals: &egui::Visuals) -> [f32; 4] {
        [0.0, 0.0, 0.0, 0.0] // полностью прозрачный фон
    }

    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if !self.started {
            self.started = true;
            // Разворачиваемся на весь экран (borderless), чтобы виджет и эффекты
            // работали в любой точке монитора.
            ctx.send_viewport_cmd(egui::ViewportCommand::Fullscreen(true));
        }
        // Каждый кадр: клик-тру + не красть фокус (дёшево, переживает fullscreen-переходы)
        apply_overlay_styles();
        // Каждый кадр: диагностика z-порядка (активна только при KEYPRESS_DEBUG=1)
        crate::zorder::debug_tick();

        self.drain_events(ctx);
        self.reload_config_if_needed();

        let now = Instant::now();
        let ppp = ctx.pixels_per_point();
        let screen = ctx.screen_rect();
        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Foreground,
            egui::Id::new("fx"),
        ));

        self.ripples
            .retain(|r| now.duration_since(r.born).as_secs_f32() < CLICK_LIFE);
        self.scrolls
            .retain(|s| now.duration_since(s.born).as_secs_f32() < SCROLL_LIFE);
        let life = self.cfg.key_duration;
        self.bubbles
            .retain(|b| now.duration_since(b.born).as_secs_f32() < life);

        if !self.paused {
            if self.cfg.show_clicks {
                self.draw_clicks(&painter, ppp, now);
            }
            if self.cfg.show_scroll {
                self.draw_scroll(&painter, ppp, now);
            }
            if self.cfg.show_keys && !self.bubbles.is_empty() {
                self.draw_keys(&painter, &screen, ctx, now);
            }
        } else {
            self.draw_paused_hint(&painter, &screen);
        }

        let busy = !self.bubbles.is_empty() || !self.ripples.is_empty() || !self.scrolls.is_empty();
        if busy {
            ctx.request_repaint_after(Duration::from_millis(16));
        } else {
            ctx.request_repaint_after(Duration::from_millis(200));
        }
    }
}

pub fn run() {
    let cfg = load();
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(crate::winutil::OVERLAY_TITLE)
            .with_decorations(false)
            .with_transparent(true)
            .with_window_level(egui::WindowLevel::AlwaysOnTop)
            // окно не должно забирать фокус при создании
            .with_active(false)
            // КЛИК-ТРУ: egui-winit сам вызывает winit::set_cursor_hittest(false),
            // тот ставит IGNORE_CURSOR_EVENT -> WS_EX_LAYERED | WS_EX_TRANSPARENT
            // через собственное состояние окна (не сбрасывается при fullscreen).
            // Без этого оверлей перехватывал все клики экрана.
            .with_mouse_passthrough(true)
            .with_inner_size([1024.0, 600.0]),
        ..Default::default()
    };
    let _ = eframe::run_native(
        crate::winutil::OVERLAY_TITLE,
        options,
        Box::new(|cc| Ok(Box::new(OverlayApp::new(cc, cfg)))),
    );
    // Аккуратно убрать иконку трея (иначе останется «призрак» до наведения
    // мыши): поток трея удаляет иконку в WM_DESTROY.
    crate::tray::shutdown();
}
