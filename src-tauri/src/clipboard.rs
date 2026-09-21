use std::collections::hash_map::DefaultHasher;
use std::fs::OpenOptions;
use std::hash::{Hash, Hasher};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use arboard::Clipboard;
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
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, RemoveClipboardFormatListener,
};
#[cfg(target_os = "windows")]
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
#[cfg(target_os = "windows")]
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, RegisterClassW,
    TranslateMessage, HWND_MESSAGE, MSG, WM_CLIPBOARDUPDATE, WNDCLASSW,
};

use crate::{classify, models::NewClipboardItem, sdk::ClipboardSdk};

struct CapturedItem {
    item: NewClipboardItem,
    hash: u64,
}

static LOG_PATH: OnceLock<PathBuf> = OnceLock::new();

pub fn init_logger(path: PathBuf) {
    let _ = LOG_PATH.set(path.clone());
    log_line(&format!("logger initialized: {}", path.display()));
}

pub fn start_watcher(app_handle: AppHandle, sdk: ClipboardSdk) {
    #[cfg(target_os = "windows")]
    {
        log_line("clipboard: starting Windows listener thread");
        eprintln!("clipboard: starting Windows listener thread");
        std::thread::spawn(move || {
            let handle = app_handle.clone();
            if let Err(err) = run_clipboard_listener(handle.clone(), sdk.clone()) {
                log_line(&format!("clipboard listener failed: {err}"));
                eprintln!("clipboard listener failed: {err}");
                start_polling_windows(handle, sdk);
            }
        });
    }

    #[cfg(not(target_os = "windows"))]
    {
        start_polling_async(app_handle, sdk);
    }
}

#[cfg(target_os = "windows")]
fn start_polling_windows(app_handle: AppHandle, sdk: ClipboardSdk) {
    std::thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        log_line("clipboard: fallback polling started");
        eprintln!("clipboard: fallback polling started");
        let mut last_hash: Option<u64> = None;
        loop {
            if let Some(captured) = capture_clipboard_with_retry() {
                handle_captured(&mut last_hash, &app_handle, &sdk, captured, true);
            }
            std::thread::sleep(Duration::from_millis(500));
        }
    });
}

#[cfg(not(target_os = "windows"))]
fn start_polling_async(app_handle: AppHandle, sdk: ClipboardSdk) {
    tauri::async_runtime::spawn(async move {
        let mut last_hash: Option<u64> = None;
        loop {
            if let Some(captured) = capture_clipboard() {
                handle_captured(&mut last_hash, &app_handle, &sdk, captured, true);
            }
            sleep(Duration::from_millis(500)).await;
        }
    });
}

fn handle_captured(
    last_hash: &mut Option<u64>,
    app_handle: &AppHandle,
    sdk: &ClipboardSdk,
    captured: CapturedItem,
    skip_unchanged: bool,
) {
    if skip_unchanged && *last_hash == Some(captured.hash) {
        return;
    }
    *last_hash = Some(captured.hash);
    log_line(&format!("clipboard: captured item hash={}", captured.hash));
    let sdk = sdk.clone();
    let handle = app_handle.clone();
    tauri::async_runtime::spawn(async move {
        match sdk.save_if_new(captured.item).await {
            Ok(false) => log_line("clipboard: duplicate content, copy count incremented"),
            Err(err) => {
                log_line(&format!("failed to insert clipboard item: {err}"));
                eprintln!("failed to insert clipboard item: {err}");
                return;
            }
            Ok(true) => log_line("clipboard: new content inserted"),
        }
        match handle.emit("clipboard://updated", ()) {
            Ok(_) => log_line("clipboard: event emitted"),
            Err(err) => log_line(&format!("clipboard: event emit failed: {err}")),
        }
    });
}

const MAX_IMAGE_SIZE: usize = 20 * 1024 * 1024;

/// Windows 文件/文件夹复制使用的剪贴板格式（CF_HDROP）。
#[cfg(target_os = "windows")]
const CF_HDROP: u32 = 15;

/// 读取剪贴板里的文件/文件夹路径列表（CF_HDROP）。
/// 在资源管理器里复制文件、文件夹时，系统写入的是 HDROP 而不是文本，
/// 因此必须显式读取该格式，才能把"复制了什么文件"记录进历史。
#[cfg(target_os = "windows")]
fn read_clipboard_file_paths() -> Option<Vec<String>> {
    use std::ptr;

    use windows::Win32::Foundation::HWND;
    use windows::Win32::System::DataExchange::{
        CloseClipboard, GetClipboardData, IsClipboardFormatAvailable, OpenClipboard,
    };
    use windows::Win32::UI::Shell::{DragQueryFileW, HDROP};

    // 剪贴板必须成对开关，读取结果先算完再统一关闭。
    let paths = unsafe {
        if OpenClipboard(HWND(ptr::null_mut())).is_err() {
            None
        } else if IsClipboardFormatAvailable(CF_HDROP).is_err() {
            None
        } else {
            match GetClipboardData(CF_HDROP) {
                Ok(handle) => {
                    let drop = HDROP(handle.0);
                    let count = DragQueryFileW(drop, u32::MAX, None);
                    let mut result = Vec::with_capacity(count as usize);
                    for index in 0..count {
                        let required = DragQueryFileW(drop, index, None) as usize;
                        if required == 0 {
                            continue;
                        }
                        let mut buffer = vec![0u16; required + 1];
                        let copied = DragQueryFileW(drop, index, Some(&mut buffer[..])) as usize;
                        if copied == 0 {
                            continue;
                        }
                        result.push(String::from_utf16_lossy(&buffer[..copied]));
                    }
                    Some(result)
                }
                Err(err) => {
                    log_line(&format!("clipboard: GetClipboardData(CF_HDROP) failed: {err}"));
                    None
                }
            }
        }
    };

    unsafe {
        let _ = CloseClipboard();
    }

    paths
}

/// 把文件/文件夹路径封装成一条历史记录：粘贴时写回的就是完整路径。
fn build_file_capture(paths: Vec<String>) -> Option<CapturedItem> {
    let normalized: Vec<String> = paths
        .into_iter()
        .map(|path| path.trim().to_string())
        .filter(|path| !path.is_empty())
        .collect();
    if normalized.is_empty() {
        return None;
    }

    // 全部是目录时按"文件夹"归类，便于界面区分文件与文件夹。
    let is_folder = normalized.iter().all(|path| Path::new(path).is_dir());
    // 多选时把每个路径各占一行，粘贴出来就是一组完整路径。
    let joined = normalized.join("\r\n");
    let hash = hash_text(&joined);
    log_line(&format!(
        "clipboard: file drop captured {} item(s), folder={}",
        normalized.len(),
        is_folder
    ));

    let item = NewClipboardItem {
        format: "file".to_string(),
        category: if is_folder { "folder" } else { "file" }.to_string(),
        text: Some(joined),
        html: None,
        file_path: Some(normalized[0].clone()),
        color: None,
        image: None,
        image_width: None,
        image_height: None,
        created_at: now_ms(),
    };
    Some(CapturedItem { item, hash })
}

fn capture_clipboard() -> Option<CapturedItem> {
    // Windows 下优先识别文件/文件夹复制：HDROP 才是权威来源，
    // 文本只是部分程序顺带写入的兼容格式。
    #[cfg(target_os = "windows")]
    if let Some(paths) = read_clipboard_file_paths() {
        if let Some(captured) = build_file_capture(paths) {
            return Some(captured);
        }
    }

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
        if bytes.len() > MAX_IMAGE_SIZE {
            log_line(&format!(
                "clipboard: image too large ({} bytes), skipping",
                bytes.len()
            ));
            return None;
        }
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
fn run_clipboard_listener(app_handle: AppHandle, sdk: ClipboardSdk) -> windows::core::Result<()> {
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
                handle_captured(&mut last_hash, &app_handle, &sdk, captured, false);
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
