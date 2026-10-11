//! 平台相关的系统调用抽象。
//!
//! 目前只包含鼠标位置读取：Windows 走 Win32，macOS 走 CoreGraphics。
//! 统一返回「主显示器左上角为原点」的坐标，Windows 下是物理像素，
//! macOS 下是逻辑点——这正是各自平台窗口定位 API 期待的坐标系。

#[cfg(target_os = "windows")]
pub fn cursor_position() -> Option<(f64, f64)> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    unsafe {
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point).ok()?;
        Some((f64::from(point.x), f64::from(point.y)))
    }
}

#[cfg(target_os = "macos")]
pub fn cursor_position() -> Option<(f64, f64)> {
    crate::macos::cursor_position()
}

#[cfg(not(any(target_os = "windows", target_os = "macos")))]
pub fn cursor_position() -> Option<(f64, f64)> {
    None
}
