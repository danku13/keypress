# agents.md — контекст проекта keypress

Этот файл — вводные для AI-агентов и новых разработчиков. Читай перед любыми изменениями.

## Задача

Минималистичный аналог Keystro (https://keystro.app/) для личного использования:
показывает на экране нажатия клавиш, клики мыши и прокрутку колесиком —
для туториалов, презентаций и записи видео.

## Вводные от владельца (зафиксированы, не менять без его явного запроса)

- **Только Windows 10/11 (64-бит)** — кроссплатформенность не нужна
- **Исключительно личное использование** — лицензионная чистота и подпись кода не приоритет
- **Один стиль** — системы тем/скинов в MVP нет
- **Минимум настроек, только эти**: размер, положение на экране (X/Y в %), формы (круг/кольцо/квадрат для кликов), цвета (фон клавиш, текст, клики, скролл)
- **Функциональный минимум**: клавиши + клики + скролл, ничего больше
- **Главный критерий архитектуры — скорость реализации**, а не расширяемость

## Технологический стек (зафиксирован)

| Компонент | Решение |
|---|---|
| Язык | Rust (edition 2021, stable) |
| UI/рендер | eframe 0.28 + egui (wgpu, default features) |
| Win API | windows-sys 0.52 (нативные low-level хуки, не rdev) |
| Потоки | std::thread + crossbeam-channel |
| Конфиг | serde + toml → `keypress.toml` рядом с exe |
| Трей/иконка | своя реализация (tray.rs): Shell_NotifyIconW + message-only окно, v0.5 |
| Лицензия | MIT (LICENSE); license/description/repository в Cargo.toml, v0.5.1 |
| CI | GitHub Actions (.github/workflows/build.yml): fmt+тесты на ubuntu, сборки windows-msvc/linux/macos, артефакты запуска, release по тегам v* |

## Архитектурные решения и почему

1. **Один бинарник, два режима**: без аргументов — оверлей + авто-открытие
   панели настроек (если ещё не открыта), `--settings` — только панель.
   Два eframe-приложения в одном процессе неудобно (multi-viewport сыроват),
   два процесса + IPC через файл — самое простое и надежное.
   Single-instance: повторный запуск exe не создает второй оверлей,
   а просто открывает панель (FindWindowW по заголовкам).
2. **Нативные хуки, а не rdev**: `KBDLLHOOKSTRUCT` дает vkCode, `MSLLHOOKSTRUCT` —
   точные координаты клика/скролла (rdev координаты в событии не отдает).
3. **Хуки в отдельном потоке**, в UI уходят готовые `UiEvent` через канал.
   Хук-проц только наблюдает: `CallNextHookEx` всегда, ввод никогда не глотается,
   инжектированный ввод (`LLKHF_INJECTED`/`LLMHF_INJECTED`) игнорируется.
4. **Оверлей**: fullscreen borderless + transparent + always-on-top.
   **Клик-тру — обязательная пара `WS_EX_LAYERED | WS_EX_TRANSPARENT`**:
   - главный механизм — `ViewportBuilder::with_mouse_passthrough(true)`,
     egui-winit вызывает winit `set_cursor_hittest(false)` → флаг
     IGNORE_CURSOR_EVENT, и winit сам применяет и держит пару LAYERED+TRANSPARENT;
   - подстраховка — каждый кадр `apply_overlay_styles()` OR-ит те же биты
     (`WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW`).
   **Баг v0.1**: ставился один `WS_EX_TRANSPARENT` без LAYERED — по Win32
   это не влияет на hit-test, фуллскрин-оверлей глотал все клики экрана
   (Пуск/таскбар нажимались только через выбор приложения Alt+Tab'ом).
   **Баг v0.2**: слоёное окно (WS_EX_LAYERED) не отображается, пока не
   вызваны SetLayeredWindowAttributes/UpdateLayeredWindow, а winit 0.29.15
   этого не делает — после включения клик-тру оверлей стал целиком
   невидимым. Фикс: `apply_overlay_styles()` каждый кадр также вызывает
   `SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA)`.
5. **IPC = файл конфига**: панель настроек пишет `keypress.toml`,
   оверлей проверяет mtime каждые 300 мс и перечитывает. Конфиг при загрузке
   прогоняется через `normalized()` (клампинг в диапазоны слайдеров — защита
   от ручной правки toml).
6. **Агрегация комбо**: `keys::KeyAggregator` (чистый, тестируемый):
   модификаторы в held-set; «Ctrl + C» собирается при нажатии обычной клавиши;
   одиночный модификатор показывается через 280 мс (WM_TIMER), если за это
   время не пришла обычная клавиша; если зажато несколько модификаторов —
   таймаут показывает весь комбо («Ctrl + Shift»). Комбо возвращается
   структурой `Vec<Part>` (Key/Mouse/Wheel), а не строкой — виджет рисует
   мышь/колесо SVG-иконками (icons.rs), текст собирается `combo_text`.
   Прокрутка: `WheelEv::New/Extend` — долгий скролл продлевает строку
   (WheelPulse), а не плодит новые.
7. **Цвета хранятся как `[f32; 4]` прямой альфы**, при отрисовке конвертируются
   в premultiplied (`fx::premultiply` + `Color32::from_rgba_premultiplied`) —
   egui так и ждет. ЛОВУШКА (v0.3): egui::Color32 — PREMULTIPLIED; передача
   пикеру unmultiplied + чтение обратно как прямого умножает RGB на альфу
   каждый кадр — цвет «съезжает в чёрный». Фикс: color.rs (split/join),
   пикер — непрозрачный RGB (color_edit_button_srgb) + слайдер альфы.
8. **SVG-иконки** (icons.rs, чистый модуль с тестами): мышь по мотивам
   freesvg.org left-click-right-click (ЛКМ/ПКМ/СКМ/колесо↑/колесо↓) и
   кейкапы «вид сверху» (включаемый режим cfg.keycap_style, дефолт —
   классический текст на бабле). SVG-строки с цветами из конфига уходят в
   egui_extras (svg) через bytes://*.svg (ctx.include_bytes) и растеризуются
   SizeHint::Size в физических пикселях (чёткость при любом ppp); кеш текстур
   по URI. Фолбэк — текст, пока текстура не готова.
9. **Колесо и инжекции**: скролл тачпада/драйверов приходит с LLMHF_INJECTED
   и раньше глушился — прокрутка не показывалась. Теперь wheel-ветка хука
   обрабатывается ДО фильтра инжекций и режет только LOWER_IL
   (input::wheel_rejected). Кнопки по-прежнему фильтруют LLMHF_INJECTED.
10. **TDD/тесты**: вся чистая логика вынесена в кроссплатформенные модули
   `keys.rs`, `input.rs`, `fx.rs`, `config.rs`, `color.rs`, `icons.rs` +
   диспетчер в `main.rs`. `cargo test` (94 теста) выполняется прямо на
   Linux/CI без Windows; GUI-клей (hooks/winutil/overlay/settings) под
   `#[cfg(windows)]` и `eframe`/`egui_extras`/`crossbeam`/`image` в
   `[target.'cfg(windows)'.dependencies]`, проверяется
   `cargo check --target x86_64-pc-windows-gnu`.
   Разработка велась строго RED → GREEN: сначала падающие тесты, потом код.
11. **Пуск на Win11 (v0.4)**: переутверждение HWND_TOPMOST каждый кадр
   работает внутри topmost-полосы, но CoreWindow Пуска Windows может держать
   в защищённой shell-полосе ВЫШЕ любых обычных topmost-окон. Решение —
   UIAccess-сборка: build.rs при `KEYPRESS_UIACCESS=1` вшивает
   assets/keypress-uiaccess.manifest (uiAccess="true") через winresource;
   окна процесса попадают в полосу UIAccess выше полосы оболочки (как
   osk.exe). Windows запускает uiAccess-exe ТОЛЬКО подписанным и из
   защищённой папки — это делает make-uiaccess.ps1 (самодписанный
   сертификат → LocalMachine Root+TrustedPublisher → Set-AuthenticodeSignature
   → Program Files). Обычная сборка манифест не вшивает вообще.
12. **Конфиг рядом с exe, но не всегда (v0.4)**: config_path() кешируется;
   порядок — KEYPRESS_CONFIG (env), папка exe если доступна на запись
   (пробная запись), иначе %APPDATA%\keypress (uiAccess-установка в
   Program Files — панель настроек должна сохранять без админа).
13. **SVG-мышь отделена от кейкапов (v0.4)**: cfg.mouse_icons — независимая
   галочка (иконки работают и в классике — смешанный бабл текст+иконки,
   и в кейкапах); cfg.mouse_body/mouse_outline — свои цвета корпуса/контура
   (дефолт = цвета клавиш, вид как в v0.3).
14. **Кастомные кейкапы (v0.4)**: cfg.keycap_image (общая) + cfg.keycap_images
   (BTreeMap по-клавишных; приоритет: точное имя → регистронезависимое →
   общая). Raster (PNG/JPG/GIF/BMP/ICO) декодируется крейтом image напрямую
   в ColorImage (egui_extras ImageLoader НЕ включен); SVG — через svg-лоадер,
   который по исходникам требует URI с расширением .svg. Ключ кеша текстур —
   FNV-1a(путь+mtime+размер)+растр: замену файла подхватывает без перезапуска.
   Подпись рисует egui поверх картинки: keycap_text_scale/dx/dy (dx/dy в
   точках, умножаются на cfg.scale); высота кейкапа — keycap_height.
15. **Системный трей (v0.5)**: tray.rs — отдельный поток с message-only
   окном (HWND_MESSAGE), Shell_NotifyIconW шлёт события мыши в WndProc; тот
   конвертирует их в TrayAction (чистый action_for_menu_id — тестируемый) и
   отправляет в ОБЩИЙ с хуками crossbeam-канал как UiEvent::TogglePause/Quit —
   никаких новых каналов/глобальных состояний. Левый клик — пауза, двойной —
   настройки (spawn_panel_if_needed), правый — меню (галочка паузы берётся из
   static PAUSED). Иконка — код (32x32 BGRA SDF-рисование кейкапа с K →
   CreateDIBSection → CreateIconIndirect). Устойчивость: TaskbarCreated
   (RegisterWindowMessage) возвращает иконку после перезапуска explorer,
   ретраи NIM_ADD при старте. Выход: shutdown() после eframe шлёт WM_CLOSE
   в окно трея → WM_DESTROY удаляет иконку (без «призраков»). Включение/выключение
   на лету: cfg.tray_icon + OverlayApp::sync_tray() после перечитывания конфига.
   ВАЖНО: трей живёт в ПРОЦЕССЕ ОВЕРЛЕЯ (панель настроек — отдельный короткоживущий
   процесс, иконка там не нужна). UiEvent::Quit («Выход»/Ctrl+Alt+Q): оверлей
   сначала шлёт WM_CLOSE окну панели (winutil::stop_settings — иначе панель,
   как отдельный процесс, оставалась висеть), затем закрывается сам.
16. **Лицензия + CI (v0.5.1)**: лицензия MIT (LICENSE). GitHub Actions
   (.github/workflows/build.yml): job test (fmt --check + cargo test, ubuntu —
   чистые модули тестируются везде) → матрица build: windows-latest
   (x86_64-pc-windows-msvc — канонический exe), ubuntu (x86_64-gnu) и
   macos-latest (aarch64) — там собирается только заглушка (eframe под
   cfg(windows), тянутся serde+toml). Артефакты через upload-artifact;
   пуш тега v* — softprops/action-gh-release (permissions contents: write).
   fail-fast: false — падение одной платформы не отменяет остальные.
   build.rs в CI безопасен: без KEYPRESS_UIACCESS=1 ранний выход.
17. **README-реворк по best-practice (v0.5.1)**: структура по паттернам топовых
   MIT-проектов (React/VS Code/Tauri/bat/fd): hero-превью в шапке (docs/preview.png,
   мок-рендер; место под реальный GIF — закомментированный блок), download-first
   (Быстрый старт через releases/latest + SmartScreen-примечание), 7 коротких
   фич с жирными ключами, полная таблица keypress.toml (ключ/дефолт/описание
   — из config.rs), таблица горячих клавиш, оглавление с якорями, таблица
   альтернатив (keyviz GPL-3.0 / Carnac MS-PL архив / KeyCastr BSD-3 / Keystro
   проприетарная — лицензии сверены по страницам GitHub), «Как это устроено»
   → аннотация + ARCHITECTURE.md, вдохновение/лицензия в конце.

## Ключевые файлы

```
src/main.rs     — диспетчер режимов (decide_mode тестируем) + автоспавн панели + тест UIAccess-манифеста
src/config.rs   — Config (вкл. mouse_icons/mouse_body/mouse_outline, keycap_height/keycap_image(s)/keycap_text_*), load/save toml, normalized(), config_path (env→exe-dir→APPDATA), fnv1a, keycap_image_for — тесты
src/input.rs    — UiEvent (Keys{parts}/WheelPulse) + хелперы: инжекции, кнопки, колесо
src/keys.rs     — VK-имена + Part/WheelEv + KeyAggregator (комбо, мышь, колесо)
src/icons.rs    — SVG-шаблоны: мышь (ЛКМ/ПКМ/СКМ/колесо) и кейкапы — чистый, тесты
src/color.rs    — конверсии цвета конфиг<->пикер/SVG (анти-дрейф в чёрный) — тесты
src/fx.rs       — чистая математика эффектов: premultiply, альфа, зум, радиусы
src/hooks.rs    — поток хуков: Win API-клей над keys/input, хоткеи
src/overlay.rs  — окно-оверлей, рендер эффектов (math берет из fx); классика текст/смешанная (текст+SVG-иконки), кейкапы с кастом-картинками и позиционированием текста
src/settings.rs — панель настроек: группы виджет/мышь/кейкапы/клики/скролл/цвета/трей, версия в подвале
src/tray.rs    — иконка в системном трее: чистая часть (TrayAction, action_for_menu_id, to_wide, пиксели иконки — тесты) + Win-клей (Shell_NotifyIconW, message-only окно, меню, TaskbarCreated)
src/winutil.rs  — FindWindow/клик-тру (LAYERED+TRANSPARENT + LWA_ALPHA!)/переутверждение HWND_TOPMOST каждый кадр (баг v0.3: Пуск перекрывал оверлей — winit ставит topmost один раз)/остановка оверлея и панели настроек (stop_settings: «Выход» из трея закрывает и её)
ARCHITECTURE.md — перенос «Как это устроено» + «Тесты (TDD)» из README (README — пользователям, ARCHITECTURE — разработчикам)
docs/preview.png (+.svg) — мок-превью виджета для шапки README (рендер скриптом вне репо; заменить на docs/demo.gif, когда владелец запишет GIF — заготовка-комментарий в README)
src/zorder.rs   — диагностика z-порядка: shell-классы (CoreWindow=Пуск/Поиск и др.), KEYPRESS_DEBUG=1 -> keypress-zdebug.log (шапка с версией), чистый + Win-клей, тесты
build.rs        — вшивает assets/keypress-uiaccess.manifest только при KEYPRESS_UIACCESS=1 (winresource)
make-uiaccess.ps1 — сборка+сертификат+подпись+Program Files (UIAccess-версия)
```

## Сборка, тесты, проверка

```bat
cargo test                                        :: 99 unit-тестов (не требует Windows)
cargo run --release                               :: оверлей + панель
cargo run --release -- --settings                 :: только панель
cargo check --target x86_64-pc-windows-gnu        :: кросс-проверка клея
KEYPRESS_UIACCESS=1 cargo build --release         :: UIAccess-вариант (см. make-uiaccess.ps1)
```

## Горячие клавиши (зафиксированы)

- `Ctrl+Alt+K` — пауза/показ (также: левый клик по иконке в трее)
- `Ctrl+Alt+Q` — выход (также: пункт «Выход» в меню трея)

## Известные ограничения MVP (осознанные)

- Только основной монитор (события вне его не отрисовываются)
- Поверх exclusive-fullscreen игр не видно (borderless/оконный — ок)
- Ввод в приложениях от админа виден только при запуске от админа (UIPI)
- Названия клавиш — английская раскладка
- Позиция виджета — процентами, а не перетаскиванием мышью

## Возможные следующие шаги (по желанию владельца)

- Звуки клавиатуры (как Thock в Keystro)
- Названия клавиш по текущей раскладке (ToUnicodeEx)
- Перетаскивание виджета мышью вместо слайдеров
- Индикация разных кнопок мыши разными цветами (поле button уже есть в UiEvent)
- Поддержка второго монитора
