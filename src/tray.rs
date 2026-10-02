//! Иконка keypress в системном трее Windows.
//!
//! Живёт в отдельном потоке с message-only окном (HWND_MESSAGE):
//! Shell_NotifyIconW присылает события мыши в это окно, WndProc превращает их
//! в UiEvent и отправляет в ТОТ ЖЕ crossbeam-канал, что и хуки клавиатуры/мыши.
//! Оверлей разбирает их в drain_events() — ни новых глобальных состояний,
//! ни второго канала.
//!
//! Поведение:
//! - левый клик по иконке — пауза/показ (аналог Ctrl+Alt+K);
//! - двойной клик — открыть панель настроек (как повторный запуск exe);
//! - правый клик — меню: пауза (с галочкой), настройки, выход.
//!
//! Иконка рисуется кодом (32x32 BGRA -> CreateDIBSection ->
//! CreateIconIndirect): синий кейкап с буквой K, без внешних ассетов.
//!
//! Перезапуск explorer (панель задач пересоздаётся) ловится сообщением
//! TaskbarCreated — иконка добавляется заново. Если иконку не удалось
//! добавить сразу (ранний старт, explorer ещё не готов) — ретраи.
//!
//! Завершение: shutdown() (зовётся оверлеем после выхода из eframe) шлёт
//! WM_CLOSE в сообщение-окно трея, WM_DESTROY удаляет иконку, поток выходит.
//! Без этого иконка-«призрак» висела бы в трее до наведения мыши.

// ---- Кроссплатформенная часть: тестируется на Linux через cargo test ----

/// Действие, выбранное пользователем в трее.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(not(windows), allow(dead_code))]
pub enum TrayAction {
    /// Пауза / продолжить показ (как Ctrl+Alt+K).
    PauseResume,
    /// Открыть панель настроек (если ещё не открыта).
    OpenSettings,
    /// Выход из программы.
    Quit,
}

/// Идентификаторы пунктов меню трея (произвольные наши числа).
#[cfg_attr(not(windows), allow(dead_code))]
pub const MENU_PAUSE: u32 = 1;
#[cfg_attr(not(windows), allow(dead_code))]
pub const MENU_SETTINGS: u32 = 2;
#[cfg_attr(not(windows), allow(dead_code))]
pub const MENU_QUIT: u32 = 3;

/// id пункта меню -> действие.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn action_for_menu_id(id: u32) -> Option<TrayAction> {
    match id {
        MENU_PAUSE => Some(TrayAction::PauseResume),
        MENU_SETTINGS => Some(TrayAction::OpenSettings),
        MENU_QUIT => Some(TrayAction::Quit),
        _ => None,
    }
}

/// UTF-16 со строковым нулём (формат строк Win-API).
#[cfg_attr(not(windows), allow(dead_code))]
pub fn to_wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Расстояние от точки до отрезка (для отрисовки буквы K).
#[cfg_attr(not(windows), allow(dead_code))]
fn sd_segment(px: f32, py: f32, x1: f32, y1: f32, x2: f32, y2: f32) -> f32 {
    let vx = x2 - x1;
    let vy = y2 - y1;
    let wx = px - x1;
    let wy = py - y1;
    let len2 = vx * vx + vy * vy;
    let t = if len2 > 0.0 {
        ((wx * vx + wy * vy) / len2).clamp(0.0, 1.0)
    } else {
        0.0
    };
    let dx = px - (x1 + t * vx);
    let dy = py - (y1 + t * vy);
    (dx * dx + dy * dy).sqrt()
}

/// Знаковое расстояние до скруглённого прямоугольника (отрицательное внутри).
#[cfg_attr(not(windows), allow(dead_code))]
fn sd_rounded_rect(px: f32, py: f32, minx: f32, miny: f32, maxx: f32, maxy: f32, r: f32) -> f32 {
    let cx = (minx + maxx) * 0.5;
    let cy = (miny + maxy) * 0.5;
    let hx = (maxx - minx) * 0.5 - r;
    let hy = (maxy - miny) * 0.5 - r;
    let dx = (px - cx).abs() - hx;
    let dy = (py - cy).abs() - hy;
    let ax = dx.max(0.0);
    let ay = dy.max(0.0);
    (ax * ax + ay * ay).sqrt() + dx.max(dy).min(0.0) - r
}

/// Пиксели иконки трея: 32x32, BGRA (порядок байт DIB), top-down, прямая
/// альфа. Синий кейкап (фирменный Windows-акцент) с белой буквой K.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn icon_pixels_bgra() -> Vec<u8> {
    const SIZE: usize = 32;
    let mut buf = vec![0u8; SIZE * SIZE * 4];
    for y in 0..SIZE {
        for x in 0..SIZE {
            let px = x as f32 + 0.5;
            let py = y as f32 + 0.5;
            let d = sd_rounded_rect(px, py, 2.5, 2.5, 29.5, 29.5, 7.0);
            let cap = (0.5 - d).clamp(0.0, 1.0); // сглаженный край кейкапа
            if cap <= 0.0 {
                continue;
            }
            // Буква K: стойка + две диагонали (сглаженная обводка)
            let k = sd_segment(px, py, 12.0, 8.0, 12.0, 24.0)
                .min(sd_segment(px, py, 21.5, 8.0, 13.0, 16.0))
                .min(sd_segment(px, py, 13.0, 16.0, 21.5, 24.0));
            let t = (1.7 - k).clamp(0.0, 1.0);
            let r = 0.0 + (255.0 - 0.0) * t;
            let g = 120.0 + (255.0 - 120.0) * t;
            let b = 215.0 + (255.0 - 215.0) * t;
            let a = cap * 255.0;
            let i = (y * SIZE + x) * 4;
            buf[i] = b as u8;
            buf[i + 1] = g as u8;
            buf[i + 2] = r as u8;
            buf[i + 3] = a as u8;
        }
    }
    buf
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn menu_ids_map_to_actions() {
        assert_eq!(
            action_for_menu_id(MENU_PAUSE),
            Some(TrayAction::PauseResume)
        );
        assert_eq!(
            action_for_menu_id(MENU_SETTINGS),
            Some(TrayAction::OpenSettings)
        );
        assert_eq!(action_for_menu_id(MENU_QUIT), Some(TrayAction::Quit));
        assert_eq!(action_for_menu_id(0), None);
        assert_eq!(action_for_menu_id(999), None);
    }

    #[test]
    fn menu_ids_are_distinct() {
        assert_ne!(MENU_PAUSE, MENU_SETTINGS);
        assert_ne!(MENU_SETTINGS, MENU_QUIT);
        assert_ne!(MENU_PAUSE, MENU_QUIT);
    }

    #[test]
    fn to_wide_is_utf16z() {
        assert_eq!(to_wide("aЯ"), vec![0x61, 0x42F, 0]);
        assert_eq!(to_wide(""), vec![0]);
        assert_eq!(to_wide("K").len(), 2);
    }

    #[test]
    fn icon_is_32x32_bgra_keycap() {
        let px = icon_pixels_bgra();
        assert_eq!(px.len(), 32 * 32 * 4);
        let alpha = |x: usize, y: usize| px[(y * 32 + x) * 4 + 3];
        // Углы прозрачны (кейкап скруглён, отступ от края)
        assert_eq!(alpha(0, 0), 0);
        assert_eq!(alpha(31, 31), 0);
        assert_eq!(alpha(0, 31), 0);
        // Центр кейкапа непрозрачен
        assert_eq!(alpha(16, 16), 255);
        // Стойка буквы K (x=12) — белая: R высокий, B высокий
        let i = (16 * 32 + 12) * 4;
        assert!(px[i + 2] > 200, "R стойки K — {}", px[i + 2]);
        // Корпус левее стойки (x=5) — синий: R низкий, B высокий
        let j = (16 * 32 + 5) * 4;
        assert!(px[j + 2] < 100, "R корпуса — {}", px[j + 2]);
        assert!(px[j] > 150, "B корпуса — {}", px[j]);
    }

    #[test]
    fn rounded_rect_sdf_is_negative_inside() {
        assert!(sd_rounded_rect(16.0, 16.0, 2.5, 2.5, 29.5, 29.5, 7.0) < 0.0);
        assert!(sd_rounded_rect(0.0, 0.0, 2.5, 2.5, 29.5, 29.5, 7.0) > 0.0);
        // Вне прямоугольника расстояние до границы корректно по осям
        assert!(sd_rounded_rect(16.0, 40.0, 2.5, 2.5, 29.5, 29.5, 7.0) > 0.0);
    }

    /// Визуальная проверка иконки (аналог dump_svgs из icons.rs):
    /// `cargo test -- --ignored dump_tray_icon --nocapture` — пишет
    /// /tmp/keypress-tray/icon.ppm (BGRA с альфой, композит на белый).
    #[test]
    #[ignore]
    fn dump_tray_icon() {
        let px = icon_pixels_bgra();
        let mut ppm = b"P6\n32 32\n255\n".to_vec();
        for i in (0..px.len()).step_by(4) {
            let a = px[i + 3] as f32 / 255.0;
            // BGRA -> RGB поверх белого
            for ch in [px[i + 2], px[i + 1], px[i]] {
                let v = (255.0 * (1.0 - a) + ch as f32 * a).round() as u8;
                ppm.push(v);
            }
        }
        std::fs::create_dir_all("/tmp/keypress-tray").ok();
        std::fs::write("/tmp/keypress-tray/icon.ppm", &ppm).unwrap();
        println!("written /tmp/keypress-tray/icon.ppm");
    }
}

// ---- Windows-реализация ----

#[cfg(windows)]
mod imp {
    use super::{
        action_for_menu_id, icon_pixels_bgra, to_wide, TrayAction, MENU_PAUSE, MENU_QUIT,
        MENU_SETTINGS,
    };
    use crate::input::UiEvent;
    use crossbeam_channel::Sender;
    use std::cell::RefCell;
    use std::sync::atomic::{AtomicBool, AtomicIsize, AtomicU32, Ordering};
    use std::time::Duration;
    use windows_sys::Win32::Foundation::{BOOL, HANDLE, HWND, LPARAM, LRESULT, POINT, WPARAM};
    use windows_sys::Win32::Graphics::Gdi::{
        CreateBitmap, CreateCompatibleDC, CreateDIBSection, DeleteDC, DeleteObject, GetDC,
        ReleaseDC, BITMAPINFO, BITMAPINFOHEADER, BI_RGB, DIB_RGB_COLORS, HGDIOBJ,
    };
    use windows_sys::Win32::System::LibraryLoader::GetModuleHandleW;
    use windows_sys::Win32::UI::Shell::{
        Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NOTIFYICONDATAW,
    };
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreateIconIndirect, CreatePopupMenu, CreateWindowExW, DefWindowProcW,
        DestroyMenu, DispatchMessageW, GetCursorPos, GetMessageW, PostMessageW, PostQuitMessage,
        RegisterClassW, RegisterWindowMessageW, SetForegroundWindow, TrackPopupMenu,
        TranslateMessage, HWND_MESSAGE, ICONINFO, MF_CHECKED, MF_SEPARATOR, MF_STRING, MSG,
        TPM_NONOTIFY, TPM_RETURNCMD, TPM_RIGHTBUTTON, WM_APP, WM_CLOSE, WM_DESTROY,
        WM_LBUTTONDBLCLK, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP, WNDCLASSW,
    };

    /// Наше callback-сообщение трея (легаси-режим: lParam = сообщение мыши).
    const WM_TRAY: u32 = WM_APP + 1;
    /// Идентификатор иконки (одна на процесс).
    const TRAY_ID: u32 = 1;
    const TIP: &str = "keypress — индикатор нажатий";

    static TRAY_HWND: AtomicIsize = AtomicIsize::new(0);
    static TRAY_ICON: AtomicIsize = AtomicIsize::new(0);
    static TASKBAR_CREATED: AtomicU32 = AtomicU32::new(0);
    /// Текущее состояние паузы — для галочки в меню.
    static PAUSED: AtomicBool = AtomicBool::new(false);
    /// Поток трея закончил цикл сообщений (для shutdown()).
    static FINISHED: AtomicBool = AtomicBool::new(false);

    thread_local! {
        static TX: RefCell<Option<Sender<UiEvent>>> = RefCell::new(None);
    }

    fn send(ev: UiEvent) {
        TX.with(|t| {
            if let Some(tx) = t.borrow().as_ref() {
                let _ = tx.send(ev);
            }
        });
    }

    pub fn set_paused(v: bool) {
        PAUSED.store(v, Ordering::Relaxed);
    }

    pub fn spawn(tx: Sender<UiEvent>) {
        std::thread::Builder::new()
            .name("tray".into())
            .spawn(move || unsafe { run(tx) })
            .expect("failed to spawn tray thread");
    }

    /// Убрать иконку и дождаться завершения потока трея (максимум ~1 с).
    /// Безопасно звать повторно и когда трей не запускался.
    pub fn shutdown() {
        let hwnd = TRAY_HWND.load(Ordering::Acquire);
        if hwnd == 0 {
            return;
        }
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
        for _ in 0..100 {
            if FINISHED.load(Ordering::Acquire) {
                return;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    unsafe fn run(tx: Sender<UiEvent>) {
        TX.with(|t| *t.borrow_mut() = Some(tx));

        // Класс регистрируется на процесс; при пересоздании трея повторная
        // регистрация вернёт ошибку — это нормально, CreateWindowExW найдёт
        // уже зарегистрированный класс.
        let class_name = to_wide("keypress_tray_host");
        let hinstance = GetModuleHandleW(std::ptr::null());
        let wc = WNDCLASSW {
            lpfnWndProc: Some(tray_wndproc),
            lpszClassName: class_name.as_ptr(),
            hInstance: hinstance,
            ..std::mem::zeroed()
        };
        RegisterClassW(&wc);

        // Message-only окно: нигде не видно, получает только наши сообщения.
        let hwnd = CreateWindowExW(
            0,
            class_name.as_ptr(),
            class_name.as_ptr(),
            0,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            0,
            hinstance,
            std::ptr::null(),
        );
        TRAY_HWND.store(hwnd, Ordering::Release);
        let wm_taskbar = RegisterWindowMessageW(to_wide("TaskbarCreated").as_ptr());
        TASKBAR_CREATED.store(wm_taskbar, Ordering::Release);

        // Иконка рисуется один раз и переиспользуется (в т.ч. при перезапуске
        // explorer через TaskbarCreated).
        let icon = create_icon();
        TRAY_ICON.store(icon, Ordering::Release);

        // explorer может быть ещё не готов (автозапуск до входа в систему) —
        // ретраим около 5 секунд, дальше живём без иконки до TaskbarCreated.
        for _ in 0..20 {
            if add_icon(hwnd) != 0 {
                break;
            }
            std::thread::sleep(Duration::from_millis(250));
        }

        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, 0, 0, 0) > 0 {
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        FINISHED.store(true, Ordering::Release);
    }

    /// Заполняет wide-буфер фиксированной длины (строка + ноль).
    fn set_wide(dst: &mut [u16], s: &str) {
        let wide = to_wide(s);
        let n = wide.len().min(dst.len());
        dst[..n].copy_from_slice(&wide[..n]);
        dst[dst.len() - 1] = 0;
    }

    unsafe fn add_icon(hwnd: HWND) -> BOOL {
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ID;
        nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP;
        nid.uCallbackMessage = WM_TRAY;
        nid.hIcon = TRAY_ICON.load(Ordering::Acquire);
        set_wide(&mut nid.szTip, TIP);
        Shell_NotifyIconW(NIM_ADD, &nid)
    }

    unsafe fn remove_icon() {
        let hwnd = TRAY_HWND.load(Ordering::Acquire);
        if hwnd == 0 {
            return;
        }
        let mut nid: NOTIFYICONDATAW = std::mem::zeroed();
        nid.cbSize = std::mem::size_of::<NOTIFYICONDATAW>() as u32;
        nid.hWnd = hwnd;
        nid.uID = TRAY_ID;
        Shell_NotifyIconW(NIM_DELETE, &nid);
    }

    /// HICON из пикселей icon_pixels_bgra: 32bpp DIB + нулевая 1bpp маска.
    /// При 32bpp-цвете с альфа-каналом Windows использует альфу (маска обязана
    /// существовать формально, но не влияет).
    unsafe fn create_icon() -> windows_sys::Win32::UI::WindowsAndMessaging::HICON {
        let pixels = icon_pixels_bgra();
        let hdc = GetDC(0 as HWND);
        let hdc_mem = CreateCompatibleDC(hdc);
        let mut bi: BITMAPINFO = std::mem::zeroed();
        bi.bmiHeader.biSize = std::mem::size_of::<BITMAPINFOHEADER>() as u32;
        bi.bmiHeader.biWidth = 32;
        bi.bmiHeader.biHeight = -32; // top-down
        bi.bmiHeader.biPlanes = 1;
        bi.bmiHeader.biBitCount = 32;
        bi.bmiHeader.biCompression = BI_RGB;
        let mut bits: *mut core::ffi::c_void = std::ptr::null_mut();
        let hbm_color = CreateDIBSection(hdc_mem, &bi, DIB_RGB_COLORS, &mut bits, 0 as HANDLE, 0);
        if !bits.is_null() {
            std::ptr::copy_nonoverlapping(pixels.as_ptr(), bits as *mut u8, pixels.len());
        }
        let mask = [0u8; 128]; // 32x32 / 8 байт
        let hbm_mask = CreateBitmap(32, 32, 1, 1, mask.as_ptr() as *const core::ffi::c_void);
        let ii = ICONINFO {
            fIcon: 1,
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: hbm_mask,
            hbmColor: hbm_color,
        };
        let icon = CreateIconIndirect(&ii);
        DeleteObject(hbm_mask as HGDIOBJ);
        DeleteObject(hbm_color as HGDIOBJ);
        DeleteDC(hdc_mem);
        ReleaseDC(0 as HWND, hdc);
        icon
    }

    unsafe extern "system" fn tray_wndproc(
        hwnd: HWND,
        msg: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        if msg == WM_TRAY {
            // Легаси-режим (без NIM_SETVERSION): lParam — сообщение мыши.
            match lparam as u32 {
                WM_LBUTTONUP => send(UiEvent::TogglePause),
                WM_LBUTTONDBLCLK => open_settings(),
                WM_RBUTTONUP => show_menu(hwnd),
                _ => {}
            }
            0
        } else {
            let wm_taskbar = TASKBAR_CREATED.load(Ordering::Acquire);
            if wm_taskbar != 0 && msg == wm_taskbar {
                // Панель задач пересоздана (explorer перезапущен) — вернуть иконку.
                add_icon(hwnd);
                0
            } else if msg == WM_DESTROY {
                remove_icon();
                PostQuitMessage(0);
                0
            } else {
                DefWindowProcW(hwnd, msg, wparam, lparam)
            }
        }
    }

    unsafe fn show_menu(hwnd: HWND) {
        let menu = CreatePopupMenu();
        if menu == 0 {
            return;
        }
        let pause_flags = if PAUSED.load(Ordering::Relaxed) {
            MF_STRING | MF_CHECKED
        } else {
            MF_STRING
        };
        AppendMenuW(
            menu,
            pause_flags,
            MENU_PAUSE as usize,
            to_wide("Пауза / Показ (Ctrl+Alt+K)").as_ptr(),
        );
        AppendMenuW(
            menu,
            MF_STRING,
            MENU_SETTINGS as usize,
            to_wide("Настройки").as_ptr(),
        );
        AppendMenuW(menu, MF_SEPARATOR, 0, std::ptr::null());
        AppendMenuW(
            menu,
            MF_STRING,
            MENU_QUIT as usize,
            to_wide("Выход").as_ptr(),
        );

        let mut pt = POINT { x: 0, y: 0 };
        GetCursorPos(&mut pt);
        // Классический трюк: без SetForegroundWindow меню не закрывается
        // по клику мимо него.
        SetForegroundWindow(hwnd);
        // С TPM_RETURNCMD TrackPopupMenu возвращает id выбранного пункта
        // (0 — ничего не выбрано), сам пункт меню НИЧЕГО не выполняет.
        let cmd = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_RETURNCMD | TPM_NONOTIFY,
            pt.x,
            pt.y,
            0,
            hwnd,
            std::ptr::null(),
        ) as u32;
        PostMessageW(hwnd, WM_NULL, 0, 0);
        DestroyMenu(menu);

        match action_for_menu_id(cmd) {
            Some(TrayAction::PauseResume) => send(UiEvent::TogglePause),
            Some(TrayAction::OpenSettings) => open_settings(),
            Some(TrayAction::Quit) => send(UiEvent::Quit),
            None => {}
        }
    }

    /// Панель настроек — отдельный процесс; та же логика, что и при повторном
    /// запуске keypress.exe (spawn_panel_if_needed в main.rs).
    fn open_settings() {
        crate::spawn_panel_if_needed();
    }
}

#[cfg(windows)]
pub use imp::{set_paused, shutdown, spawn};

#[cfg(not(windows))]
mod imp {
    pub fn set_paused(_v: bool) {}
    pub fn shutdown() {}
}

#[cfg(not(windows))]
pub use imp::{set_paused, shutdown};
