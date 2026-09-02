use tauri::{AppHandle, Manager};

#[cfg(target_os = "windows")]
use std::sync::{Mutex, OnceLock};

#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, POINT};
#[cfg(target_os = "windows")]
use windows::Win32::Graphics::Gdi::{
    GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST,
};
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    GetCursorPos, GetForegroundWindow, SetForegroundWindow,
};

#[cfg(target_os = "windows")]
static PREVIOUS_WINDOW: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

/// 显示快捷窗口，并保存原前台窗口供自动粘贴时恢复焦点。
pub fn toggle_popup(app: &AppHandle) -> Result<(), String> {
    let window = app
        .get_webview_window("popup")
        .ok_or_else(|| "快捷窗口不存在".to_string())?;
    if window.is_visible().unwrap_or(false) {
        return window.hide().map_err(|error| error.to_string());
    }

    #[cfg(target_os = "windows")]
    remember_foreground_window(&window);

    let (x, y) = popup_position(&window);
    window
        .set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;
    // 跨越不同缩放比例的显示器后，窗口外框尺寸可能随 DPI 更新，再校正一次位置。
    let (corrected_x, corrected_y) = popup_position(&window);
    if (corrected_x, corrected_y) != (x, y) {
        window
            .set_position(tauri::PhysicalPosition::new(corrected_x, corrected_y))
            .map_err(|error| error.to_string())?;
    }
    window.show().map_err(|error| error.to_string())?;
    window.set_focus().map_err(|error| error.to_string())
}

pub fn hide_popup(app: &AppHandle) -> Result<(), String> {
    app.get_webview_window("popup")
        .ok_or_else(|| "快捷窗口不存在".to_string())?
        .hide()
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "windows")]
pub fn restore_previous_window() -> Result<(), String> {
    let handle = PREVIOUS_WINDOW
        .get_or_init(|| Mutex::new(None))
        .lock()
        .map_err(|_| "前台窗口状态读取失败".to_string())?
        .take();
    if let Some(handle) = handle {
        let restored = unsafe { SetForegroundWindow(HWND(handle as *mut _)) };
        if !restored.as_bool() {
            return Err("恢复原窗口焦点失败".to_string());
        }
    }
    Ok(())
}

#[cfg(not(target_os = "windows"))]
pub fn restore_previous_window() -> Result<(), String> {
    Ok(())
}

#[cfg(target_os = "windows")]
fn remember_foreground_window(window: &tauri::WebviewWindow) {
    let foreground = unsafe { GetForegroundWindow() };
    let popup_handle = window.hwnd().ok().map(|handle| handle.0 as isize);
    let foreground_handle = foreground.0 as isize;
    if !foreground.is_invalid() && popup_handle != Some(foreground_handle) {
        if let Ok(mut state) = PREVIOUS_WINDOW.get_or_init(|| Mutex::new(None)).lock() {
            *state = Some(foreground_handle);
        }
    }
}

#[cfg(target_os = "windows")]
fn popup_position(window: &tauri::WebviewWindow) -> (i32, i32) {
    unsafe {
        let mut point = POINT::default();
        if GetCursorPos(&mut point).is_err() {
            return (100, 100);
        }
        let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        if !GetMonitorInfoW(monitor, &mut info).as_bool() {
            return (point.x + 10, point.y + 10);
        }
        let area = info.rcWork;
        // Tauri 配置中的宽高是逻辑像素，而 Win32 工作区和窗口位置使用物理像素。
        // 读取真实外框尺寸，避免系统缩放后窗口越出显示器工作区。
        let window_size = window
            .outer_size()
            .unwrap_or(tauri::PhysicalSize::new(360, 500));
        let window_width = i32::try_from(window_size.width).unwrap_or(i32::MAX);
        let window_height = i32::try_from(window_size.height).unwrap_or(i32::MAX);
        let max_x = (area.right - window_width).max(area.left);
        let max_y = (area.bottom - window_height).max(area.top);
        let x = (point.x + 10).clamp(area.left, max_x);
        let y = (point.y + 10).clamp(area.top, max_y);
        (x, y)
    }
}

#[cfg(not(target_os = "windows"))]
fn popup_position(_window: &tauri::WebviewWindow) -> (i32, i32) {
    (100, 100)
}
