// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod classify;
mod clipboard;
mod db;
mod models;

use std::borrow::Cow;

use arboard::ImageData;
use sqlx::SqlitePool;
use tauri::{Manager, State};

use crate::models::{ClipboardItem, ClipboardItemRow};

fn parse_hotkey(hotkey: &str) -> (Option<tauri_plugin_global_shortcut::Modifiers>, tauri_plugin_global_shortcut::Code) {
  use tauri_plugin_global_shortcut::{Code, Modifiers};
  
  let parts: Vec<&str> = hotkey.split('+').map(|s| s.trim()).collect();
  let mut modifiers_flags = Modifiers::empty();
  let mut key = Code::KeyV; // 默认
  
  for part in &parts {
    match part.to_uppercase().as_str() {
      "CTRL" | "CONTROL" => modifiers_flags |= Modifiers::CONTROL,
      "ALT" => modifiers_flags |= Modifiers::ALT,
      "SHIFT" => modifiers_flags |= Modifiers::SHIFT,
      "SUPER" | "WIN" | "CMD" => modifiers_flags |= Modifiers::SUPER,
      // 字母键
      "A" => key = Code::KeyA,
      "B" => key = Code::KeyB,
      "C" => key = Code::KeyC,
      "D" => key = Code::KeyD,
      "E" => key = Code::KeyE,
      "F" => key = Code::KeyF,
      "G" => key = Code::KeyG,
      "H" => key = Code::KeyH,
      "I" => key = Code::KeyI,
      "J" => key = Code::KeyJ,
      "K" => key = Code::KeyK,
      "L" => key = Code::KeyL,
      "M" => key = Code::KeyM,
      "N" => key = Code::KeyN,
      "O" => key = Code::KeyO,
      "P" => key = Code::KeyP,
      "Q" => key = Code::KeyQ,
      "R" => key = Code::KeyR,
      "S" => key = Code::KeyS,
      "T" => key = Code::KeyT,
      "U" => key = Code::KeyU,
      "V" => key = Code::KeyV,
      "W" => key = Code::KeyW,
      "X" => key = Code::KeyX,
      "Y" => key = Code::KeyY,
      "Z" => key = Code::KeyZ,
      _ => {}
    }
  }
  
  let modifiers = if modifiers_flags.is_empty() {
    None
  } else {
    Some(modifiers_flags)
  };
  
  (modifiers, key)
}

struct AppState {
  db: SqlitePool,
}

#[tauri::command]
async fn list_history(
  state: State<'_, AppState>,
  limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
  let limit = limit.unwrap_or(200).clamp(1, 1000);
  let rows = db::list_items(&state.db, limit)
    .await
    .map_err(|err| err.to_string())?;
  Ok(rows.into_iter().map(ClipboardItem::from_row).collect())
}

#[tauri::command]
async fn search_history(
  state: State<'_, AppState>,
  query: String,
  limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
  let trimmed = query.trim();
  if trimmed.is_empty() {
    return list_history(state, limit).await;
  }
  let limit = limit.unwrap_or(200).clamp(1, 1000);
  let rows = db::search_items(&state.db, trimmed, limit)
    .await
    .map_err(|err| err.to_string())?;
  Ok(rows.into_iter().map(ClipboardItem::from_row).collect())
}

#[tauri::command]
async fn set_clipboard(state: State<'_, AppState>, id: i64) -> Result<(), String> {
  let row = db::get_item(&state.db, id)
    .await
    .map_err(|err| err.to_string())?
    .ok_or_else(|| "记录不存在".to_string())?;

  write_to_clipboard(row).map_err(|err| err.to_string())
}

#[cfg(target_os = "windows")]
#[tauri::command]
async fn set_clipboard_and_paste(state: State<'_, AppState>, id: i64) -> Result<(), String> {
  use windows::Win32::UI::Input::KeyboardAndMouse::{
    SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP, VK_CONTROL, VK_V
  };
  
  // 先写入剪贴板
  let row = db::get_item(&state.db, id)
    .await
    .map_err(|err| err.to_string())?
    .ok_or_else(|| "记录不存在".to_string())?;
  
  write_to_clipboard(row).map_err(|err| err.to_string())?;
  
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
async fn list_history_by_date(
  state: State<'_, AppState>,
  start_ts: i64,
  end_ts: i64,
  limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
  let limit = limit.unwrap_or(200).clamp(1, 1000);
  let rows = db::list_items_by_date_range(&state.db, start_ts, end_ts, limit)
    .await
    .map_err(|err| err.to_string())?;
  Ok(rows.into_iter().map(ClipboardItem::from_row).collect())
}

#[tauri::command]
async fn search_history_by_date(
  state: State<'_, AppState>,
  query: String,
  start_ts: i64,
  end_ts: i64,
  limit: Option<i64>,
) -> Result<Vec<ClipboardItem>, String> {
  let trimmed = query.trim();
  if trimmed.is_empty() {
    return list_history_by_date(state, start_ts, end_ts, limit).await;
  }
  let limit = limit.unwrap_or(200).clamp(1, 1000);
  let rows = db::search_items_by_date_range(&state.db, trimmed, start_ts, end_ts, limit)
    .await
    .map_err(|err| err.to_string())?;
  Ok(rows.into_iter().map(ClipboardItem::from_row).collect())
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
    Ok("Alt+V".to_string())
  }
}

#[tauri::command]
async fn set_hotkey(app: tauri::AppHandle, hotkey: String) -> Result<(), String> {
  // 保存快捷键到配置文件
  let config_dir = app.path().app_config_dir().map_err(|e| e.to_string())?;
  std::fs::create_dir_all(&config_dir).map_err(|e| e.to_string())?;
  let config_file = config_dir.join("hotkey.txt");
  
  std::fs::write(config_file, hotkey).map_err(|e| e.to_string())?;
  Ok(())
}

fn write_to_clipboard(row: ClipboardItemRow) -> Result<(), arboard::Error> {
  let mut clipboard = arboard::Clipboard::new()?;
  match row.format.as_str() {
    "image" => {
      if let (Some(bytes), Some(width), Some(height)) =
        (row.image, row.image_width, row.image_height)
      {
        let data = ImageData {
          width: width as usize,
          height: height as usize,
          bytes: Cow::Owned(bytes),
        };
        clipboard.set_image(data)?;
      }
    }
    "html" => {
      if let Some(html) = row.html {
        clipboard.set_html(html, None)?;
      } else if let Some(text) = row.text {
        clipboard.set_text(text)?;
      }
    }
    _ => {
      if let Some(text) = row.text {
        clipboard.set_text(text)?;
      } else if let Some(file_path) = row.file_path {
        clipboard.set_text(file_path)?;
      } else if let Some(color) = row.color {
        clipboard.set_text(color)?;
      }
    }
  }
  Ok(())
}

fn main() {
  tauri::Builder::default()
    .setup(|app| {
      let app_data_dir = app.path().app_data_dir()?;
      std::fs::create_dir_all(&app_data_dir)?;
      let log_path = app_data_dir.join("clipboard.log");
      clipboard::init_logger(log_path);
      let db_path = app_data_dir.join("clipboard.db");
      let pool = tauri::async_runtime::block_on(db::init_db(&db_path))?;
      app.manage(AppState { db: pool.clone() });
      let handle = app.handle().clone();
      clipboard::start_watcher(handle.clone(), pool);
      
      // 设置系统托盘菜单
      use tauri::{menu::{Menu, MenuItem}, tray::TrayIconBuilder};
      
      let show_item = MenuItem::with_id(app, "show", "显示主窗口", true, None::<&str>)?;
      let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
      let menu = Menu::with_items(app, &[&show_item, &quit_item])?;
      
      let app_handle_for_click = app.handle().clone();
      let _tray = TrayIconBuilder::with_id("main")
        .menu(&menu)
        .on_menu_event(move |app, event| {
          match event.id.as_ref() {
            "show" => {
              if let Some(window) = app.get_webview_window("main") {
                let _ = window.show();
                let _ = window.set_focus();
              }
            }
            "quit" => {
              app.exit(0);
            }
            _ => {}
          }
        })
        .on_tray_icon_event(move |_tray, event| {
          println!("Tray event: {:?}", event);
          if let tauri::tray::TrayIconEvent::Click { button, .. } = event {
            println!("Tray clicked, button: {:?}", button);
            if button == tauri::tray::MouseButton::Left {
              println!("Tray icon left clicked-----------------------------");
              if let Some(window) = app_handle_for_click.get_webview_window("main") {
                println!("Found main window, showing...");
                let _ = window.show();
                let _ = window.set_focus();
              } else {
                println!("Main window not found!");
              }
            }
          }
        })
        .build(app)?;
      
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
      let app_handle = app.handle().clone();
      tauri::async_runtime::spawn(async move {
        use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, Code, Modifiers};
        
        // 读取保存的快捷键配置
        let config_dir = app_handle.path().app_config_dir().unwrap();
        let config_file = config_dir.join("hotkey.txt");
        let hotkey_str = if config_file.exists() {
          std::fs::read_to_string(config_file).unwrap_or("Alt+V".to_string())
        } else {
          "Alt+V".to_string()
        };
        
        // 解析快捷键字符串
        let (modifiers, key_code) = parse_hotkey(&hotkey_str);
        let shortcut = Shortcut::new(modifiers, key_code);
        
        let _ = app_handle.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, _event| {
          let handle = app.clone();
          tauri::async_runtime::spawn(async move {
            if let Some(window) = handle.get_webview_window("popup") {
              // 获取光标位置并计算窗口位置
              #[cfg(target_os = "windows")]
              {
                use windows::Win32::Foundation::POINT;
                use windows::Win32::Graphics::Gdi::{MonitorFromPoint, GetMonitorInfoW, MONITORINFO, MONITOR_DEFAULTTONEAREST};
                use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;
                
                let (x, y) = unsafe {
                  let mut point = POINT { x: 0, y: 0 };
                  if GetCursorPos(&mut point).is_ok() {
                    // 获取光标所在的显示器信息
                    let monitor = MonitorFromPoint(point, MONITOR_DEFAULTTONEAREST);
                    let mut monitor_info = MONITORINFO {
                      cbSize: std::mem::size_of::<MONITORINFO>() as u32,
                      ..Default::default()
                    };
                    
                    if GetMonitorInfoW(monitor, &mut monitor_info).as_bool() {
                      let work_area = monitor_info.rcWork;
                      let screen_left = work_area.left;
                      let screen_top = work_area.top;
                      
                      // 窗口尺寸（包含边框）
                      let window_width = 360;
                      let window_height = 500;
                      
                      // 计算初始位置（光标右下方）
                      let mut final_x = point.x + 10;
                      let mut final_y = point.y + 10;
                      
                      // 检查右边界
                      if final_x + window_width > work_area.right {
                        // 放在光标左侧
                        final_x = point.x - window_width - 10;
                        // 如果左侧也放不下，紧贴右边界
                        if final_x < screen_left {
                          final_x = work_area.right - window_width;
                        }
                      }
                      
                      // 检查底边界
                      if final_y + window_height > work_area.bottom {
                        // 放在光标上方
                        final_y = point.y - window_height - 10;
                        // 如果上方也放不下，紧贴底边界
                        if final_y < screen_top {
                          final_y = work_area.bottom - window_height;
                        }
                      }
                      
                      // 确保不超出左边界
                      if final_x < screen_left {
                        final_x = screen_left;
                      }
                      
                      // 确保不超出顶边界
                      if final_y < screen_top {
                        final_y = screen_top;
                      }
                      
                      (final_x, final_y)
                    } else {
                      // 如果无法获取显示器信息，使用简单的偏移
                      (point.x + 10, point.y + 10)
                    }
                  } else {
                    (100, 100)
                  }
                };
                
                use tauri::PhysicalPosition;
                let _ = window.set_position(PhysicalPosition::new(x, y));
              }
              
              let _ = window.show();
              // 不设置焦点，避免抢夺原输入框的焦点
              // let _ = window.set_focus();
            }
          });
        });
      });
      
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      list_history,
      search_history,
      set_clipboard,
      set_clipboard_and_paste,
      list_history_by_date,
      search_history_by_date,
      get_cursor_position,
      get_hotkey,
      set_hotkey
    ])
    .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
