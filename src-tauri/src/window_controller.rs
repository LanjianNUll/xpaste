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
        // 一次收起整套：面板和编辑器是一组的，留着编辑器单独飘着没有意义
        hide_panels(app);
        return Ok(());
    }

    #[cfg(target_os = "windows")]
    remember_foreground_window(&window);

    let (x, y) = popup_position(&window);
    apply_popup_position(&window, x, y)?;
    // 跨越不同缩放比例的显示器后，窗口外框尺寸可能随 DPI 更新，再校正一次位置。
    let (corrected_x, corrected_y) = popup_position(&window);
    if (corrected_x, corrected_y) != (x, y) {
        apply_popup_position(&window, corrected_x, corrected_y)?;
    }
    window.show().map_err(|error| error.to_string())?;
    // macOS：面板是 nonactivating 的，show() 里的 makeKeyAndOrderFront 已经让它
    // 拿到键盘焦点，而且不会激活应用。再调 set_focus 会走一遍
    // `[NSApp activateIgnoringOtherApps:]`，系统随即切到应用窗口所在的桌面空间，
    // 全屏应用里的面板看起来就"跑到桌面上去了"。
    #[cfg(not(target_os = "macos"))]
    window.set_focus().map_err(|error| error.to_string())?;
    // 通知前端做"每次打开都复位"（清空搜索词、聚焦搜索框、列表回顶）。
    // 这里必须用广播 emit：前端 listen() 默认把目标注册为 Any，而 emit_to 走的是
    // AnyLabel 标签匹配，Any 候选不会被命中，指定标签反而收不到事件。
    let _ = tauri::Emitter::emit(app, "popup://shown", ());
    Ok(())
}

/// 调整快捷面板尺寸。
///
/// 早期的编辑态是把面板本身拉宽（360×500 → 760×560），那套做法有两个毛病：
/// macOS 上 tao 的 `set_size` 走 `DispatchQueue::main().exec_async`，命令返回时
/// 外框还是旧值，得轮询等它生效；即便等到了，用户也会看到窗口先向右下长出去、
/// 再整体跳到鼠标处。现在编辑改成独立的 editor 窗口，面板尺寸固定，这里自然
/// 也就不需要了。
///
/// 保留这个函数是因为主窗口仍有程序化改尺寸的可能，但快捷面板已经不调用。
#[allow(dead_code)]
pub async fn resize_popup(app: &AppHandle, width: f64, height: f64) -> Result<(), String> {
    let window = app
        .get_webview_window("popup")
        .ok_or_else(|| "快捷窗口不存在".to_string())?;
    window
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|error| error.to_string())?;

    let scale = window.scale_factor().unwrap_or(1.0);
    let scale = if scale > 0.0 { scale } else { 1.0 };
    for _ in 0..30 {
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        if let Ok(size) = window.outer_size() {
            let applied_width = f64::from(size.width) / scale;
            let applied_height = f64::from(size.height) / scale;
            if (applied_width - width).abs() < 2.0 && (applied_height - height).abs() < 2.0 {
                break;
            }
        }
    }

    let (x, y) = popup_position(&window);
    apply_popup_position(&window, x, y)?;
    let (corrected_x, corrected_y) = popup_position(&window);
    if (corrected_x, corrected_y) != (x, y) {
        apply_popup_position(&window, corrected_x, corrected_y)?;
    }
    Ok(())
}

/// 焦点是否还在本应用的浮层上。
///
/// 面板失焦时要区分两种情况：焦点去了自己的编辑器窗口（同一个编辑会话，面板
/// 必须留着），还是用户真的切去了别的应用（整套收起）。只看 webview 的 blur
/// 事件分不出这两者——自己的另一个窗口抢焦点，同样会触发 blur。
///
/// 这里用窗口的 key 状态判断，而不是"最前面的窗口属于哪个进程"：面板被设成
/// NSPopUpMenuWindowLevel(101)，在 CGWindowList 里不在 layer 0，按进程去查会
/// 永远查不到自己，进而误判成"用户切走了"。`isKeyWindow` 与层级无关，对
/// nonactivating 面板同样有效。
pub fn own_window_is_frontmost(app: &AppHandle) -> bool {
    ["popup", "editor"].iter().any(|label| {
        app.get_webview_window(label)
            .and_then(|window| window.is_focused().ok())
            .unwrap_or(false)
    })
}

/// 编辑器窗口宽度。与快捷面板（360）并排时，两个窗口加起来约 790 逻辑像素，
/// 13 寸屏也放得下。
pub const EDITOR_WIDTH: f64 = 420.0;
/// 编辑器右侧历史栏的宽度。展开后整窗 680，加上面板共约 1050，1280 宽的屏仍放得下。
pub const EDITOR_SIDEBAR_WIDTH: f64 = 260.0;
/// 展开侧栏后允许压到的最窄宽度，必须与样式里的 `.editor-main`(260) +
/// `.editor-sidebar`(200) 对得上：再窄主编辑区就会被挤没，或者侧栏被裁掉。
const EDITOR_MIN_WIDTH: f64 = 460.0;
/// 窗口与屏幕边缘之间留的余量
const SCREEN_EDGE_MARGIN: f64 = 8.0;
/// 两个窗口之间的留白
const PANEL_GAP: f64 = 8.0;

/// 打开编辑器窗口，贴在快捷面板右侧。
///
/// 面板本身不动——这正是这一版和上一版的根本区别：上一版把同一个窗口从 360
/// 拉宽到 760 再重新定位，一次点击能看到两三次位移；这里所有尺寸与位置都在
/// `show()` 之前设置好，窗口只会"直接出现在那里"。
///
/// 位置优先放右侧；右侧装不下翻到左侧；左右都装不下退到面板下方；再不行就
/// 贴屏幕边缘（允许与面板重叠，至少保证整窗可见）。
pub fn open_editor(app: &AppHandle) -> Result<(), String> {
    let popup = app
        .get_webview_window("popup")
        .ok_or_else(|| "快捷窗口不存在".to_string())?;
    let editor = app
        .get_webview_window("editor")
        .ok_or_else(|| "编辑器窗口不存在".to_string())?;

    // 高度与面板齐平，两个窗口上沿对齐看起来才像一组
    let scale = popup.scale_factor().unwrap_or(1.0);
    let scale = if scale > 0.0 { scale } else { 1.0 };
    let height = popup
        .outer_size()
        .map(|size| f64::from(size.height) / scale)
        .unwrap_or(500.0);
    let height = if height > 1.0 { height } else { 500.0 };

    editor
        .set_size(tauri::LogicalSize::new(EDITOR_WIDTH, height))
        .map_err(|error| error.to_string())?;

    let (x, y) = editor_position(&popup, EDITOR_WIDTH, height, scale);
    editor
        .set_position(tauri::LogicalPosition::new(x, y))
        .map_err(|error| error.to_string())?;

    editor.show().map_err(|error| error.to_string())?;
    let _ = tauri::Emitter::emit(app, "editor://opened", ());
    Ok(())
}

/// 关闭编辑器窗口，把键盘焦点还给快捷面板。
///
/// 不用 `set_focus`：macOS 上它会走 `activateIgnoringOtherApps`，激活应用会让
/// 系统切回应用所在的桌面空间。面板本身是 nonactivating panel，`show()` 内部的
/// makeKeyAndOrderFront 足以让它重新成为 key window。
pub fn close_editor(app: &AppHandle) -> Result<(), String> {
    let editor = app
        .get_webview_window("editor")
        .ok_or_else(|| "编辑器窗口不存在".to_string())?;
    editor.hide().map_err(|error| error.to_string())?;

    if let Some(popup) = app.get_webview_window("popup") {
        if popup.is_visible().unwrap_or(false) {
            let _ = popup.show();
        }
    }
    Ok(())
}

/// 展开 / 收起编辑器右侧的历史栏。
///
/// 尺寸与位置的规则：窗口只朝**背离快捷面板**的方向长，贴着面板的那条边一动不动。
/// 面板在编辑器左边时向右长（常规情况），面板在编辑器右边时向左长——否则展开
/// 侧栏会正好盖住用户正要点击的那一列素材。屏幕放不下时收窄侧栏，也不越过面板。
pub fn set_editor_sidebar(app: &AppHandle, open: bool) -> Result<(), String> {
    let editor = app
        .get_webview_window("editor")
        .ok_or_else(|| "编辑器窗口不存在".to_string())?;

    let scale = positive_scale(editor.scale_factor().unwrap_or(1.0));
    let position = editor.outer_position().map_err(|error| error.to_string())?;
    let size = editor.outer_size().map_err(|error| error.to_string())?;
    let left = f64::from(position.x) / scale;
    let top = f64::from(position.y) / scale;
    let right = left + f64::from(size.width) / scale;
    let height = {
        let value = f64::from(size.height) / scale;
        if value > 1.0 {
            value
        } else {
            500.0
        }
    };

    // 编辑器整体落在面板左侧时，向左长；其余情况（含重叠的极端布局）向右长。
    let grow_left = popup_left_edge(app).is_some_and(|popup_left| right <= popup_left + 1.0);

    let width = if open {
        clamp_growth(
            &editor,
            if grow_left { right } else { left },
            EDITOR_WIDTH + EDITOR_SIDEBAR_WIDTH,
            grow_left,
        )
    } else {
        // 收起就是回到默认宽度，不需要钳制：窗口本来就在屏幕里，只会变小。
        EDITOR_WIDTH
    };
    let next_left = if grow_left { right - width } else { left };

    editor
        .set_size(tauri::LogicalSize::new(width, height))
        .map_err(|error| error.to_string())?;
    // 有的平台改尺寸是绕中心缩放，改完再把固定边放回原处，保证只有一条边在动。
    editor
        .set_position(tauri::LogicalPosition::new(next_left, top))
        .map_err(|error| error.to_string())?;
    Ok(())
}

/// 面板左边缘的逻辑坐标。拿不到就返回 None，调用方按常规方向处理。
fn popup_left_edge(app: &AppHandle) -> Option<f64> {
    let popup = app.get_webview_window("popup")?;
    let scale = positive_scale(popup.scale_factor().ok()?);
    popup
        .outer_position()
        .ok()
        .map(|position| f64::from(position.x) / scale)
}

/// 把目标宽度钳到「从固定边到屏幕边」的范围里，最少保留一个能用的编辑区。
///
/// `edge` 是窗口固定不动的那条边：向右长时是左边缘，向左长时是右边缘。
fn clamp_growth(editor: &tauri::WebviewWindow, edge: f64, target: f64, grow_left: bool) -> f64 {
    let Some(monitor) = editor.current_monitor().ok().flatten() else {
        return target;
    };
    let area_scale = positive_scale(monitor.scale_factor());
    let area_left = f64::from(monitor.position().x) / area_scale;
    let area_right =
        (f64::from(monitor.position().x) + f64::from(monitor.size().width)) / area_scale;
    let available = if grow_left {
        edge - area_left - SCREEN_EDGE_MARGIN
    } else {
        area_right - edge - SCREEN_EDGE_MARGIN
    };
    target.min(available).max(EDITOR_MIN_WIDTH)
}

/// 同时收起两个窗口。投递前必须两个都收——否则焦点还留在自己的某个窗口上，
/// 合成的粘贴按键会打空。
pub fn hide_panels(app: &AppHandle) {
    if let Some(popup) = app.get_webview_window("popup") {
        let _ = popup.hide();
    }
    if let Some(editor) = app.get_webview_window("editor") {
        let _ = editor.hide();
    }
}

/// 计算编辑器窗口在屏幕上的逻辑坐标。
fn editor_position(
    popup: &tauri::WebviewWindow,
    width: f64,
    height: f64,
    scale: f64,
) -> (f64, f64) {
    let (Ok(position), Ok(size)) = (popup.outer_position(), popup.outer_size()) else {
        return (100.0, 100.0);
    };

    let panel_left = f64::from(position.x) / scale;
    let panel_top = f64::from(position.y) / scale;
    let panel_width = f64::from(size.width) / scale;
    let panel_height = f64::from(size.height) / scale;

    // 以面板所在显示器的工作区做边界。取不到就退化成"紧贴右侧"，不做钳制。
    let Some(monitor) = popup.current_monitor().ok().flatten() else {
        return (panel_left + panel_width + PANEL_GAP, panel_top);
    };
    let area = monitor.work_area();
    let area_scale = {
        let value = monitor.scale_factor();
        if value > 0.0 {
            value
        } else {
            1.0
        }
    };
    let area_left = f64::from(area.position.x) / area_scale;
    let area_top = f64::from(area.position.y) / area_scale;
    let area_right = (f64::from(area.position.x) + f64::from(area.size.width)) / area_scale;
    let area_bottom = (f64::from(area.position.y) + f64::from(area.size.height)) / area_scale;

    let right = panel_left + panel_width + PANEL_GAP;
    let left = panel_left - PANEL_GAP - width;
    let x = if right + width <= area_right {
        right
    } else if left >= area_left {
        left
    } else {
        // 左右都放不下：优先放在面板下方，仍不行就贴右边缘（允许重叠）
        let below = panel_top + panel_height + PANEL_GAP;
        if below + height <= area_bottom {
            return (panel_left.clamp(area_left, (area_right - width).max(area_left)), below);
        }
        (area_right - width).max(area_left)
    };

    let y = panel_top.clamp(area_top, (area_bottom - height).max(area_top));
    (x, y)
}

/// Windows 下用物理像素定位，其余平台用逻辑坐标定位。
#[cfg(target_os = "windows")]
fn apply_popup_position(window: &tauri::WebviewWindow, x: i32, y: i32) -> Result<(), String> {
    window
        .set_position(tauri::PhysicalPosition::new(x, y))
        .map_err(|error| error.to_string())
}

/// macOS 的鼠标位置（CGEvent）与显示器查询（CGDisplayBounds）都工作在逻辑点
/// 坐标系里，直接按逻辑坐标下发即可，避免多一次乘除缩放带来偏移。
#[cfg(not(target_os = "windows"))]
fn apply_popup_position(window: &tauri::WebviewWindow, x: i32, y: i32) -> Result<(), String> {
    window
        .set_position(tauri::LogicalPosition::new(f64::from(x), f64::from(y)))
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

/// macOS / Linux：把快捷窗口放到鼠标右下方，并用鼠标所在显示器的工作区做边界约束。
#[cfg(not(target_os = "windows"))]
fn popup_position(window: &tauri::WebviewWindow) -> (i32, i32) {
    let Some((cursor_x, cursor_y)) = crate::platform::cursor_position() else {
        return (100, 100);
    };

    let scale = positive_scale(window.scale_factor().unwrap_or(1.0));
    let window_size = window
        .outer_size()
        .unwrap_or(tauri::PhysicalSize::new(360, 500));
    let window_width = f64::from(window_size.width) / scale;
    let window_height = f64::from(window_size.height) / scale;

    // monitor_from_point 在 macOS 上按 CGDisplayBounds 判定，接收的正是逻辑坐标。
    let bounds = window
        .monitor_from_point(cursor_x, cursor_y)
        .ok()
        .flatten()
        .or_else(|| window.current_monitor().ok().flatten())
        .map(|monitor| {
            let scale = positive_scale(monitor.scale_factor());
            let position = monitor.position();
            let size = monitor.size();
            let left = f64::from(position.x) / scale;
            let top = f64::from(position.y) / scale;
            (
                left,
                top,
                left + f64::from(size.width) / scale,
                top + f64::from(size.height) / scale,
            )
        });

    let (left, top, right, bottom) = bounds.unwrap_or((f64::MIN, f64::MIN, f64::MAX, f64::MAX));
    let x = (cursor_x + 10.0).clamp(left, (right - window_width).max(left));
    let y = (cursor_y + 10.0).clamp(top, (bottom - window_height).max(top));
    (x.round() as i32, y.round() as i32)
}

#[cfg(not(target_os = "windows"))]
fn positive_scale(scale: f64) -> f64 {
    if scale > 0.0 {
        scale
    } else {
        1.0
    }
}
