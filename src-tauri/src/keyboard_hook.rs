#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYEVENTF_KEYUP,
    VIRTUAL_KEY, VK_CONTROL, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

type HotkeyCallback = Arc<dyn Fn() + Send + Sync + 'static>;

struct HookState {
    callback: Option<HotkeyCallback>,
    key_code: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    key_down: bool,
    mask_win_on_release: bool,
}

// 使用 OnceLock 来存储全局状态
static HOOK_STATE: OnceLock<Mutex<HookState>> = OnceLock::new();
static HOOK_HANDLE: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

/// 发送无实际功能的虚拟键，避免 Win 组合键释放后弹出开始菜单。
fn mask_windows_key() {
    const VK_MASK: VIRTUAL_KEY = VIRTUAL_KEY(0x07);
    let inputs = [
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_MASK,
                    ..Default::default()
                },
            },
        },
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VK_MASK,
                    dwFlags: KEYEVENTF_KEYUP,
                    ..Default::default()
                },
            },
        },
    ];

    unsafe {
        let _ = SendInput(&inputs, std::mem::size_of::<INPUT>() as i32);
    }
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let kb_struct = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode;
        let message = wparam.0 as u32;
        let mut callback = None;
        let mut should_mask_windows_key = false;
        let mut should_suppress = false;

        if let Some(state_mutex) = HOOK_STATE.get() {
            if let Ok(mut state) = state_mutex.lock() {
                if (vk_code == VK_LWIN.0 as u32 || vk_code == VK_RWIN.0 as u32)
                    && (message == WM_KEYUP || message == WM_SYSKEYUP)
                    && state.mask_win_on_release
                {
                    state.mask_win_on_release = false;
                    should_mask_windows_key = true;
                } else if vk_code == state.key_code
                    && state.key_down
                    && (message == WM_KEYUP || message == WM_SYSKEYUP)
                {
                    state.key_down = false;
                    should_suppress = true;
                } else if message == WM_KEYDOWN || message == WM_SYSKEYDOWN {
                    // 检查修饰键状态
                    let ctrl_pressed = (GetAsyncKeyState(VK_CONTROL.0 as i32) as u16 & 0x8000) != 0;
                    let alt_pressed = (GetAsyncKeyState(VK_MENU.0 as i32) as u16 & 0x8000) != 0;
                    let shift_pressed = (GetAsyncKeyState(VK_SHIFT.0 as i32) as u16 & 0x8000) != 0;
                    let win_pressed = (GetAsyncKeyState(VK_LWIN.0 as i32) as u16 & 0x8000) != 0
                        || (GetAsyncKeyState(VK_RWIN.0 as i32) as u16 & 0x8000) != 0;

                    // 检查是否匹配我们的快捷键
                    if vk_code == state.key_code
                        && ctrl_pressed == state.ctrl
                        && alt_pressed == state.alt
                        && shift_pressed == state.shift
                        && win_pressed == state.win
                    {
                        should_suppress = true;
                        if !state.key_down {
                            state.key_down = true;
                            state.mask_win_on_release = state.win;
                            callback = state.callback.clone();
                        }
                    }
                }
            }
        }

        // 必须在状态锁释放后发送输入并执行回调，防止钩子重入造成死锁。
        if should_mask_windows_key {
            mask_windows_key();
        }
        if let Some(callback) = callback {
            callback();
        }
        if should_suppress {
            return LRESULT(1);
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

#[cfg(target_os = "windows")]
pub fn register_hotkey<F>(
    key_code: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
    callback: F,
) -> Result<(), String>
where
    F: Fn() + Send + Sync + 'static,
{
    unsafe {
        let callback: HotkeyCallback = Arc::new(callback);
        if HOOK_HANDLE
            .get()
            .and_then(|handle| handle.lock().ok())
            .and_then(|handle| *handle)
            .is_some()
        {
            if let Some(state) = HOOK_STATE.get() {
                *state.lock().map_err(|_| "快捷键状态更新失败".to_string())? = HookState {
                    callback: Some(callback),
                    key_code,
                    ctrl,
                    alt,
                    shift,
                    win,
                    key_down: false,
                    mask_win_on_release: false,
                };
                return Ok(());
            }
        }

        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)
            .map_err(|e| format!("设置键盘钩子失败: {}", e))?;

        // 存储钩子句柄
        let hook_handle = hook.0 as isize;
        HOOK_HANDLE
            .get_or_init(|| Mutex::new(None))
            .lock()
            .unwrap()
            .replace(hook_handle);

        // 存储状态
        let state = HookState {
            callback: Some(callback),
            key_code,
            ctrl,
            alt,
            shift,
            win,
            key_down: false,
            mask_win_on_release: false,
        };

        if let Some(state_mutex) = HOOK_STATE.get() {
            *state_mutex.lock().unwrap() = state;
        } else {
            HOOK_STATE.get_or_init(|| Mutex::new(state));
        }

        Ok(())
    }
}

#[cfg(target_os = "windows")]
pub fn unregister_hotkey() {
    unsafe {
        if let Some(handle_mutex) = HOOK_HANDLE.get() {
            if let Ok(mut handle_opt) = handle_mutex.lock() {
                if let Some(handle) = handle_opt.take() {
                    let hhook = HHOOK(handle as *mut _);
                    let _ = UnhookWindowsHookEx(hhook);
                }
            }
        }
    }
}

#[cfg(not(target_os = "windows"))]
pub fn register_hotkey<F>(
    _key_code: u32,
    _ctrl: bool,
    _alt: bool,
    _shift: bool,
    _win: bool,
    _callback: F,
) -> Result<(), String>
where
    F: Fn() + Send + Sync + 'static,
{
    Err("键盘钩子仅在 Windows 上支持".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn unregister_hotkey() {}
