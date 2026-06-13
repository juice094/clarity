use raw_window_handle::{HasWindowHandle, RawWindowHandle};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWMWA_WINDOW_CORNER_PREFERENCE, DwmSetWindowAttribute};

/// Apply Windows 11 DWM rounded corners to the application window.
/// No-op on Windows 10 (DWM silently ignores the attribute).
#[allow(unsafe_code)]
pub fn apply_rounded_corners(cc: &eframe::CreationContext<'_>) -> Option<()> {
    let handle = cc.window_handle().ok()?;
    let raw = handle.as_raw();
    let hwnd = match raw {
        RawWindowHandle::Win32(h) => HWND(h.hwnd.get() as _),
        _ => return None,
    };

    // DWMWCP_ROUND = 2
    let preference: u32 = 2;
    // SAFETY: `hwnd` is a valid window handle obtained from `eframe`.
    // `preference` is a properly aligned `u32` with lifetime extending past
    // this call. The pointer and size arguments match the expected type.
    unsafe {
        DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            std::mem::size_of::<u32>() as u32,
        )
        .ok()?;
    }
    Some(())
}
