//! macOS 平台能力：鼠标位置读取、模拟 ⌘V 自动粘贴、辅助功能权限探测。
//!
//! 全部直接调用 CoreGraphics / ApplicationServices 的 C 接口，不引入额外的
//! Objective-C 绑定依赖，避免为一两个系统调用拖入整棵 objc2 依赖树。

use std::ffi::c_void;
use std::ptr;

/// `kVK_ANSI_V`：macOS 上 V 键的虚拟键码（和 Windows 的 VK 码不是一套）。
const KVK_ANSI_V: u16 = 0x09;

/// `kCGEventSourceStateHIDSystemState`：以硬件层输入状态作为事件源。
const CG_EVENT_SOURCE_STATE_HID_SYSTEM: u32 = 1;
/// `kCGHIDEventTap`：把事件投递到 HID 层，等价于真实硬件输入。
const CG_HID_EVENT_TAP: u32 = 0;
/// `kCGEventFlagMaskCommand`：Command（⌘）修饰键。
const CG_EVENT_FLAG_MASK_COMMAND: u64 = 1 << 20;

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CGPoint {
    x: f64,
    y: f64,
}

#[link(name = "CoreGraphics", kind = "framework")]
extern "C" {
    fn CGEventCreate(source: *const c_void) -> *mut c_void;
    fn CGEventCreateKeyboardEvent(
        source: *const c_void,
        virtual_key: u16,
        key_down: bool,
    ) -> *mut c_void;
    fn CGEventSetFlags(event: *mut c_void, flags: u64);
    fn CGEventPost(tap: u32, event: *mut c_void);
    fn CGEventGetLocation(event: *mut c_void) -> CGPoint;
    fn CGEventSourceCreate(state_id: u32) -> *mut c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    /// 是否已被授予「辅助功能」权限。没有它，系统会静默丢弃我们合成的按键。
    fn AXIsProcessTrusted() -> u8;
}

/// 当前鼠标位置。坐标系与 Windows 一致：原点在主显示器左上角，单位为逻辑点。
pub fn cursor_position() -> Option<(f64, f64)> {
    unsafe {
        let event = CGEventCreate(ptr::null());
        if event.is_null() {
            return None;
        }
        let point = CGEventGetLocation(event);
        CFRelease(event as *const c_void);
        Some((point.x, point.y))
    }
}

/// 是否拥有模拟按键所需的「辅助功能」权限。
pub fn has_accessibility_permission() -> bool {
    unsafe { AXIsProcessTrusted() != 0 }
}

/// 模拟一次 ⌘V，把剪贴板内容粘贴到当前前台应用。
///
/// 前置条件：应用已被授予「辅助功能」权限，且快捷窗口已经隐藏、焦点交还给了
/// 目标应用。调用方需先用 [`has_accessibility_permission`] 判断，否则用户在
/// 无权限时只会看到「点了没反应」。
pub fn simulate_paste() -> Result<(), String> {
    unsafe {
        let source = CGEventSourceCreate(CG_EVENT_SOURCE_STATE_HID_SYSTEM);
        if source.is_null() {
            return Err("无法创建输入事件源".to_string());
        }

        let key_down = CGEventCreateKeyboardEvent(source, KVK_ANSI_V, true);
        let key_up = CGEventCreateKeyboardEvent(source, KVK_ANSI_V, false);
        if key_down.is_null() || key_up.is_null() {
            if !key_down.is_null() {
                CFRelease(key_down as *const c_void);
            }
            if !key_up.is_null() {
                CFRelease(key_up as *const c_void);
            }
            CFRelease(source as *const c_void);
            return Err("无法创建按键事件".to_string());
        }

        // 按下与抬起都必须带上 ⌘，否则目标应用收到的是一次裸 V 键。
        CGEventSetFlags(key_down, CG_EVENT_FLAG_MASK_COMMAND);
        CGEventSetFlags(key_up, CG_EVENT_FLAG_MASK_COMMAND);
        CGEventPost(CG_HID_EVENT_TAP, key_down);
        CGEventPost(CG_HID_EVENT_TAP, key_up);

        CFRelease(key_down as *const c_void);
        CFRelease(key_up as *const c_void);
        CFRelease(source as *const c_void);
    }

    Ok(())
}
