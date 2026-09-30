//! Минимальная панель настроек: размер, позиция, формы, цвета.
//! Правит keyviz-lite.toml, оверлей подхватывает изменения на лету.

use crate::config::{ClickShape, Config};
use crate::winutil;
use eframe::egui;
use std::time::{Duration, Instant};

pub struct SettingsApp {
    cfg: Config,
    overlay_running: bool,
    last_status_check: Option<Instant>,
}

fn to_edit(c: [f32; 4]) -> egui::Color32 {
    let f = |v: f32| ((v.clamp(0.0, 1.0)) * 255.0) as u8;
    egui::Color32::from_rgba_unmultiplied(f(c[0]), f(c[1]), f(c[2]), f(c[3]))
}

fn from_edit(c: egui::Color32) -> [f32; 4] {
    [
        c.r() as f32 / 255.0,
        c.g() as f32 / 255.0,
        c.b() as f32 / 255.0,
        c.a() as f32 / 255.0,
    ]
}

fn color_row(ui: &mut egui::Ui, label: &str, c: &mut [f32; 4]) -> bool {
    let mut col = to_edit(*c);
    let before = col;
    ui.horizontal(|ui| {
        ui.label(label);
        ui.color_edit_button_srgba(&mut col);
    });
    let changed = col != before;
    *c = from_edit(col);
    changed
}

impl SettingsApp {
    pub fn new() -> Self {
        Self {
            cfg: crate::config::load(),
            overlay_running: false,
            last_status_check: None,
        }
    }
}

impl eframe::App for SettingsApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        if self
            .last_status_check
            .map_or(true, |t| t.elapsed() > Duration::from_secs(1))
        {
            self.last_status_check = Some(Instant::now());
            self.overlay_running = winutil::overlay_running();
        }

        let mut changed = false;

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Keystro-lite — настройки");
            ui.add_space(4.0);
            ui.label(format!(
                "Оверлей: {}",
                if self.overlay_running {
                    "запущен"
                } else {
                    "не запущен"
                }
            ));
            ui.separator();

            ui.group(|ui| {
                ui.strong("Виджет клавиш");
                changed |= ui
                    .add(egui::Slider::new(&mut self.cfg.scale, 0.5..=3.0).text("Размер"))
                    .changed();
                let mut px = self.cfg.pos_x * 100.0;
                if ui
                    .add(egui::Slider::new(&mut px, 0.0..=100.0).text("Позиция X, %"))
                    .changed()
                {
                    self.cfg.pos_x = px / 100.0;
                    changed = true;
                }
                let mut py = self.cfg.pos_y * 100.0;
                if ui
                    .add(egui::Slider::new(&mut py, 0.0..=100.0).text("Позиция Y, %"))
                    .changed()
                {
                    self.cfg.pos_y = py / 100.0;
                    changed = true;
                }
                changed |= ui
                    .add(
                        egui::Slider::new(&mut self.cfg.key_duration, 0.5..=5.0)
                            .text("Время показа, сек"),
                    )
                    .changed();
                changed |= ui
                    .add(
                        egui::Slider::new(&mut self.cfg.key_radius, 0.0..=40.0)
                            .text("Скругление клавиш"),
                    )
                    .changed();
                changed |= ui.checkbox(&mut self.cfg.show_keys, "Показывать клавиши").changed();
            });

            ui.add_space(6.0);

            ui.group(|ui| {
                ui.strong("Клики мыши");
                changed |=
                    ui.checkbox(&mut self.cfg.show_clicks, "Показывать клики").changed();
                ui.label("Форма индикатора:");
                ui.horizontal(|ui| {
                    changed |= ui
                        .radio_value(&mut self.cfg.click_shape, ClickShape::Circle, "Круг")
                        .changed();
                    changed |= ui
                        .radio_value(&mut self.cfg.click_shape, ClickShape::Ring, "Кольцо")
                        .changed();
                    changed |= ui
                        .radio_value(&mut self.cfg.click_shape, ClickShape::Square, "Квадрат")
                        .changed();
                });
                changed |= color_row(ui, "Цвет кликов:", &mut self.cfg.click_color);
            });

            ui.add_space(6.0);

            ui.group(|ui| {
                ui.strong("Прокрутка колесиком");
                changed |= ui
                    .checkbox(&mut self.cfg.show_scroll, "Показывать прокрутку")
                    .changed();
                changed |= color_row(ui, "Цвет прокрутки:", &mut self.cfg.scroll_color);
            });

            ui.add_space(6.0);

            ui.group(|ui| {
                ui.strong("Цвета клавиш");
                changed |= color_row(ui, "Фон клавиш:", &mut self.cfg.key_bg);
                changed |= color_row(ui, "Текст клавиш:", &mut self.cfg.key_text);
            });

            ui.separator();

            ui.horizontal(|ui| {
                if ui.button("Запустить оверлей").clicked() {
                    let exe = std::env::current_exe().unwrap_or_default();
                    let _ = std::process::Command::new(exe).spawn();
                }
                if ui.button("Остановить оверлей").clicked() {
                    winutil::stop_overlay();
                }
            });

            ui.add_space(8.0);
            ui.label("Горячие клавиши: Ctrl+Alt+K — пауза/показ, Ctrl+Alt+Q — выход");
            ui.label(
                "Настройки сохраняются автоматически в keyviz-lite.toml рядом с программой \
                 и применяются на лету.",
            );
        });

        if changed {
            crate::config::save(&self.cfg);
        }
    }
}

pub fn run() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("Keystro-lite — настройки")
            .with_inner_size([470.0, 720.0]),
        ..Default::default()
    };
    let _ = eframe::run_native(
        "Keystro-lite — настройки",
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(SettingsApp::new()))
        }),
    );
}
