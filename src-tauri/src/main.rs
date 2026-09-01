// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod classify;
mod clipboard;
mod db;
mod history_repository;
mod keyboard_hook;
mod models;
mod sdk;
mod window_controller;

use tauri::{Manager, State};
use tauri_plugin_autostart::ManagerExt;

use crate::models::{ClipboardItem, HistoryPage, HistoryPageQuery};
use crate::sdk::ClipboardSdk;

fn parse_hotkey(hotkey: &str) -> (u32, bool, bool, bool, bool) {
    let parts: Vec<&str> = hotkey.split('+').map(|s| s.trim()).collect();
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut win = false;
    let mut key_code = 0x56u32; // V 键的虚拟键码

    for part in &parts {
        match part.to_uppercase().as_str() {
            "CTRL" | "CONTROL" => ctrl = true,
            "ALT" => alt = true,
            "SHIFT" => shift = true,
            "SUPER" | "WIN" | "CMD" => win = true,
            // 字母键 (A-Z 对应 0x41-0x5A)
            "A" => key_code = 0x41,
            "B" => key_code = 0x42,
            "C" => key_code = 0x43,
            "D" => key_code = 0x44,
            "E" => key_code = 0x45,
            "F" => key_code = 0x46,
            "G" => key_code = 0x47,
            "H" => key_code = 0x48,
            "I" => key_code = 0x49,
            "J" => key_code = 0x4A,
            "K" => key_code = 0x4B,
            "L" => key_code = 0x4C,
            "M" => key_code = 0x4D,
            "N" => key_code = 0x4E,
            "O" => key_code = 0x4F,
            "P" => key_code = 0x50,
            "Q" => key_code = 0x51,
            "R" => key_code = 0x52,
            "S" => key_code = 0x53,
            "T" => key_code = 0x54,
            "U" => key_code = 0x55,
            "V" => key_code = 0x56,
            "W" => key_code = 0x57,
            "X" => key_code = 0x58,
            "Y" => key_code = 0x59,
            "Z" => key_code = 0x5A,
            _ => {}
        }
    }

    (key_code, ctrl, alt, shift, win)
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

#[cfg(not(target_os = "windows"))]
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

#[cfg(target_os = "windows")]
#[tauri::command]
async fn get_cursor_position() -> Result<(i32, i32), String> {
    use windows::Win32::Foundation::POINT;
    use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

    unsafe {
        let mut point = POINT { x: 0, y: 0 };
        GetCursorPos(&mut point).map_err(|e| e.to_string())?;
        Ok((point.x, point.y))
    }
}

#[cfg(not(target_os = "windows"))]
#[tauri::command]
async fn get_cursor_position() -> Result<(i32, i32), String> {
    Ok((0, 0))
}

#[tauri::command]
async fn get_hotkey(app: tauri::AppHandle) -> Result<String, String> {
    // 从配置文件读取快捷键，如果不存在则返回默认值
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    let config_file = config_dir.join("hotkey.txt");

    if config_file.exists() {
        std::fs::read_to_string(config_file).map_err(|e| e.to_string())
    } else {
        Ok("Win+V".to_string())
    }
}

#[tauri::command]
async fn set_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
    // 保存快捷键到配置文件
    let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
    std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
    let config_file = config_dir.join("hotkey.txt");

    std::fs::write(config_file, &hotkey).map_err(|e| e.to_string())?;

    // 重新注册快捷键
    #[cfg(target_os = "windows")]
    {
        let (key_code, ctrl, alt, shift, win) = parse_hotkey(&hotkey);
        let app_handle = app.clone();

        keyboard_hook::register_hotkey(key_code, ctrl, alt, shift, win, move || {
            let handle = app_handle.clone();
            tauri::async_runtime::spawn(async move {
                let _ = window_controller::toggle_popup(&handle);
            });
        })
        .map_err(|e| format!("重新注册快捷键失败: {}", e))?;
    }

    Ok(())
}

fn main() {
    let result = tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .plugin(tauri_plugin_shell::init())
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
                let _ = app.handle().autolaunch().enable();
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

            let tray = TrayIconBuilder::with_id("main")
                .tooltip("Xpaste")
                .icon(app.default_window_icon().unwrap().clone())
                .menu(&menu)
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

            // 注册全局快捷键（使用低级键盘钩子以提高优先级）
            let app_handle = app.handle().clone();

            // 读取保存的快捷键配置
            let config_dir = app.path().app_config_dir()?;
            std::fs::create_dir_all(&config_dir)?;
            let config_file = config_dir.join("hotkey.txt");
            let saved_hotkey = std::fs::read_to_string(&config_file).unwrap_or_default();
            let hotkey_str = if saved_hotkey.trim().is_empty() || saved_hotkey.trim() == "Alt+V" {
                let default_hotkey = "Win+V".to_string();
                std::fs::write(&config_file, &default_hotkey)?;
                default_hotkey
            } else {
                saved_hotkey
            };

            // 解析快捷键字符串
            let (key_code, ctrl, alt, shift, win) = parse_hotkey(&hotkey_str);

            #[cfg(target_os = "windows")]
            {
                keyboard_hook::register_hotkey(key_code, ctrl, alt, shift, win, move || {
                    let handle = app_handle.clone();
                    tauri::async_runtime::spawn(async move {
                        let _ = window_controller::toggle_popup(&handle);
                    });
                })
                .map_err(|e| format!("注册快捷键失败: {}", e))?;
            }

            #[cfg(not(target_os = "windows"))]
            {
                // 非 Windows 系统使用原来的方式
                tauri::async_runtime::spawn(async move {
                    use tauri_plugin_global_shortcut::{
                        Code, GlobalShortcutExt, Modifiers, Shortcut,
                    };

                    let shortcut =
                        Shortcut::new(if alt { Some(Modifiers::ALT) } else { None }, Code::KeyV);

                    let _ = app_handle.global_shortcut().on_shortcut(
                        shortcut,
                        move |app, _shortcut, _event| {
                            // ... 原来的逻辑
                        },
                    );
                });
            }

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
