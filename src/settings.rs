//! Минимальная панель настроек: размер, позиция, формы, цвета.
//! Правит keypress.toml, оверлей подхватывает изменения на лету.

use crate::config::{ClickShape, Config};
use crate::winutil;
use eframe::egui;
use std::time::{Duration, Instant};

pub struct SettingsApp {
    cfg: Config,
    overlay_running: bool,
    last_status_check: Option<Instant>,
    /// Буферы добавления по-клавишной картинки кейкапа
    cap_key: String,
    cap_path: String,
}

/// Цвет из конфига: пикер для НЕПРОЗРАЧНОГО RGB + отдельный слайдер альфы.
///
/// РЕГРЕССИЯ (v0.2): раньше цвет целиком уходил в ui.color_edit_button_srgba
/// с конверсией unmultiplied<->premultiplied каждый кадр — для полупрозрачных
/// цветов RGB умножался на альфу и цвет «съезжал в чёрный». Теперь RGB
/// редактируется color_edit_button_srgb (без альфы — без потерь), а альфа —
/// отдельным слайдером; конверсии — из color.rs, покрыты тестом дрейфа.
fn color_row(ui: &mut egui::Ui, label: &str, c: &mut [f32; 4]) -> bool {
    let (mut rgb, mut alpha) = crate::color::split(*c);
    let before = (rgb, alpha);
    let mut edited = false;
    ui.horizontal(|ui| {
        ui.label(label);
        edited |= ui.color_edit_button_srgb(&mut rgb).changed();
        edited |= ui
            .add(egui::Slider::new(&mut alpha, 0..=255).text("Непрозрачность"))
            .changed();
    });
    let changed = (rgb, alpha) != before;
    if changed {
        *c = crate::color::join(rgb, alpha);
    }
    changed || edited
}

impl SettingsApp {
    pub fn new() -> Self {
        Self {
            cfg: crate::config::load(),
            overlay_running: false,
            last_status_check: None,
            cap_key: String::new(),
            cap_path: String::new(),
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
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.heading("Keypress — настройки");
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
                    changed |= ui
                        .checkbox(&mut self.cfg.show_keys, "Показывать клавиши")
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut self.cfg.max_keys, 1..=10)
                                .text("Строк в виджете"),
                        )
                        .changed();
                    changed |= ui
                        .checkbox(
                            &mut self.cfg.show_cyrillic,
                            "Кириллица (символы русской раскладки)",
                        )
                        .changed();
                    changed |= ui
                        .checkbox(
                            &mut self.cfg.keycap_style,
                            "Кейкапы (вид сверху) — иначе текст",
                        )
                        .changed();
                });

                ui.add_space(6.0);

                ui.group(|ui| {
                    ui.strong("Мышь (SVG-иконка)");
                    ui.label("Работает независимо от режима кейкапов.");
                    changed |= ui
                        .checkbox(
                            &mut self.cfg.mouse_icons,
                            "Мышь и колесо — SVG-иконкой (иначе текст)",
                        )
                        .changed();
                    changed |= color_row(ui, "Корпус мыши:", &mut self.cfg.mouse_body);
                    changed |= color_row(ui, "Контур мыши:", &mut self.cfg.mouse_outline);
                    ui.label("Подсветка нажатой кнопки/колеса — цвета кликов и прокрутки.");
                });

                ui.add_space(6.0);

                ui.group(|ui| {
                    ui.strong("Кейкапы (режим — галочка «Кейкапы» выше)");
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut self.cfg.keycap_height, 0.5..=2.5)
                                .text("Высота кейкапа"),
                        )
                        .changed();

                    ui.add_space(4.0);
                    ui.strong("Картинка-кейкап (PNG/JPG/GIF/BMP/ICO/SVG)");
                    ui.horizontal(|ui| {
                        if ui.button("Сбросить").clicked() {
                            self.cfg.keycap_image.clear();
                            changed = true;
                        }
                        ui.label("пусто — стандартный кейкап");
                    });
                    changed |= ui
                        .text_edit_singleline(&mut self.cfg.keycap_image)
                        .changed();
                    ui.label(
                        "Путь к файлу; относительный — от папки с keypress.toml. \
                         Картинка растягивается на размер кейкапа.",
                    );

                    ui.add_space(4.0);
                    ui.strong("Картинки по клавишам (перекрывают общую)");
                    if !self.cfg.keycap_images.is_empty() {
                        let keys: Vec<String> = self.cfg.keycap_images.keys().cloned().collect();
                        for k in keys {
                            ui.horizontal(|ui| {
                                ui.monospace(&k);
                                ui.label("=");
                                if let Some(v) = self.cfg.keycap_images.get_mut(&k) {
                                    changed |= ui.text_edit_singleline(v).changed();
                                }
                                if ui.small_button("Удалить").clicked() {
                                    self.cfg.keycap_images.remove(&k);
                                    changed = true;
                                }
                            });
                        }
                    }
                    ui.horizontal(|ui| {
                        ui.label("Клавиша:");
                        ui.text_edit_singleline(&mut self.cap_key);
                        ui.label("Файл:");
                        ui.text_edit_singleline(&mut self.cap_path);
                        if ui.button("+").clicked()
                            && !self.cap_key.trim().is_empty()
                            && !self.cap_path.trim().is_empty()
                        {
                            self.cfg.keycap_images.insert(
                                self.cap_key.trim().to_string(),
                                self.cap_path.trim().to_string(),
                            );
                            self.cap_key.clear();
                            self.cap_path.clear();
                            changed = true;
                        }
                    });
                    ui.label(
                        "Имя клавиши — как в виджете («A», «Ctrl», «Пробел»; \
                         регистр не важен).",
                    );

                    ui.add_space(6.0);
                    ui.strong("Текст на кейкапе");
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut self.cfg.keycap_text_scale, 0.3..=3.0)
                                .text("Размер текста"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut self.cfg.keycap_text_dx, -50.0..=50.0)
                                .text("Сдвиг текста X"),
                        )
                        .changed();
                    changed |= ui
                        .add(
                            egui::Slider::new(&mut self.cfg.keycap_text_dy, -50.0..=50.0)
                                .text("Сдвиг текста Y"),
                        )
                        .changed();
                    ui.label("Сдвиги в точках: X вправо, Y вниз (от центра кейкапа).");
                });

                ui.add_space(6.0);

                ui.group(|ui| {
                    ui.strong("Клики мыши");
                    changed |= ui
                        .checkbox(&mut self.cfg.show_clicks, "Показывать клики")
                        .changed();
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
                    "Настройки сохраняются автоматически в keypress.toml рядом с программой \
                 и применяются на лету.",
                );
                ui.add_space(4.0);
                ui.label(format!("Keypress v{}", env!("CARGO_PKG_VERSION")));
            });
        });

        if changed {
            crate::config::save(&self.cfg);
        }
    }
}

pub fn run() {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title(crate::winutil::SETTINGS_TITLE)
            .with_inner_size([470.0, 720.0]),
        ..Default::default()
    };
    let _ = eframe::run_native(
        crate::winutil::SETTINGS_TITLE,
        options,
        Box::new(|cc| {
            cc.egui_ctx.set_visuals(egui::Visuals::dark());
            Ok(Box::new(SettingsApp::new()))
        }),
    );
}
