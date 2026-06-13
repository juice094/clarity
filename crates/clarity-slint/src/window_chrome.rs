//! 无边框窗口的自定义标题栏支持（Windows 专用）。
//!
//! 参考成熟实现：保留 `WS_CAPTION | WS_THICKFRAME` 让 DWM 继续绘制阴影并支持
//! Aero Snap，然后在 `WM_NCCALCSIZE` 中把标准标题栏/边框的客户区计算为零，
//! 再通过 `WM_NCHITTEST` 手动把顶部区域映射为标题栏、边缘映射为缩放热区。
//!
//! 本模块调用 Win32 API 时必须使用 `unsafe`，已在调用处加 SAFETY 注释。
#![allow(unsafe_code)]

use slint::ComponentHandle;
use std::sync::OnceLock;

use clarity_slint::ui::AppWindow;

/// 自定义标题栏逻辑高度（与 `ui/app.slint` 中的 `Theme.title-bar-height` 一致）。
const TITLE_BAR_HEIGHT_LOGICAL: i32 = 40;
/// 边缘缩放热区宽度（物理像素）。
const BORDER_WIDTH_PHYSICAL: i32 = 8;
/// 右上角窗口控制按钮区宽度（物理像素），此区域不触发拖动。
const CONTROLS_WIDTH_PHYSICAL: i32 = 140;
/// 保存当前窗口的 HWND，供按钮回调使用。
static HWND: OnceLock<isize> = OnceLock::new();

/// 初始化窗口：子类化并调整样式。
#[cfg(windows)]
pub fn setup(ui: &AppWindow) {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use windows_sys::Win32::UI::Shell::SetWindowSubclass;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GWL_STYLE, GetWindowLongPtrW, SWP_FRAMECHANGED, SWP_NOMOVE, SWP_NOOWNERZORDER, SWP_NOSIZE,
        SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowLongPtrW, SetWindowPos, WS_CAPTION, WS_MAXIMIZEBOX,
        WS_MINIMIZEBOX, WS_POPUP, WS_SYSMENU, WS_THICKFRAME, WS_VISIBLE,
    };

    let handle = ui.window().window_handle();
    let raw = match handle.window_handle() {
        Ok(h) => h.as_raw(),
        Err(e) => {
            tracing::warn!("无法获取窗口句柄: {e}");
            return;
        }
    };

    let hwnd = match raw {
        RawWindowHandle::Win32(h) => h.hwnd.get(),
        _ => {
            tracing::warn!("当前平台不支持 Win32 窗口句柄");
            return;
        }
    };

    let _ = HWND.set(hwnd as isize);

    // SAFETY: 句柄来自 Slint 创建的合法窗口，且本函数只在 UI 线程调用。
    unsafe {
        // 读取当前样式并保留 WS_VISIBLE，避免 SetWindowLongPtrW 把整个样式覆盖后
        // 窗口失去可见位导致“进程在跑但窗口不显示”。
        let old_style = GetWindowLongPtrW(hwnd as _, GWL_STYLE) as u32;
        let new_style = old_style
            | WS_POPUP
            | WS_CAPTION
            | WS_THICKFRAME
            | WS_MAXIMIZEBOX
            | WS_MINIMIZEBOX
            | WS_SYSMENU
            | WS_VISIBLE;
        SetWindowLongPtrW(hwnd as _, GWL_STYLE, new_style as _);
        SetWindowPos(
            hwnd as _,
            std::ptr::null_mut(),
            0,
            0,
            0,
            0,
            SWP_FRAMECHANGED
                | SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOOWNERZORDER
                | SWP_SHOWWINDOW,
        );

        SetWindowSubclass(hwnd as _, Some(subclass_proc), 0, 0);
    }
}

/// 非 Windows 平台为空实现。
#[cfg(not(windows))]
pub fn setup(_ui: &AppWindow) {}

/// 子类化窗口过程。
#[cfg(windows)]
unsafe extern "system" fn subclass_proc(
    hwnd: windows_sys::Win32::Foundation::HWND,
    msg: u32,
    wparam: windows_sys::Win32::Foundation::WPARAM,
    lparam: windows_sys::Win32::Foundation::LPARAM,
    _uidsubclass: usize,
    _dwrefdata: usize,
) -> windows_sys::Win32::Foundation::LRESULT {
    use windows_sys::Win32::Foundation::RECT;
    use windows_sys::Win32::Graphics::Gdi::{
        GetMonitorInfoW, MONITOR_DEFAULTTONULL, MONITORINFO, MonitorFromWindow,
    };
    use windows_sys::Win32::UI::HiDpi::GetDpiForWindow;
    use windows_sys::Win32::UI::Shell::DefSubclassProc;
    use windows_sys::Win32::UI::WindowsAndMessaging::{
        GetWindowRect, HTBOTTOM, HTBOTTOMLEFT, HTBOTTOMRIGHT, HTCAPTION, HTCLIENT, HTLEFT, HTRIGHT,
        HTTOP, HTTOPLEFT, HTTOPRIGHT, IsZoomed, NCCALCSIZE_PARAMS, WM_NCACTIVATE, WM_NCCALCSIZE,
        WM_NCHITTEST,
    };

    match msg {
        // 去掉标准标题栏和边框，但保留 DWM 阴影 / Aero Snap。
        WM_NCCALCSIZE => {
            if wparam != 0 {
                let params = lparam as *mut NCCALCSIZE_PARAMS;
                // SAFETY: `lparam` 在 `WM_NCCALCSIZE` 且 `wparam != 0` 时保证指向有效结构。
                unsafe {
                    let rc = &mut (*params).rgrc[0];
                    // 最大化时，把客户区限制到显示器工作区，避免窗口边框溢出屏幕产生白边/空白。
                    if IsZoomed(hwnd) != 0 {
                        let hmon = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONULL);
                        if !hmon.is_null() {
                            let mut mi = MONITORINFO {
                                cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                                rcMonitor: RECT {
                                    left: 0,
                                    top: 0,
                                    right: 0,
                                    bottom: 0,
                                },
                                rcWork: RECT {
                                    left: 0,
                                    top: 0,
                                    right: 0,
                                    bottom: 0,
                                },
                                dwFlags: 0,
                            };
                            if GetMonitorInfoW(hmon, &mut mi) != 0 {
                                *rc = mi.rcWork;
                            }
                        }
                    }
                    // 非最大化时，让标准非客户区（标题栏、边框）全部消失，
                    // 整个窗口矩形都作为客户区。应用内容会铺满到窗口边缘，
                    // 左侧/右侧/底部不再出现系统边框或黑条。
                    // 缩放热区由 WM_NCHITTEST 在窗口内部 8px 处提供。
                }
            }
            0
        }

        // 在禁用 DWM 合成的基础主题下，阻止窗口激活时重新绘制经典边框。
        WM_NCACTIVATE => 1,

        // 自定义命中测试：边缘缩放 + 顶部标题栏拖动 + 其余区域 HTCLIENT。
        WM_NCHITTEST => {
            let x = (lparam & 0xFFFF) as i16 as i32;
            let y = ((lparam >> 16) & 0xFFFF) as i16 as i32;

            let mut rc = std::mem::MaybeUninit::<RECT>::uninit();
            // SAFETY: `rc` 已分配且 `hwnd` 为有效窗口句柄。
            unsafe { GetWindowRect(hwnd, rc.as_mut_ptr()) };
            // SAFETY: 前一行已初始化。
            let rc = unsafe { rc.assume_init() };

            // SAFETY: `hwnd` 为有效窗口句柄。
            let dpi = unsafe { GetDpiForWindow(hwnd) };
            let title_height = TITLE_BAR_HEIGHT_LOGICAL * (dpi as i32).max(96) / 96;

            let left = x - rc.left;
            let top = y - rc.top;
            let right = rc.right - x;
            let bottom = rc.bottom - y;

            let hit = if top < BORDER_WIDTH_PHYSICAL && left < BORDER_WIDTH_PHYSICAL {
                HTTOPLEFT
            } else if top < BORDER_WIDTH_PHYSICAL && right < BORDER_WIDTH_PHYSICAL {
                HTTOPRIGHT
            } else if bottom < BORDER_WIDTH_PHYSICAL && left < BORDER_WIDTH_PHYSICAL {
                HTBOTTOMLEFT
            } else if bottom < BORDER_WIDTH_PHYSICAL && right < BORDER_WIDTH_PHYSICAL {
                HTBOTTOMRIGHT
            } else if left < BORDER_WIDTH_PHYSICAL {
                HTLEFT
            } else if right < BORDER_WIDTH_PHYSICAL {
                HTRIGHT
            } else if top < BORDER_WIDTH_PHYSICAL {
                HTTOP
            } else if bottom < BORDER_WIDTH_PHYSICAL {
                HTBOTTOM
            } else if top < title_height && right > CONTROLS_WIDTH_PHYSICAL {
                HTCAPTION
            } else {
                HTCLIENT
            };

            hit as isize
        }

        // SAFETY: 转发给子类默认过程。
        _ => unsafe { DefSubclassProc(hwnd, msg, wparam, lparam) },
    }
}

/// 最小化窗口。
pub fn minimize() {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{SW_MINIMIZE, ShowWindow};
        if let Some(&hwnd) = HWND.get() {
            // SAFETY: HWND 为有效窗口句柄。
            unsafe {
                ShowWindow(hwnd as _, SW_MINIMIZE);
            }
        }
    }
}

/// 切换最大化/还原状态，并同步到 UI 属性。
pub fn toggle_maximize(ui: &AppWindow) {
    #[cfg(windows)]
    {
        use windows_sys::Win32::UI::WindowsAndMessaging::{
            IsZoomed, SW_MAXIMIZE, SW_RESTORE, ShowWindow,
        };
        if let Some(&hwnd) = HWND.get() {
            // SAFETY: HWND 为有效窗口句柄。
            let maximized = unsafe { IsZoomed(hwnd as _) != 0 };
            let next = if maximized { SW_RESTORE } else { SW_MAXIMIZE };
            unsafe {
                ShowWindow(hwnd as _, next);
            }
            ui.set_window_maximized(!maximized);
        }
    }
}

/// 关闭窗口。
pub fn close() {
    // 优先走 Slint 的事件循环退出，保留清理路径。
    slint::quit_event_loop().ok();
}
