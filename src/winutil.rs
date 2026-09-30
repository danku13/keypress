//! Утилиты уровня окна: клик-тру оверлея, поиск его HWND, запуск/остановка.

#[cfg(windows)]
mod imp {
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        FindWindowW, GetWindowLongPtrW, PostMessageW, SetWindowLongPtrW, GWL_EXSTYLE, WM_CLOSE,
        WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT,
    };

    pub const OVERLAY_TITLE: &str = "keyviz-lite overlay";

    fn utf16z(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    fn find_overlay() -> isize {
        let t = utf16z(OVERLAY_TITLE);
        unsafe { FindWindowW(std::ptr::null(), t.as_ptr()) }
    }

    /// Делает оверлей прозрачным для мыши и не даём ему красть фокус.
    /// Вызываем каждый кадр — переживает любые смены стилей окна (fullscreen и т.п.).
    pub fn apply_overlay_styles() {
        let hwnd = find_overlay();
        if hwnd == 0 {
            return;
        }
        unsafe {
            let ex = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
            let add = (WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE) as isize;
            SetWindowLongPtrW(hwnd, GWL_EXSTYLE, ex | add);
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
    pub fn apply_overlay_styles() {}
    pub fn stop_overlay() {}
    pub fn overlay_running() -> bool {
        false
    }
}

pub use imp::*;
