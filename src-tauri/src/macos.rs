//! macOS 平台能力：鼠标位置读取、模拟 ⌘V 自动粘贴、辅助功能权限探测。
//!
//! 全部直接调用 CoreGraphics / ApplicationServices 的 C 接口，不引入额外的
//! Objective-C 绑定依赖，避免为一两个系统调用拖入整棵 objc2 依赖树。

use std::ffi::{c_char, c_void, CString};
use std::ptr;

/// `kVK_ANSI_V`：macOS 上 V 键的虚拟键码（和 Windows 的 VK 码不是一套）。
const KVK_ANSI_V: u16 = 0x09;

/// `kCGEventSourceStateHIDSystemState`：以硬件层输入状态作为事件源。
const CG_EVENT_SOURCE_STATE_HID_SYSTEM: u32 = 1;
/// `kCGHIDEventTap`：把事件投递到 HID 层，等价于真实硬件输入。
const CG_HID_EVENT_TAP: u32 = 0;
/// `kCGEventFlagMaskCommand`：Command（⌘）修饰键。
const CG_EVENT_FLAG_MASK_COMMAND: u64 = 1 << 20;

/// `kCGWindowListOptionOnScreenOnly`：只取当前屏幕上的窗口。
const CG_WINDOW_LIST_ON_SCREEN_ONLY: u32 = 1 << 0;
/// `kCGWindowListExcludeDesktopElements`：排除桌面、壁纸这类元素。
const CG_WINDOW_LIST_EXCLUDE_DESKTOP: u32 = 1 << 4;
/// `kCFStringEncodingUTF8`。
const CF_STRING_ENCODING_UTF8: u32 = 0x0800_0100;
/// `kCFNumberSInt32Type`：pid、窗口层级都是 32 位整数。
const CF_NUMBER_SINT32: i64 = 3;

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
    /// 取屏幕上窗口的信息（每个窗口一个字典），用于判断当前是哪个应用在前台。
    fn CGWindowListCopyWindowInfo(option: u32, relative_window: u32) -> *mut c_void;
}

#[link(name = "CoreFoundation", kind = "framework")]
extern "C" {
    fn CFRelease(cf: *const c_void);
    fn CFArrayGetCount(array: *const c_void) -> isize;
    fn CFArrayGetValueAtIndex(array: *const c_void, index: isize) -> *const c_void;
    fn CFDictionaryGetValue(dict: *const c_void, key: *const c_void) -> *const c_void;
    fn CFNumberGetValue(number: *const c_void, the_type: i64, value_ptr: *mut c_void) -> u8;
    fn CFStringCreateWithCString(
        allocator: *const c_void,
        c_str: *const c_char,
        encoding: u32,
    ) -> *const c_void;
    fn CFDictionaryCreate(
        allocator: *const c_void,
        keys: *const *const c_void,
        values: *const *const c_void,
        num_values: isize,
        key_callbacks: *const c_void,
        value_callbacks: *const c_void,
    ) -> *mut c_void;
    static kCFBooleanTrue: *const c_void;
    static kCFTypeDictionaryKeyCallBacks: c_void;
    static kCFTypeDictionaryValueCallBacks: c_void;
}

#[link(name = "ApplicationServices", kind = "framework")]
extern "C" {
    /// 是否已被授予「辅助功能」权限。没有它，系统会静默丢弃我们合成的按键。
    fn AXIsProcessTrusted() -> u8;
    /// 带选项版本：传入 `kAXTrustedCheckOptionPrompt = true` 时，系统会主动弹出
    /// 授权引导，并把当前这份二进制登记进「辅助功能」列表。
    ///
    /// 只调用 `AXIsProcessTrusted` 只做查询，不会登记；用户手动去列表面里打开开关时，
    /// 系统登记的可能是旧版本留下的同一条目，于是出现「开关明明是开的、程序仍报无权限」。
    fn AXIsProcessTrustedWithOptions(options: *const c_void) -> u8;
    static kAXTrustedCheckOptionPrompt: *const c_void;
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

/// 取当前处在最前面的普通应用窗口的进程号。
///
/// 用途是判断"焦点有没有真的交还出去"：面板隐藏后如果这个值仍然是自己的 pid，
/// 说明系统还没把目标应用切回前台，此时合成的 ⌘V 会落空。不是用
/// `NSWorkspace.frontmostApplication`，是为了不引入 Objective-C 绑定。
pub fn frontmost_window_owner_pid() -> Option<i32> {
    unsafe {
        let info = CGWindowListCopyWindowInfo(
            CG_WINDOW_LIST_ON_SCREEN_ONLY | CG_WINDOW_LIST_EXCLUDE_DESKTOP,
            0, // kCGNullWindowID
        );
        if info.is_null() {
            return None;
        }

        let layer_key = cf_string("kCGWindowLayer");
        let pid_key = cf_string("kCGWindowOwnerPID");
        let mut frontmost = None;

        for index in 0..CFArrayGetCount(info) {
            let window = CFArrayGetValueAtIndex(info, index);
            if window.is_null() {
                continue;
            }

            // 只看 layer 0 的普通窗口：菜单栏图标、悬浮面板的 layer 都大于 0。
            let mut layer: i32 = -1;
            let layer_ref = CFDictionaryGetValue(window, layer_key);
            if layer_ref.is_null()
                || CFNumberGetValue(
                    layer_ref,
                    CF_NUMBER_SINT32,
                    &mut layer as *mut i32 as *mut c_void,
                ) == 0
            {
                continue;
            }
            if layer != 0 {
                continue;
            }

            let mut pid: i32 = 0;
            let pid_ref = CFDictionaryGetValue(window, pid_key);
            if !pid_ref.is_null()
                && CFNumberGetValue(pid_ref, CF_NUMBER_SINT32, &mut pid as *mut i32 as *mut c_void) != 0
            {
                frontmost = Some(pid);
            }
            break;
        }

        if !layer_key.is_null() {
            CFRelease(layer_key);
        }
        if !pid_key.is_null() {
            CFRelease(pid_key);
        }
        CFRelease(info);

        frontmost
    }
}

/// 把 Rust 字符串转成 CFString，失败时返回空指针（调用方需判空后再释放）。
unsafe fn cf_string(value: &str) -> *const c_void {
    match CString::new(value) {
        Ok(c_str) => CFStringCreateWithCString(ptr::null(), c_str.as_ptr(), CF_STRING_ENCODING_UTF8),
        Err(_) => ptr::null(),
    }
}

/// 主动申请「辅助功能」权限，返回申请后（此刻）的授权状态。
///
/// 与只读的 [`has_accessibility_permission`] 的区别在于：这里会把
/// `kAXTrustedCheckOptionPrompt` 置为 true，系统据此弹出授权引导，并把**当前这份
/// 二进制**正式登记到「辅助功能」列表里。用户随后打开开关时，登记的才是真正
/// 会执行合成按键的进程，而不是升级前的旧副本。
///
/// 返回值是调用瞬间的状态：用户还没去系统设置里打开开关时仍然是 false。
pub fn request_accessibility_permission() -> bool {
    unsafe {
        let keys = [kAXTrustedCheckOptionPrompt];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const c_void,
            &kCFTypeDictionaryValueCallBacks as *const c_void,
        );
        if options.is_null() {
            return AXIsProcessTrusted() != 0;
        }
        let trusted = AXIsProcessTrustedWithOptions(options) != 0;
        CFRelease(options);
        trusted
    }
}

/// 清掉本应用在「辅助功能」里的历史授权记录。
///
/// 这是「系统设置里开关明明是绿的、程序却报无权限」的根治手段。TCC 的每条记录
/// 都绑定着一份「代码签名指定要求」：应用一旦重新签名（换证书，或 ad-hoc 重编译
/// 导致 cdhash 变化），新二进制就对不上旧记录，而列表里那条旧记录仍然显示为开启。
/// 只有删掉旧记录、再用当前二进制重新申请一次，系统才会按新签名新建一条能匹配的记录。
pub fn reset_accessibility_grant(bundle_id: &str) -> Result<(), String> {
    let output = std::process::Command::new("/usr/bin/tccutil")
        .args(["reset", "Accessibility", bundle_id])
        .output()
        .map_err(|error| format!("无法执行 tccutil：{error}"))?;

    if output.status.success() {
        return Ok(());
    }

    let stderr = String::from_utf8_lossy(&output.stderr).trim().to_string();
    Err(if stderr.is_empty() {
        format!("tccutil 退出码 {:?}", output.status.code())
    } else {
        stderr
    })
}

/// 读取某个 .app 的签名摘要（标识、cdhash、指定要求），用于日志排查。
///
/// 排查授权问题时最需要确认的是"系统认的到底是哪一份二进制"：cdhash 能唯一
/// 标识它，指定要求则说明这份签名会不会随重新编译而变化（ad-hoc 只有 cdhash，
/// 换证书后是 `identifier + certificate root`）。
pub fn signing_diagnostics(bundle: &std::path::Path) -> String {
    let output = std::process::Command::new("/usr/bin/codesign")
        .args(["-d", "--verbose=4", "-r-"])
        .arg(bundle)
        .output();

    let output = match output {
        Ok(output) => output,
        Err(_) => return "codesign 不可用".to_string(),
    };

    // codesign 的 -d 信息全部走 stderr
    let text = String::from_utf8_lossy(&output.stderr);
    let mut parts = Vec::new();
    for line in text.lines() {
        let line = line.trim();
        if line.starts_with("Identifier=")
            || line.starts_with("CDHash=")
            || line.starts_with("designated =>")
        {
            parts.push(line.to_string());
        }
    }

    if parts.is_empty() {
        "无法读取签名信息".to_string()
    } else {
        parts.join(" | ")
    }
}

/// 直接跳到「系统设置 → 隐私与安全性 → 辅助功能」。
///
/// 用 `open` 打开 URL 而不是自己拼 NSWorkspace：URL scheme 由系统解析，
/// 即使将来面板层级调整也由系统负责重定向，应用内不需要跟着改。
pub fn open_accessibility_settings() -> Result<(), String> {
    std::process::Command::new("open")
        .arg("x-apple.systempreferences:com.apple.preference.security?Privacy_Accessibility")
        .status()
        .map_err(|error| format!("打开系统设置失败：{error}"))?;
    Ok(())
}

// ── 快捷面板：跨空间 + 叠在全屏应用之上 ──────────────────────────────────
//
// 下面这些常量来自 AppKit 的 NSWindow.h。全屏叠加在 macOS 上不是"把窗口置顶"
// 那么简单，需要同时满足四件事，少一条都会在某个场景下静默失效：
//
//   1. 窗口必须是 NSPanel（普通 NSWindow 会被系统挡在别人的全屏空间之外）
//   2. 面板要带 nonactivatingPanel 样式，才能在不激活应用的前提下成为 key window
//   3. 窗口层级要高于全屏内容（NSFloatingWindowLevel 只有 3，不够）
//   4. collection behavior 要允许跟随当前空间并加入全屏空间

/// `NSWindowCollectionBehaviorCanJoinAllSpaces`：窗口可以出现在任何空间里。
const NS_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES: usize = 1 << 0;
/// `NSWindowCollectionBehaviorFullScreenAuxiliary`：窗口可以进入别人的全屏空间。
///
/// 只加这一个不够：全屏空间是系统另外创建的空间，窗口还得先"愿意跟随当前空间"
/// （即上面的 CanJoinAllSpaces），否则系统仍会把它留在原处。
const NS_COLLECTION_BEHAVIOR_FULL_SCREEN_AUXILIARY: usize = 1 << 8;

/// `NSWindowStyleMaskNonactivatingPanel`：面板成为 key window 时不激活所属应用。
///
/// 只对 NSPanel 生效。它正是"全屏应用里弹出面板却不切走全屏"的关键：应用一旦被
/// 激活，系统就会切到应用窗口所在的空间（也就是桌面），面板自然看起来"跑到桌面
/// 上了"。
const NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL: usize = 1 << 7;

/// `NSPopUpMenuWindowLevel`：弹出菜单层级（101），高于全屏应用内容与菜单栏之上
/// 的浮动层，系统状态项的浮层用的就是这个层级。
const NS_POPUP_MENU_WINDOW_LEVEL: i64 = 101;

#[link(name = "objc")]
extern "C" {
    /// 把选择子名字（如 `setCollectionBehavior:`）注册成 SEL。
    fn sel_registerName(name: *const c_char) -> *mut c_void;
    /// 按名字取类对象（如 `NSPanel`）。
    fn objc_getClass(name: *const c_char) -> *mut c_void;
    /// 直接改对象的类。NSPanel 没有在 NSWindow 之上新增实例变量，换类不会破坏
    /// 内存布局，这也是社区里把 Tauri 窗口变成面板的通行做法。
    fn object_setClass(obj: *mut c_void, cls: *mut c_void) -> *mut c_void;
    /// 运行时创建类与挂方法，用来给面板补回 `canBecomeKeyWindow`。
    fn objc_allocateClassPair(
        superclass: *mut c_void,
        name: *const c_char,
        extra_bytes: usize,
    ) -> *mut c_void;
    fn objc_registerClassPair(cls: *mut c_void);
    fn class_addMethod(
        cls: *mut c_void,
        name: *mut c_void,
        imp: *const c_void,
        types: *const c_char,
    ) -> i8;
    /// Objective-C 的消息发送入口。参数个数与类型由选择子决定，因此下面一律
    /// 先转成对应签名的函数指针再调用。
    fn objc_msgSend();
}

type MsgSend = unsafe extern "C" fn();

unsafe fn selector(name: &str) -> *mut c_void {
    match CString::new(name) {
        Ok(name) => sel_registerName(name.as_ptr()),
        Err(_) => ptr::null_mut(),
    }
}

unsafe fn class_named(name: &str) -> *mut c_void {
    match CString::new(name) {
        Ok(name) => objc_getClass(name.as_ptr()),
        Err(_) => ptr::null_mut(),
    }
}

/// `-[NSWindow canBecomeKeyWindow]` 的替身：恒为 YES。
///
/// 无边框窗口（decorations: false）默认不能成为 key window，也就收不到键盘
/// 事件。tao 是在自己的 `TaoWindow` 子类里用 `focusable` 实例变量实现这一点的，
/// 但那个类不是 NSPanel，换类后就丢了——必须在我们自己的面板子类里补回来。
extern "C" fn can_become_key_window(_this: *mut c_void, _cmd: *mut c_void) -> i8 {
    1
}

/// 懒创建一次面板类：`NSPanel` 的子类 `XpastePopupPanel`，只多一个
/// `canBecomeKeyWindow` 覆写。
///
/// 为什么不能直接用 `NSPanel`：把类换成 NSPanel 之后，实测
/// `canBecomeKeyWindow` 对无边框窗口仍返回 NO，面板会显示但打不了字。
/// 为什么不能改 `NSPanel` 本身：那是全局类，会影响同进程内所有面板。
fn popup_panel_class() -> Result<*mut c_void, String> {
    static CLASS: std::sync::OnceLock<usize> = std::sync::OnceLock::new();

    if let Some(class) = CLASS.get() {
        return Ok(*class as *mut c_void);
    }

    unsafe {
        let superclass = class_named("NSPanel");
        if superclass.is_null() {
            return Err("取不到 NSPanel 类".to_string());
        }
        let name = match CString::new("XpastePopupPanel") {
            Ok(name) => name,
            Err(_) => return Err("类名非法".to_string()),
        };

        let class = objc_allocateClassPair(superclass, name.as_ptr(), 0);
        if class.is_null() {
            return Err("创建面板类失败".to_string());
        }

        let types = CString::new("c@:").map_err(|error| error.to_string())?;
        let imp: *const c_void =
            can_become_key_window as extern "C" fn(*mut c_void, *mut c_void) -> i8 as *const c_void;
        class_addMethod(
            class,
            selector("canBecomeKeyWindow"),
            imp,
            types.as_ptr(),
        );
        objc_registerClassPair(class);

        // 并发调用时可能有一个线程先注册成功，另一个拿到 null；用第一个为准。
        let _ = CLASS.set(class as usize);
        Ok(CLASS.get().map_or(class, |stored| *stored as *mut c_void))
    }
}

/// 把快捷面板改造成"能叠在全屏应用上"的浮动面板。
///
/// 背景：本应用常驻托盘、Dock 隐藏后是 UIElement，没有自己的空间。用户在全屏应用
/// （浏览器、播放器、IDE）里按快捷键时，只设 collectionBehavior 是不够的——
/// 实测面板仍会被丢到桌面空间，因为普通 NSWindow 不被允许进入别人的全屏空间，
/// 而调用 `set_focus` 激活应用又会把系统切回桌面空间。
///
/// 因此这里一次做四件事：换成 NSPanel 子类、加上 nonactivatingPanel 样式（成为
/// key window 但不激活应用）、层级提到 NSPopUpMenuWindowLevel、补齐 collection
/// behavior。配合调用方不再调用 `set_focus`，面板才会"原地"浮在全屏内容之上。
///
/// 返回设置后的 collection behavior 位，供调用方写进日志核对。
///
/// 必须在主线程调用：AppKit 的对象不能在别的线程上收发消息。函数是幂等的，重复
/// 调用只会把同样的值再写一遍。
pub fn configure_popup_panel(ns_window: *mut c_void) -> Result<usize, String> {
    if ns_window.is_null() {
        return Err("窗口句柄为空".to_string());
    }

    unsafe {
        let panel_class = popup_panel_class()?;
        let send: MsgSend = objc_msgSend;
        let get_usize: unsafe extern "C" fn(*mut c_void, *mut c_void) -> usize =
            std::mem::transmute(send);
        let set_usize: unsafe extern "C" fn(*mut c_void, *mut c_void, usize) =
            std::mem::transmute(send);
        let set_i64: unsafe extern "C" fn(*mut c_void, *mut c_void, i64) =
            std::mem::transmute(send);
        let set_bool: unsafe extern "C" fn(*mut c_void, *mut c_void, i8) =
            std::mem::transmute(send);

        // 1. NSWindow -> NSPanel
        object_setClass(ns_window, panel_class);

        // 2. 保留原有样式位，只追加"非激活面板"
        let style = get_usize(ns_window, selector("styleMask"));
        set_usize(
            ns_window,
            selector("setStyleMask:"),
            style | NS_WINDOW_STYLE_MASK_NONACTIVATING_PANEL,
        );

        // 3. 面板默认会在应用失活时自动隐藏，这里显式关掉：
        //    本应用本来就不激活，交给快捷键显隐来控制。
        set_bool(ns_window, selector("setHidesOnDeactivate:"), 0);

        // 4. 层级与空间行为
        set_i64(
            ns_window,
            selector("setLevel:"),
            NS_POPUP_MENU_WINDOW_LEVEL,
        );

        let behavior_selector = selector("collectionBehavior");
        let behavior = get_usize(ns_window, behavior_selector)
            | NS_COLLECTION_BEHAVIOR_CAN_JOIN_ALL_SPACES
            | NS_COLLECTION_BEHAVIOR_FULL_SCREEN_AUXILIARY;
        set_usize(ns_window, selector("setCollectionBehavior:"), behavior);

        Ok(behavior)
    }
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
