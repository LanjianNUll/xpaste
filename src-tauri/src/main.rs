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
      Ok(())
    })
    .invoke_handler(tauri::generate_handler![
      list_history,
      search_history,
      set_clipboard
    ])
    .plugin(tauri_plugin_global_shortcut::Builder::new().build())
    .run(tauri::generate_context!())
    .expect("error while running tauri application");
}
