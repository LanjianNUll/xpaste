use std::collections::hash_map::DefaultHasher;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arboard::Clipboard;
use sqlx::SqlitePool;
use tauri::{AppHandle, Emitter};

#[cfg(not(target_os = "windows"))]
use tokio::time::sleep;

#[cfg(target_os = "windows")]
use windows::core::w;
#[cfg(target_os = "windows")]
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
#[cfg(target_os = "windows")]
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
#[cfg(target_os = "windows")]
use windows::Win32::System::DataExchange::{AddClipboardFormatListener, RemoveClipboardFormatListener};
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
  CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW, TranslateMessage,
  HWND_MESSAGE, MSG, WNDCLASSW, WM_CLIPBOARDUPDATE,
};

use crate::{classify, db, models::NewClipboardItem};

struct CapturedItem {
  item: NewClipboardItem,
  hash: u64,
}

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init_logger(path: PathBuf) {
  let _ = LOG_PATH.set(path.clone());
  log_line(&format!("logger initialized: {}", path.display()));
}

pub fn start_watcher(app_handle: AppHandle, pool: SqlitePool) {
  #[cfg(target_os = "windows")]
  {
    log_line("clipboard: starting Windows listener thread");
    eprintln!("clipboard: starting Windows listener thread");
    std::thread::spawn(move || {
      let handle = app_handle.clone();
      if let Err(err) = run_clipboard_listener(handle.clone(), pool.clone()) {
        log_line(&format!("clipboard listener failed: {err}"));
        eprintln!("clipboard listener failed: {err}");
        start_polling_windows(handle, pool);
      }
    });
  }

  #[cfg(not(target_os = "windows"))]
  {
    start_polling_async(app_handle, pool);
  }
}

#[cfg(target_os = "windows")]
fn start_polling_windows(app_handle: AppHandle, pool: SqlitePool) {
  std::thread::spawn(move || {
    unsafe {
      let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }
    log_line("clipboard: fallback polling started");
    eprintln!("clipboard: fallback polling started");
    let mut last_hash: Option<u64> = None;
    loop {
      if let Some(captured) = capture_clipboard_with_retry() {
        handle_captured(&mut last_hash, &app_handle, &pool, captured);
      }
      std::thread::sleep(Duration::from_millis(500));
    }
  });
}

#[cfg(not(target_os = "windows"))]
fn start_polling_async(app_handle: AppHandle, pool: SqlitePool) {
  tauri::async_runtime::spawn(async move {
    let mut last_hash: Option<u64> = None;
    loop {
      if let Some(captured) = capture_clipboard() {
        handle_captured(&mut last_hash, &app_handle, &pool, captured);
      }
      sleep(Duration::from_millis(500)).await;
    }
  });
}

fn handle_captured(
  last_hash: &mut Option<u64>,
  app_handle: &AppHandle,
  pool: &SqlitePool,
  captured: CapturedItem,
) {
  if *last_hash == Some(captured.hash) {
    return;
  }
  *last_hash = Some(captured.hash);
  log_line(&format!("clipboard: captured item hash={}", captured.hash));
  let pool = pool.clone();
  let handle = app_handle.clone();
  tauri::async_runtime::spawn(async move {
    if let Err(err) = db::insert_item(&pool, captured.item).await {
      log_line(&format!("failed to insert clipboard item: {err}"));
      eprintln!("failed to insert clipboard item: {err}");
      return;
    }
    match handle.emit("clipboard://updated", ()) {
      Ok(_) => log_line("clipboard: event emitted"),
      Err(err) => log_line(&format!("clipboard: event emit failed: {err}")),
    }
  });
}

fn capture_clipboard() -> Option<CapturedItem> {
  let mut clipboard = match Clipboard::new() {
    Ok(clipboard) => clipboard,
    Err(err) => {
      log_line(&format!("clipboard init failed: {err}"));
      eprintln!("clipboard init failed: {err}");
      return None;
    }
  };

  if let Ok(image) = clipboard.get_image() {
    let bytes = image.bytes.into_owned();
    let hash = hash_image(&bytes, image.width, image.height);
    let item = NewClipboardItem {
      format: "image".to_string(),
      category: "image".to_string(),
      text: None,
      html: None,
      file_path: None,
      color: None,
      image: Some(bytes),
      image_width: Some(image.width as i64),
      image_height: Some(image.height as i64),
      created_at: now_ms(),
    };
    return Some(CapturedItem { item, hash });
  }

  if let Ok(text) = clipboard.get_text() {
    let trimmed = text.trim();
    if trimmed.is_empty() {
      return None;
    }
    let classification = classify::classify_text(trimmed);
    let hash = hash_text(trimmed);
    let item = NewClipboardItem {
      format: classification.format,
      category: classification.category,
      text: Some(trimmed.to_string()),
      html: None,
      file_path: classification.file_path,
      color: classification.color,
      image: None,
      image_width: None,
      image_height: None,
      created_at: now_ms(),
    };
    return Some(CapturedItem { item, hash });
  }

  None
}

#[cfg(target_os = "windows")]
fn capture_clipboard_with_retry() -> Option<CapturedItem> {
  for attempt in 0..5 {
    if let Some(captured) = capture_clipboard() {
      return Some(captured);
    }
    if attempt < 4 {
      std::thread::sleep(Duration::from_millis(60));
    }
  }
  None
}

fn now_ms() -> i64 {
  SystemTime::now()
    .duration_since(UNIX_EPOCH)
    .map(|d| d.as_millis() as i64)
    .unwrap_or(0)
}

fn hash_text(text: &str) -> u64 {
  let mut hasher = DefaultHasher::new();
  text.hash(&mut hasher);
  hasher.finish()
}

fn hash_image(bytes: &[u8], width: usize, height: usize) -> u64 {
  let mut hasher = DefaultHasher::new();
  bytes.hash(&mut hasher);
  width.hash(&mut hasher);
  height.hash(&mut hasher);
  hasher.finish()
}

#[cfg(target_os = "windows")]
fn run_clipboard_listener(app_handle: AppHandle, pool: SqlitePool) -> windows::core::Result<()> {
  unsafe {
    CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
  }
  log_line("clipboard: listener thread initialized");

  let class_name = w!("PasteAppClipboardListener");
  let hinstance = unsafe { GetModuleHandleW(None)? };
  let wnd_class = WNDCLASSW {
    hInstance: hinstance.into(),
    lpszClassName: class_name,
    lpfnWndProc: Some(window_proc),
    ..Default::default()
  };
  unsafe {
    RegisterClassW(&wnd_class);
  }

  let hwnd = unsafe {
    CreateWindowExW(
      Default::default(),
      class_name,
      w!(""),
      Default::default(),
      0,
      0,
      0,
      0,
      HWND_MESSAGE,
      None,
      hinstance,
      None,
    )?
  };
  log_line("clipboard: message-only window created");

  unsafe {
    AddClipboardFormatListener(hwnd)?;
  }
  log_line("clipboard: AddClipboardFormatListener ok");

  let mut msg = MSG::default();
  let mut last_hash: Option<u64> = None;
  loop {
    let result = unsafe { GetMessageW(&mut msg, HWND(std::ptr::null_mut()), 0, 0) };
    if result.0 == 0 {
      break;
    }
    if result.0 == -1 {
      break;
    }
    if msg.message == WM_CLIPBOARDUPDATE {
      log_line("clipboard: WM_CLIPBOARDUPDATE received");
      let captured = capture_clipboard_with_retry();
      if let Some(captured) = captured {
        handle_captured(&mut last_hash, &app_handle, &pool, captured);
      } else {
        log_line("clipboard update received but no data captured");
        eprintln!("clipboard update received but no data captured");
      }
    }
    unsafe {
      let _ = TranslateMessage(&msg);
      DispatchMessageW(&msg);
    }
  }

  unsafe {
    let _ = RemoveClipboardFormatListener(hwnd);
    CoUninitialize();
  }
  log_line("clipboard: listener thread exiting");

  Ok(())
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn window_proc(
  hwnd: HWND,
  msg: u32,
  wparam: WPARAM,
  lparam: LPARAM,
) -> LRESULT {
  DefWindowProcW(hwnd, msg, wparam, lparam)
}

fn log_line(message: &str) {
  if let Some(path) = LOG_PATH.get() {
    let line = format!("{} {}\n", now_ms(), message);
    if let Ok(mut file) = OpenOptions::new().create(true).append(true).open(path) {
      let _ = file.write_all(line.as_bytes());
    }
  }
}
