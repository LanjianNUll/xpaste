// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod classify;
mod clipboard;
mod db;
mod history_repository;
mod keyboard_hook;
#[cfg(target_os = "macos")]
mod macos;
mod models;
mod platform;
mod sdk;
mod window_controller;

use tauri::{Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::models::{ClipboardItem, HistoryPage, HistoryPageQuery};
use crate::sdk::{ClipboardSdk, ItemSource};

/// 平台无关的快捷键描述，解析自用户在设置里填写的字符串。
///
/// 同一个字符串（例如 `Cmd+Shift+V`）在 Windows 上会落到低级键盘钩子，
/// 在 macOS 上会落到系统全局快捷键，因此这里只保留语义，不保留虚拟键码。
struct HotkeySpec {
    /// 主键，目前仅支持字母 A-Z。
    letter: char,
    ctrl: bool,
    alt: bool,
    shift: bool,
    /// Windows 徽标键 / macOS Command 键。
    meta: bool,
}

impl HotkeySpec {
    fn parse(hotkey: &str) -> Self {
        let mut spec = HotkeySpec {
            letter: 'V',
            ctrl: false,
            alt: false,
            shift: false,
            meta: false,
        };

        for part in hotkey.split('+') {
            match part.trim().to_uppercase().as_str() {
                "CTRL" | "CONTROL" => spec.ctrl = true,
                "ALT" | "OPTION" => spec.alt = true,
                "SHIFT" => spec.shift = true,
                "SUPER" | "WIN" | "CMD" | "COMMAND" | "META" => spec.meta = true,
                token => {
                    if let Some(letter) = single_letter(token) {
                        spec.letter = letter;
                    }
                }
            }
        }

        spec
    }

    /// 当前平台的默认快捷键。
    ///
    /// Windows 上抢占系统 Win+V 是设计目标（覆盖原生剪贴板历史）；
    /// macOS 上 ⌘V 是所有应用的粘贴键，全局抢占会破坏正常粘贴，
    /// 因此默认改用 ⌘⇧V——这也是 Mac 剪贴板管理器的通行做法。
    fn default_for_platform() -> &'static str {
        if cfg!(target_os = "windows") {
            "Win+V"
        } else if cfg!(target_os = "macos") {
            "Cmd+Shift+V"
        } else {
            "Ctrl+Shift+V"
        }
    }

    /// Windows 虚拟键码，A-Z 恰好对应 0x41-0x5A。
    #[cfg(target_os = "windows")]
    fn virtual_key_code(&self) -> u32 {
        self.letter as u32
    }
}

#[cfg(not(target_os = "windows"))]
impl HotkeySpec {
    /// 映射到 global-hotkey 的按键枚举。
    fn code(&self) -> Option<tauri_plugin_global_shortcut::Code> {
        use tauri_plugin_global_shortcut::Code;

        Some(match self.letter {
            'A' => Code::KeyA,
            'B' => Code::KeyB,
            'C' => Code::KeyC,
            'D' => Code::KeyD,
            'E' => Code::KeyE,
            'F' => Code::KeyF,
            'G' => Code::KeyG,
            'H' => Code::KeyH,
            'I' => Code::KeyI,
            'J' => Code::KeyJ,
            'K' => Code::KeyK,
            'L' => Code::KeyL,
            'M' => Code::KeyM,
            'N' => Code::KeyN,
            'O' => Code::KeyO,
            'P' => Code::KeyP,
            'Q' => Code::KeyQ,
            'R' => Code::KeyR,
            'S' => Code::KeyS,
            'T' => Code::KeyT,
            'U' => Code::KeyU,
            'V' => Code::KeyV,
            'W' => Code::KeyW,
            'X' => Code::KeyX,
            'Y' => Code::KeyY,
            'Z' => Code::KeyZ,
            _ => return None,
        })
    }
}

fn single_letter(token: &str) -> Option<char> {
    let mut chars = token.chars();
    let letter = chars.next()?;
    if chars.next().is_some() || !letter.is_ascii_alphabetic() {
        return None;
    }
    Some(letter.to_ascii_uppercase())
}

/// 让快捷键立即生效。
///
/// Windows 走低级键盘钩子：可以抢占系统已占用的 Win 组合键；
/// 其余平台走 tauri-plugin-global-shortcut（macOS 底层是 Carbon 热键注册）。
#[cfg(target_os = "windows")]
fn apply_hotkey(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    let spec = HotkeySpec::parse(hotkey);
    let app_handle = app.clone();

    keyboard_hook::register_hotkey(
        spec.virtual_key_code(),
        spec.ctrl,
        spec.alt,
        spec.shift,
        spec.meta,
        move || {
            let handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let _ = window_controller::toggle_popup(&handle);
            });
        },
    )
}

#[cfg(not(target_os = "windows"))]
fn apply_hotkey(app: &tauri::AppHandle, hotkey: &str) -> Result<(), String> {
    use tauri_plugin_global_shortcut::{GlobalShortcutExt, Modifiers, Shortcut};

    let spec = HotkeySpec::parse(hotkey);
    let code = spec
        .code()
        .ok_or_else(|| format!("不支持的按键（目前仅支持 A-Z）：{hotkey}"))?;

    let mut modifiers = Modifiers::empty();
    if spec.ctrl {
        modifiers |= Modifiers::CONTROL;
    }
    if spec.alt {
        modifiers |= Modifiers::ALT;
    }
    if spec.shift {
        modifiers |= Modifiers::SHIFT;
    }
    if spec.meta {
        modifiers |= Modifiers::SUPER;
    }

    let shortcut = Shortcut::new((!modifiers.is_empty()).then_some(modifiers), code);
    let manager = app.global_shortcut();
    // 换绑前先清掉旧组合键，否则旧键会一直留在系统里，两个键都能唤起面板。
    let _ = manager.unregister_all();

    manager
        .on_shortcut(shortcut, move |app, _shortcut, event| {
            // 快捷键的按下和松开各回调一次：只在按下时唤起面板。
            // 若不过滤，松开时会再 toggle 一次，面板刚弹出就被关掉，
            // 表现为「按住快捷键才能看到面板，一松手就消失」。
            if event.state() != tauri_plugin_global_shortcut::ShortcutState::Pressed {
                return;
            }
            let handle = app.clone();
            tauri::async_runtime::spawn(async move {
                let _ = window_controller::toggle_popup(&handle);
            });
        })
        .map_err(|error| format!("注册全局快捷键失败：{error}"))
}

struct AppState {
    sdk: ClipboardSdk,
}

#[tauri::command]
async fn list_history(
    state: State<'_, AppState>,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
    state.sdk.list_history("", limit).await
}

#[tauri::command]
async fn search_history(
    state: State<'_, AppState>,
    query: String,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
    state.sdk.list_history(&query, limit).await
}

#[tauri::command]
async fn get_history_page(
    state: State<'_, AppState>,
    query: HistoryPageQuery,
) -> Result<HistoryPage, String> {
    state.sdk.history_page(query).await
}

#[tauri::command]
async fn set_clipboard(
    state: State<'_, AppState>,
    id: i64,
    source: Option<String>,
) -> Result<(), String> {
    state
        .sdk
        .copy_item(ItemSource::parse(source.as_deref()), id)
        .await
}

#[tauri::command]
async fn get_clipboard_image(
    state: State<'_, AppState>,
    id: i64,
    thumbnail: Option<bool>,
    source: Option<String>,
) -> Result<String, String> {
    state
        .sdk
        .image_base64(ItemSource::parse(source.as_deref()), id, thumbnail.unwrap_or(false))
        .await
}

#[tauri::command]
async fn save_clipboard_text(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    text: String,
) -> Result<(), String> {
    state.sdk.save_text(text).await?;
    let _ = tauri::Emitter::emit(&app, "clipboard://updated", ());
    Ok(())
}

#[tauri::command]
async fn save_clipboard_image(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    image_base64: String,
) -> Result<(), String> {
    state.sdk.save_png_base64(image_base64).await?;
    let _ = tauri::Emitter::emit(&app, "clipboard://updated", ());
    Ok(())
}

#[tauri::command]
async fn clear_history(state: State<'_, AppState>) -> Result<(), String> {
    state.sdk.clear_history().await
}

#[tauri::command]
async fn delete_history_item(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.sdk.delete_item(id).await
}

#[tauri::command]
async fn delete_history_items(state: State<'_, AppState>, ids: Vec<i64>) -> Result<u64, String> {
    state.sdk.delete_items(&ids).await
}

#[tauri::command]
async fn delete_history_by_format(
    state: State<'_, AppState>,
    format: String,
) -> Result<u64, String> {
    state.sdk.delete_by_format(&format).await
}

#[tauri::command]
async fn delete_history_by_category(
    state: State<'_, AppState>,
    category: String,
) -> Result<u64, String> {
    state.sdk.delete_by_category(&category).await
}

#[tauri::command]
async fn delete_history_by_date(
    state: State<'_, AppState>,
    start_ts: i64,
    end_ts: i64,
) -> Result<u64, String> {
    state.sdk.delete_by_date(start_ts, end_ts).await
}

#[tauri::command]
async fn get_format_stats(state: State<'_, AppState>) -> Result<Vec<(String, i64)>, String> {
    state.sdk.format_stats().await
}

#[tauri::command]
async fn get_category_stats(state: State<'_, AppState>) -> Result<Vec<(String, i64)>, String> {
    state.sdk.category_stats().await
}

#[tauri::command]
async fn toggle_favorite(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<bool, String> {
    let favorited = state.sdk.toggle_favorite(id).await?;
    // 复用数据变更广播：两个窗口的既有监听会各自刷新列表和星标状态。
    let _ = tauri::Emitter::emit(&app, "clipboard://updated", ());
    Ok(favorited)
}

#[tauri::command]
async fn remove_favorite(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    state.sdk.remove_favorite(id).await?;
    let _ = tauri::Emitter::emit(&app, "clipboard://updated", ());
    Ok(())
}

#[tauri::command]
async fn list_favorites(
    state: State<'_, AppState>,
    query: Option<String>,
    format: Option<String>,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
    state
        .sdk
        .list_favorites(query.as_deref().unwrap_or(""), format, limit)
        .await
}

/// Windows：隐藏面板 → 把焦点还给呼出面板前的窗口 → 模拟 Ctrl+V。
///
/// 抽成独立函数是因为内容来源有两条：历史记录（按 id 复制）和编辑器里的
/// 任意文本。投递这一段完全一致，差别只在往剪贴板里塞什么。
#[cfg(target_os = "windows")]
async fn deliver_to_frontmost(app: &tauri::AppHandle) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };

    // 编辑器和面板都要收起：焦点留在自己的任一窗口上，合成的按键都会打空
    window_controller::hide_panels(app);
    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;
    window_controller::restore_previous_window()?;

    // 等待剪贴板写入完成
    tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;

    // 模拟 Ctrl+V 按键
    unsafe {
        let inputs = [
            // 按下 Ctrl
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: Default::default(),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            // 按下 V
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_V,
                        wScan: 0,
                        dwFlags: Default::default(),
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            // 释放 V
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_V,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
            // 释放 Ctrl
            INPUT {
                r#type: INPUT_KEYBOARD,
                Anonymous: INPUT_0 {
                    ki: KEYBDINPUT {
                        wVk: VK_CONTROL,
                        wScan: 0,
                        dwFlags: KEYEVENTF_KEYUP,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            },
        ];

        let result = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
        if result == 0 {
            return Err("模拟按键失败".to_string());
        }
    }

    Ok(())
}

/// macOS：写入剪贴板 → 隐藏面板并把焦点交还上一个应用 → 模拟 ⌘V。
///
/// macOS 没有 Windows 那种"记住前台窗口再抢回来"的可靠做法，改用系统自带的
/// `hide()`：它会把整个应用藏起来，系统随即把焦点还给上一次的前台应用，
/// 之后合成的 ⌘V 才会落进用户原本的输入框而不是我们自己。
#[cfg(target_os = "macos")]
async fn deliver_to_frontmost(app: &tauri::AppHandle) -> Result<(), String> {
    // 没有辅助功能权限时系统会静默丢弃合成按键，表现为"点了没反应"。
    // 这里刻意不隐藏面板：用户必须看到这条提示，才知道要去系统设置里授权。
    // 内容已经进了剪贴板，此时手动按 ⌘V 依然可用。
    let trusted = macos::has_accessibility_permission();
    clipboard::log_line(&format!("paste: accessibility_trusted={trusted}"));
    if !trusted {
        return Err(
            "已复制到剪贴板。要自动粘贴，请到「设置 → 通用 → 自动粘贴权限」点「一键修复」。"
                .to_string(),
        );
    }

    let self_pid = std::process::id() as i32;
    clipboard::log_line(&format!(
        "paste: frontmost_before={:?} self_pid={self_pid}",
        macos::frontmost_window_owner_pid()
    ));

    window_controller::hide_panels(app);
    let _ = app.hide();

    // 焦点必须真的交还出去，合成的 ⌘V 才会落进用户原本的输入框。
    // 固定 sleep 在慢一点的情况下会抢跑——按键发给还处于前台的自己，于是"点了没反应"。
    // 这里改成轮询前台窗口的 pid，确认换台之后再发按键。
    let mut waited_ms = 0u64;
    loop {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        waited_ms += 20;

        match macos::frontmost_window_owner_pid() {
            Some(pid) if pid != self_pid => {
                clipboard::log_line(&format!("paste: focus returned after {waited_ms}ms (pid={pid})"));
                break;
            }
            _ if waited_ms >= 800 => {
                clipboard::log_line(&format!(
                    "paste: focus not returned after {waited_ms}ms, frontmost={:?}",
                    macos::frontmost_window_owner_pid()
                ));
                break;
            }
            _ => {}
        }
    }

    // 前台切换完成后留一点余量，避开目标应用还在处理激活的那一小段时间。
    tokio::time::sleep(tokio::time::Duration::from_millis(80)).await;

    let result = macos::simulate_paste();
    clipboard::log_line(&format!("paste: simulate_paste -> {result:?}"));
    result
}

/// 其余平台暂不支持模拟按键，退化为写入剪贴板并收起面板。
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
async fn deliver_to_frontmost(app: &tauri::AppHandle) -> Result<(), String> {
    window_controller::hide_panels(app);
    Ok(())
}

/// 把一条历史记录写进剪贴板，再投递到用户原本所在的窗口。
#[tauri::command]
async fn set_clipboard_and_paste(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
    source: Option<String>,
) -> Result<(), String> {
    state
        .sdk
        .copy_item(ItemSource::parse(source.as_deref()), id)
        .await?;
    deliver_to_frontmost(&app).await
}

/// 把任意文本写进系统剪贴板（编辑器送出时用，不经过历史记录）。
fn write_text_to_clipboard(text: &str) -> Result<(), String> {
    let mut clipboard =
        arboard::Clipboard::new().map_err(|err| format!("写入剪贴板失败：{err}"))?;
    clipboard
        .set_text(text.to_string())
        .map_err(|err| format!("写入剪贴板失败：{err}"))
}

/// 编辑器送出：写入任意文本 → 投递到原应用。
#[tauri::command]
async fn paste_text(app: tauri::AppHandle, text: String) -> Result<(), String> {
    write_text_to_clipboard(&text)?;
    deliver_to_frontmost(&app).await
}

/// 编辑器「仅复制」：只写剪贴板，不模拟按键，也不收起面板。
#[tauri::command]
fn copy_text(text: String) -> Result<(), String> {
    write_text_to_clipboard(&text)
}

/// 取一条记录的完整文本。列表返回的是截断版，编辑器插入素材时必须拿原文。
#[tauri::command]
async fn get_item_text(
    state: State<'_, AppState>,
    id: i64,
    source: Option<String>,
) -> Result<String, String> {
    state
        .sdk
        .item_text(ItemSource::parse(source.as_deref()), id)
        .await
}

/// 收起整套浮层（面板 + 编辑器）。Esc、标题栏关闭、失焦走同一条路径。
#[tauri::command]
fn hide_popup(app: tauri::AppHandle) -> Result<(), String> {
    window_controller::hide_panels(&app);
    Ok(())
}

/// 打开编辑器窗口：贴在快捷面板右侧，面板本身不动。
#[tauri::command]
fn open_editor(app: tauri::AppHandle) -> Result<(), String> {
    window_controller::open_editor(&app)
}

/// 关闭编辑器窗口，把键盘焦点还给快捷面板。
#[tauri::command]
fn close_editor(app: tauri::AppHandle) -> Result<(), String> {
    window_controller::close_editor(&app)
}

/// 展开 / 收起编辑器右侧的历史栏（窗口朝背离面板的方向变宽，贴着面板的那条边不动）。
#[tauri::command]
fn set_editor_sidebar(app: tauri::AppHandle, open: bool) -> Result<(), String> {
    window_controller::set_editor_sidebar(&app, open)
}

/// 编辑器是否开着。面板失焦时要靠它判断"焦点是去了编辑器，还是去了别的应用"。
#[tauri::command]
fn is_editor_visible(app: tauri::AppHandle) -> bool {
    app.get_webview_window("editor")
        .and_then(|window| window.is_visible().ok())
        .unwrap_or(false)
}

/// 焦点是否还在本应用的浮层上。用于区分"焦点去了自己的编辑器"和"切到了别的应用"。
#[tauri::command]
fn is_own_window_frontmost(app: tauri::AppHandle) -> bool {
    window_controller::own_window_is_frontmost(&app)
}

#[tauri::command]
async fn list_history_by_date(
    state: State<'_, AppState>,
    start_ts: i64,
    end_ts: i64,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
    state
        .sdk
        .list_history_by_date("", start_ts, end_ts, limit)
        .await
}

#[tauri::command]
async fn search_history_by_date(
    state: State<'_, AppState>,
    query: String,
    start_ts: i64,
    end_ts: i64,
    limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
    state
        .sdk
        .list_history_by_date(&query, start_ts, end_ts, limit)
        .await
}

#[tauri::command]
async fn get_cursor_position() -> Result<(i32, i32), String> {
    platform::cursor_position()
        .map(|(x, y)| (x as i32, y as i32))
        .ok_or_else(|| "当前平台不支持读取鼠标位置".to_string())
}

#[tauri::command]
async fn get_hotkey(app: tauri::AppHandle) -> Result<String, String> {
    // 从配置文件读取快捷键，如果不存在则返回当前平台的默认值
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let config_file = config_dir.join("hotkey.txt");

    if config_file.exists() {
        std::fs::read_to_string(config_file).map_err(|e| e.to_string())
    } else {
        Ok(HotkeySpec::default_for_platform().to_string())
    }
}

#[tauri::command]
async fn set_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    // 保存快捷键到配置文件
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let config_file = config_dir.join("hotkey.txt");

    std::fs::write(config_file, &hotkey).map_err(|e| e.to_string())?;

    // 立即换绑，不必等下次启动
    apply_hotkey(&app, &hotkey)
}

/// 当前是否已被授予「辅助功能」权限（只查询，不弹窗）。
///
/// 供前端在设置页/错误提示里判断要不要给"去授权"入口，非 macOS 平台恒为 true。
#[tauri::command]
fn check_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::has_accessibility_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// 主动申请「辅助功能」权限并返回当前状态。
///
/// 会触发系统授权引导。必须走这个接口而不是让用户自己去列表里翻开关：
/// 只有这样系统才把当前这份二进制登记进去，避免"开关是开的但程序仍无权限"。
#[tauri::command]
fn request_accessibility_permission() -> bool {
    #[cfg(target_os = "macos")]
    {
        macos::request_accessibility_permission()
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

/// 一键修复「辅助功能」授权的结果。
#[derive(serde::Serialize)]
struct AccessibilityRepair {
    /// 修复动作执行完之后此刻的授权状态
    trusted: bool,
    /// 面向用户的说明，界面直接展示，不需要再自行拼文案
    message: String,
}

/// 一键修复「辅助功能」授权：删掉旧记录 → 用当前二进制重新申请 → 打开设置页。
///
/// 三步顺序不能颠倒。TCC 的记录绑定在「代码签名指定要求」上，应用换签名之后
/// 旧记录虽然还显示为开启，却永远匹配不上新二进制；不先删除，用户无论怎么拨开关
/// 都不会生效——这正是「系统设置里明明允许了、程序还是说没权限」的成因。
#[tauri::command]
async fn repair_accessibility_permission(
    app: tauri::AppHandle,
) -> Result<AccessibilityRepair, String> {
    #[cfg(target_os = "macos")]
    {
        let bundle_id = app.config().identifier.clone();
        let mut notes: Vec<String> = Vec::new();

        match macos::reset_accessibility_grant(&bundle_id) {
            Ok(()) => {
                clipboard::log_line("repair: tccutil reset Accessibility ok");
                notes.push("已清除旧的授权记录".to_string());
            }
            Err(error) => {
                clipboard::log_line(&format!("repair: tccutil reset failed: {error}"));
                notes.push(format!("清除旧记录失败（{error}）"));
            }
        }

        // tccd 落盘有一点延迟，紧接着申请才不会被刚删掉的记录干扰
        tokio::time::sleep(tokio::time::Duration::from_millis(400)).await;

        let trusted = macos::request_accessibility_permission();
        clipboard::log_line(&format!("repair: request_accessibility -> trusted={trusted}"));
        if trusted {
            notes.push("已经获得授权，可以自动粘贴了".to_string());
            return Ok(AccessibilityRepair { trusted, message: notes.join("；") });
        }

        // 还没授权：把设置页直接推到用户面前，省掉自己找入口这一步
        let opened = macos::open_accessibility_settings().is_ok();
        notes.push(if opened {
            "请在打开的列表里打开 Xpaste 的开关，回到这里会自动刷新".to_string()
        } else {
            "请手动打开「系统设置 → 隐私与安全性 → 辅助功能」，打开 Xpaste 的开关".to_string()
        });

        Ok(AccessibilityRepair { trusted, message: notes.join("；") })
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        Ok(AccessibilityRepair {
            trusted: true,
            message: "当前平台无需此授权".to_string(),
        })
    }
}

/// 打开「系统设置 → 隐私与安全性 → 辅助功能」。
#[tauri::command]
fn open_accessibility_settings() -> Result<(), String> {
    #[cfg(target_os = "macos")]
    {
        macos::open_accessibility_settings()
    }
    #[cfg(not(target_os = "macos"))]
    {
        Err("当前平台无需此设置".to_string())
    }
}

/// Dock 图标可见性的配置文件。
///
/// 单独存一个文件而不是塞进 hotkey.txt：两者独立读写，避免为了改一个开关
/// 去解析另一份配置。
fn dock_config_path(app: &tauri::AppHandle) -> Result<std::path::PathBuf, String> {
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    Ok(config_dir.join("dock_icon.txt"))
}

/// Dock 图标是否可见。默认 false：本应用常驻托盘，Dock 图标属于噪声。
#[tauri::command]
fn get_dock_visible(app: tauri::AppHandle) -> Result<bool, String> {
    let path = dock_config_path(&app)?;
    Ok(std::fs::read_to_string(path)
        .map(|value| value.trim() == "visible")
        .unwrap_or(false))
}

/// 切换 Dock 图标可见性，立即生效并落盘。
#[tauri::command]
fn set_dock_visible(app: tauri::AppHandle, visible: bool) -> Result<(), String> {
    let path = dock_config_path(&app)?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    std::fs::write(&path, if visible { "visible" } else { "hidden" })
        .map_err(|e| e.to_string())?;

    #[cfg(target_os = "macos")]
    app.set_dock_visibility(visible).map_err(|e| e.to_string())?;

    Ok(())
}

fn main() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
        // 自动更新：检查远端 latest.json、下载并校验签名、调用安装程序
        .plugin(tauri_plugin_updater::Builder::new().build())
        // 安装完成后重启应用（Windows 下由安装程序负责重启）
        .plugin(tauri_plugin_process::init())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let log_path = app_data_dir.join("clipboard.log");
            clipboard::init_logger(log_path);
            let db_path = app_data_dir.join("clipboard.db");
            let pool = tauri::async_runtime::block_on(db::init_db(&db_path))?;
            let sdk = ClipboardSdk::new(pool);
            app.manage(AppState { sdk: sdk.clone() });
            let handle = app.handle().clone();
            clipboard::start_watcher(handle.clone(), sdk);

            // 首次运行时，默认开启自启动
            let first_run_flag = app_data_dir.join(".first_run");
            if !first_run_flag.exists() {
                // 原本这里是 let _ = ，失败时一点线索都不留；
                // 自启动在 macOS 上依赖 LaunchAgent，失败原因值得记进日志。
                if let Err(error) = app.handle().autolaunch().enable() {
                    clipboard::log_line(&format!("autostart: 首次运行开启自启动失败: {error}"));
                }
                std::fs::File::create(first_run_flag)?;
            }

            // 启动自检：把「当前这份二进制」的路径、签名摘要与授权状态写进日志。
            //
            // 排查「授权了还是粘贴不了」时，唯一可靠的依据就是这几行——它能证明
            // 系统认的是哪一份二进制、以及签名会不会随重新编译而变化。
            #[cfg(target_os = "macos")]
            {
                let exe = std::env::current_exe()
                    .map(|path| path.display().to_string())
                    .unwrap_or_else(|_| "?".to_string());
                clipboard::log_line(&format!(
                    "startup: exe={exe} trusted={}",
                    macos::has_accessibility_permission()
                ));

                // Contents/MacOS/paste_app -> Contents/MacOS -> Contents -> Xpaste.app
                let bundle = std::env::current_exe().ok().and_then(|path| {
                    path.parent()
                        .and_then(|p| p.parent())
                        .and_then(|p| p.parent())
                        .map(|p| p.to_path_buf())
                });
                if let Some(bundle) = bundle {
                    clipboard::log_line(&format!(
                        "startup: bundle={} {}",
                        bundle.display(),
                        macos::signing_diagnostics(&bundle)
                    ));
                    if bundle.to_string_lossy().starts_with("/Volumes/") {
                        clipboard::log_line(
                            "startup: 警告 — 当前从 DMG 卷内直接运行，请先拖进「应用程序」再打开",
                        );
                    }
                }
            }

            // 设置系统托盘菜单
            use tauri::{
                menu::{Menu, MenuItem, PredefinedMenuItem},
                tray::TrayIconBuilder,
            };

            let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
            let separator = PredefinedMenuItem::separator(app)?;
            let quit_item = MenuItem::with_id(app, "quit", "退出应用", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show_item, &separator, &quit_item])?;

            let app_handle_for_click = app.handle().clone();
            let app_handle_for_menu = app.handle().clone();

            let mut tray_builder = TrayIconBuilder::with_id("main").tooltip("Xpaste").menu(&menu);

            // 托盘图标：macOS 的默认窗口图标来自 bundle 里的 icns/png。
            // 这里做一次存在性判断，避免图标缺失时 unwrap 直接把应用打崩——
            // 托盘是本应用唯一常驻入口，宁可少个图标也不能崩在启动阶段。
            if let Some(icon) = app.default_window_icon() {
                tray_builder = tray_builder.icon(icon.clone());
            }

            // macOS 上托盘左键默认会弹出菜单，和「左键显示主窗口」打架：
            // 关掉之后左键 = 显示主窗口、右键 = 菜单，行为与 Windows 版一致。
            #[cfg(target_os = "macos")]
            {
                tray_builder = tray_builder.show_menu_on_left_click(false);
            }

            let tray = tray_builder
                .on_menu_event(move |_app, event| {
                    println!("Tray menu event: {:?}", event.id);
                    match event.id.as_ref() {
                        "show" => {
                            println!("Menu: Show main window");
                            if let Some(window) = app_handle_for_menu.get_webview_window("main") {
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        "quit" => {
                            println!("Menu: Quit application");
                            std::process::exit(0);
                        }
                        _ => {}
                    }
                })
                .on_tray_icon_event(move |_tray, event| {
                    println!("Tray icon event: {:?}", event);
                    match event {
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Left,
                            ..
                        } => {
                            println!("Tray icon left clicked");
                            if let Some(window) = app_handle_for_click.get_webview_window("main") {
                                println!("Showing main window");
                                let _ = window.show();
                                let _ = window.set_focus();
                            }
                        }
                        tauri::tray::TrayIconEvent::Click {
                            button: tauri::tray::MouseButton::Right,
                            ..
                        } => {
                            println!("Tray icon right clicked - menu should appear");
                            // 在某些系统上，右键会自动显示菜单
                            // 如果不自动显示，可以尝试手动触发
                        }
                        _ => {}
                    }
                })
                .build(app)?;

            println!("Tray icon created successfully with ID: {:?}", tray.id());

            // 主窗口关闭事件处理：隐藏而不是退出
            if let Some(main_window) = app.get_webview_window("main") {
                let window = main_window.clone();
                let _ = main_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                });
            }

            // 两个浮层窗口都要脱离默认的「空间归属」：
            //  - 面板：全屏应用里呼出时不能被系统丢到桌面
            //  - 编辑器：与面板成组出现，通常落在同一个空间
            // 不设这一步，全屏场景下窗口会被放到桌面上，看起来像"没反应"。
            // AppKit 的对象只能在主线程收发消息，setup 正好在主线程。
            #[cfg(target_os = "macos")]
            for label in ["popup", "editor"] {
                let Some(window) = app.get_webview_window(label) else {
                    continue;
                };
                match window.ns_window() {
                    Ok(handle) => match macos::configure_popup_panel(handle) {
                        Ok(behavior) => clipboard::log_line(&format!(
                            "{label}: 已转为 NSPanel（nonactivating + level 101），collectionBehavior=0x{behavior:x}"
                        )),
                        Err(error) => {
                            clipboard::log_line(&format!("{label}: 配置浮层失败: {error}"))
                        }
                    },
                    Err(error) => {
                        clipboard::log_line(&format!("{label}: 取不到 NSWindow 句柄: {error}"))
                    }
                }
            }

            // Popup窗口关闭事件处理：隐藏而不是销毁
            if let Some(popup_window) = app.get_webview_window("popup") {
                let window = popup_window.clone();
                let _ = popup_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        println!("Popup close requested, hiding instead");
                        api.prevent_close();
                        let _ = window.hide();
                    }
                });
            }

            // 编辑器窗口同理：它会被反复开关，销毁后重建 WebView 有明显延迟
            if let Some(editor_window) = app.get_webview_window("editor") {
                let window = editor_window.clone();
                let _ = editor_window.on_window_event(move |event| {
                    if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                        api.prevent_close();
                        let _ = window.hide();
                    }
                });
            }

            // 注册全局快捷键
            //
            // 读取保存的配置；空值以及历史上的 Windows 默认值都回落到当前平台默认键
            // ——在 macOS 上沿用 Win+V 等于抢占系统 ⌘V，会把所有应用的粘贴都劫走。
            let config_dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            let config_file = config_dir.join("hotkey.txt");
            let saved_hotkey = std::fs::read_to_string(&config_file).unwrap_or_default();
            let trimmed_hotkey = saved_hotkey.trim();

            let needs_platform_default = trimmed_hotkey.is_empty()
                || trimmed_hotkey == "Alt+V"
                || (cfg!(target_os = "macos") && trimmed_hotkey == "Win+V");

            let hotkey_str = if needs_platform_default {
                let default_hotkey = HotkeySpec::default_for_platform().to_string();
                std::fs::write(&config_file, &default_hotkey)?;
                default_hotkey
            } else {
                trimmed_hotkey.to_string()
            };

            apply_hotkey(app.handle(), &hotkey_str)
                .map_err(|error| format!("注册快捷键失败：{error}"))?;

            // macOS 的 Dock 图标：默认隐藏，只留顶部菜单栏的状态项。
            //
            // 这对应 Windows 的 skipTaskbar——本应用是常驻托盘的剪贴板工具，
            // 常开的主窗口图标只会挤占 Dock。隐藏后入口仍有两个：菜单栏图标
            // （左键显示主窗口 / 右键菜单）和全局快捷键。设置页里可以改回来，
            // 配置落在 dock_icon.txt，改动即时生效。
            #[cfg(target_os = "macos")]
            {
                let dock_file = config_dir.join("dock_icon.txt");
                let dock_visible = std::fs::read_to_string(&dock_file)
                    .map(|value| value.trim() == "visible")
                    .unwrap_or(false);
                if let Err(error) = app.handle().set_dock_visibility(dock_visible) {
                    clipboard::log_line(&format!("dock: 设置 Dock 图标可见性失败: {error}"));
                }
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_history,
            search_history,
            get_history_page,
            set_clipboard,
            set_clipboard_and_paste,
            paste_text,
            copy_text,
            get_item_text,
            open_editor,
            close_editor,
            set_editor_sidebar,
            is_editor_visible,
            is_own_window_frontmost,
            get_clipboard_image,
            save_clipboard_text,
            save_clipboard_image,
            hide_popup,
            clear_history,
            list_history_by_date,
            search_history_by_date,
            get_cursor_position,
            get_hotkey,
            set_hotkey,
            delete_history_item,
            delete_history_items,
            delete_history_by_format,
            delete_history_by_category,
            delete_history_by_date,
            get_format_stats,
            get_category_stats,
            toggle_favorite,
            remove_favorite,
            list_favorites,
            check_accessibility_permission,
            request_accessibility_permission,
            repair_accessibility_permission,
            open_accessibility_settings,
            get_dock_visible,
            set_dock_visible
        ])
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .run(tauri::generate_context!());
    keyboard_hook::unregister_hotkey();
    result.expect("error while running tauri application");
}
