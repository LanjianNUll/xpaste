mod classify;
mod clipboard;
mod db;
mod models;

use std::borrow::Cow;

use arboard::ImageData;
use sqlx::SqlitePool;
use tauri::{Manager, State};

use crate::models::{ClipboardItem, ClipboardItemRow};

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
      clipboard::start_watcher(handle, pool);
      
      // 注册全局快捷键 Alt+V
      let app_handle = app.handle().clone();
      tauri::async_runtime::spawn(async move {
        use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut};
        
        let shortcut = Shortcut::new(Some(tauri_plugin_global_shortcut::Modifiers::ALT), tauri_plugin_global_shortcut::Code::KeyV);
        
        let _ = app_handle.global_shortcut().on_shortcut(shortcut, move |app, _shortcut, _event| {
          let handle = app.clone();
          tauri::async_runtime::spawn(async move {
            if let Some(window) = handle.get_webview_window("popup") {
              // 获取光标位置并计算窗口位置
              #[cfg(target_os = "windows")]
              {
                use windows::Win32::Foundation::POINT;
                use windows::Win32::UI::WindowsAndMessaging::{GetCursorPos, GetSystemMetrics, SM_CXSCREEN, SM_CYSCREEN};
                
                let (x, y) = unsafe {
                  let mut point = POINT { x: 0, y: 0 };
                  if GetCursorPos(&mut point).is_ok() {
                    let screen_width = GetSystemMetrics(SM_CXSCREEN);
                    let screen_height = GetSystemMetrics(SM_CYSCREEN);
                    
                    // 窗口尺寸
                    let window_width = 360;
                    let window_height = 500;
                    
                    // 计算位置，确保窗口在屏幕内
                    let mut final_x = point.x + 10;
                    let mut final_y = point.y + 10;
                    
                    // 如果右侧超出屏幕，向左调整
                    if final_x + window_width > screen_width {
                      final_x = screen_width - window_width - 10;
                      if final_x < 0 {
                        final_x = 0;
                      }
                    }
                    
                    // 如果底部超出屏幕，向上调整
                    if final_y + window_height > screen_height {
                      final_y = screen_height - window_height - 10;
                      if final_y < 0 {
                        final_y = 0;
                      }
                    }
                    
                    (final_x, final_y)
                  } else {
                    (100, 100)
                  }
                };
                
                use tauri::PhysicalPosition;
                let _ = window.set_position(PhysicalPosition::new(x, y));
              }
              
              let _ = window.show();
              let _ = window.set_focus();
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
      list_history_by_date,
      search_history_by_date,
      get_cursor_position
    ])
    .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
