//! Оверлей: прозрачное, always-on-top, click-through окно на весь экран.
//! Рисует клавиши, клики и прокрутку. Ничего не перехватывает, только показывает.

use crate::config::{config_path, load, ClickShape, Config};
use crate::hooks::spawn_hooks;
use crate::input::UiEvent;
use crate::winutil::apply_overlay_styles;
use crossbeam_channel::Receiver;
use eframe::egui;
use std::time::{Duration, Instant};

const CLICK_LIFE: f32 = 0.55;
const SCROLL_LIFE: f32 = 0.5;

struct Bubble {
    text: String,
    born: Instant,
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
    bubbles: Vec<Bubble>,
    ripples: Vec<Ripple>,
    scrolls: Vec<ScrollFx>,
    paused: bool,
    started: bool,
    cfg_last_check: Instant,
    cfg_mtime: Option<std::time::SystemTime>,
}

/// Прямой альфа-канал (как в конфиге) -> premultiplied (как любит egui).
/// Конвертация считается в fx::premultiply (покрыта тестами).
fn col(c: [f32; 4], mul: f32) -> egui::Color32 {
    let [r, g, b, a] = crate::fx::premultiply(c, mul);
    egui::Color32::from_rgba_premultiplied(r, g, b, a)
}

impl OverlayApp {
    pub fn new(cfg: Config) -> Self {
        crate::hooks::set_show_cyrillic(cfg.show_cyrillic);
        let (tx, rx) = crossbeam_channel::unbounded();
        spawn_hooks(tx);
        Self {
            cfg,
            rx,
            bubbles: Vec::new(),
            ripples: Vec::new(),
            scrolls: Vec::new(),
            paused: false,
            started: false,
            cfg_last_check: Instant::now(),
            cfg_mtime: None,
        }
    }

    fn drain_events(&mut self, ctx: &egui::Context) {
        while let Ok(ev) = self.rx.try_recv() {
            match ev {
                UiEvent::Keys { text } => {
                    if self.paused || !self.cfg.show_keys {
                        continue;
                    }
                    self.bubbles.push(Bubble {
                        text,
                        born: Instant::now(),
                    });
                    // Количество строк в стопке настраивается (cfg.max_keys)
                    if self.bubbles.len() > self.cfg.max_keys {
                        self.bubbles.remove(0);
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
                }
                UiEvent::Quit => {
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

    fn draw_keys(&self, painter: &egui::Painter, screen: &egui::Rect, now: Instant) {
        let life = self.cfg.key_duration;
        let ax = screen.min.x + screen.width() * self.cfg.pos_x;
        let ay = screen.min.y + screen.height() * self.cfg.pos_y;
        let font_size = 20.0 * self.cfg.scale;
        let pad = 10.0 * self.cfg.scale;
        let gap = 6.0 * self.cfg.scale;

        // Новый комбо появляется внизу (у якоря), предыдущие уезжают вверх
        let mut y_bottom = ay;
        for b in self.bubbles.iter().rev() {
            let age = now.duration_since(b.born).as_secs_f32();
            let t = age / life;
            let alpha = crate::fx::bubble_alpha(t);
            // лёгкая «пружинка» при появлении
            let zoom = crate::fx::pop_zoom(age);

            let galley = painter.layout_no_wrap(
                b.text.clone(),
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

            y_bottom -= bh + gap;
        }
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
                self.draw_keys(&painter, &screen, now);
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
        Box::new(|_cc| Ok(Box::new(OverlayApp::new(cfg)))),
    );
}
