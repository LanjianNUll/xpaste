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
use crate::sdk::ClipboardSdk;

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
        .on_shortcut(shortcut, move |app, _shortcut, _event| {
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
async fn set_clipboard(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    state.sdk.copy_item(id).await
}

#[tauri::command]
async fn get_clipboard_image(
    state: State<'_, AppState>,
    id: i64,
    thumbnail: Option<bool>,
) -> Result<String, String> {
    state.sdk.image_base64(id, thumbnail.unwrap_or(false)).await
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

#[cfg(target_os = "windows")]
#[tauri::command]
async fn set_clipboard_and_paste(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V,
    };

    state.sdk.copy_item(id).await?;
    window_controller::hide_popup(&app)?;
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
#[tauri::command]
async fn set_clipboard_and_paste(
    app: tauri::AppHandle,
    state: State<'_, AppState>,
    id: i64,
) -> Result<(), String> {
    state.sdk.copy_item(id).await?;

    // 没有辅助功能权限时系统会静默丢弃合成按键，表现为"点了没反应"。
    // 这里刻意不隐藏面板：用户必须看到这条提示，才知道要去系统设置里授权。
    // 内容已经进了剪贴板，此时手动按 ⌘V 依然可用。
    if !macos::has_accessibility_permission() {
        return Err(
            "已复制到剪贴板。要自动粘贴，请在「系统设置 → 隐私与安全性 → 辅助功能」中允许 Xpaste。"
                .to_string(),
        );
    }

    let _ = window_controller::hide_popup(&app);
    let _ = app.hide();
    tokio::time::sleep(tokio::time::Duration::from_millis(120)).await;

    macos::simulate_paste()
}

/// 其余平台暂不支持模拟按键，退化为仅写入剪贴板。
#[cfg(all(not(target_os = "windows"), not(target_os = "macos")))]
#[tauri::command]
async fn set_clipboard_and_paste(state: State<'_, AppState>, id: i64) -> Result<(), String> {
    set_clipboard(state, id).await
}

#[tauri::command]
fn hide_popup(app: tauri::AppHandle) -> Result<(), String> {
    window_controller::hide_popup(&app)
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

            // macOS 上窗口是自绘标题栏的，Dock 图标保持默认（可见）：这样即使托盘
            // 状态项没出现，用户也还能正常切换/退出应用。想做成纯托盘应用（对齐
            // Windows 的 skipTaskbar），把下面这行放开即可。
            // #[cfg(target_os = "macos")]
            // app.handle().set_dock_visibility(false)?;

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            list_history,
            search_history,
            get_history_page,
            set_clipboard,
            set_clipboard_and_paste,
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
            get_category_stats
        ])
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        .run(tauri::generate_context!());
    keyboard_hook::unregister_hotkey();
    result.expect("error while running tauri application");
}
