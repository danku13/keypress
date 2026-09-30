//! Утилиты уровня окна: клик-тру оверлея, поиск его HWND, запуск/остановка.

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowLongPtrW, PostMessageW, SetLayeredWindowAttributes,
        SetWindowLongPtrW, GWL_EXSTYLE, LWA_ALPHA, WM_CLOSE, WS_EX_LAYERED, WS_EX_NOACTIVATE,
        WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };

    pub const OVERLAY_TITLE: &str = "keyviz-lite overlay";
    pub const SETTINGS_TITLE: &str = "Keystro-lite — настройки";

    fn utf16z(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// Открыто ли окно с таким заголовком (поиск по верхней копии).
    pub fn window_exists(title: &str) -> bool {
        let t = utf16z(title);
        unsafe { FindWindowW(std::ptr::null(), t.as_ptr()) != 0 }
    }

    fn find_overlay() -> isize {
        let t = utf16z(OVERLAY_TITLE);
        unsafe { FindWindowW(std::ptr::null(), t.as_ptr()) }
    }

    /// Делает оверлей прозрачным для мыши, видимым и не даёт красть фокус.
    ///
    /// ВАЖНО 1: клик-тру работает ТОЛЬКО в паре WS_EX_LAYERED | WS_EX_TRANSPARENT.
    /// Раньше ставился один WS_EX_TRANSPARENT — по правилам Win32 он без
    /// WS_EX_LAYERED не влияет на hit-test, и фуллскрин-оверлей глотал все
    /// клики (в т.ч. по меню Пуск).
    ///
    /// ВАЖНО 2: слоёное окно (WS_EX_LAYERED) вообще НЕ ОТОБРАЖАЕТСЯ, пока для
    /// него не вызваны SetLayeredWindowAttributes/UpdateLayeredWindow.
    /// winit 0.29.15 при включении mouse passthrough ставит
    /// WS_EX_LAYERED|WS_EX_TRANSPARENT, но атрибуты слоёв НЕ задаёт — поэтому
    /// после включения клик-тру оверлей стал полностью невидимым.
    /// SetLayeredWindowAttributes(LWA_ALPHA, 255) включает отрисовку окна;
    /// собственный per-pixel альфа-канал egui (прозрачный фон) сохраняется,
    /// а WS_EX_TRANSPARENT продолжает пропускать клики.
    ///
    /// Вызываем каждый кадр — переживает любые смены стилей окна
    /// (fullscreen и т.п.).
    pub fn apply_overlay_styles() {
        let hwnd = find_overlay();
        if hwnd == 0 {
            return;
        }
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let add =
                (WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW) as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | add);
            // Без этого вызова слоёное окно невидимо: winit 0.29.15 ставит
            // WS_EX_LAYERED, но атрибуты слоёв не задаёт.
            SetLayeredWindowAttributes(hwnd, 0, 255, LWA_ALPHA);
        }
    }

    pub fn stop_overlay() {
        let hwnd = find_overlay();
        if hwnd == 0 {
            return;
        }
        unsafe {
            PostMessageW(hwnd, WM_CLOSE, 0, 0);
        }
    }

    pub fn overlay_running() -> bool {
        find_overlay() != 0
    }
}

#[cfg(not(windows))]
mod imp {
    pub const OVERLAY_TITLE: &str = "keyviz-lite overlay";
    pub const SETTINGS_TITLE: &str = "Keystro-lite — настройки";
    pub fn apply_overlay_styles() {}
    pub fn stop_overlay() {}
    pub fn overlay_running() -> bool {
        false
    }
    pub fn window_exists(_title: &str) -> bool {
        false
    }
}

pub use imp::*;
