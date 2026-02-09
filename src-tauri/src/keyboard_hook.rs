#[cfg(target_os = "windows")]
use std::sync::{Arc, Mutex, OnceLock};
use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetAsyncKeyState, VK_CONTROL, VK_MENU, VK_SHIFT, VK_LWIN, VK_RWIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

type HotkeyCallback = Arc<dyn Fn() + Send + Sync + 'static>;

struct HookState {
    callback: Option<HotkeyCallback>,
    key_code: u32,
    ctrl: bool,
    alt: bool,
    shift: bool,
    win: bool,
}

// 使用 OnceLock 来存储全局状态
static HOOK_STATE: OnceLock<Mutex<HookState>> = OnceLock::new();
static HOOK_HANDLE: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 && (wparam.0 as u32 == WM_KEYDOWN || wparam.0 as u32 == WM_SYSKEYDOWN) {
        let kb_struct = *(lparam.0 as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode;

        if let Some(state_mutex) = HOOK_STATE.get() {
            if let Ok(state) = state_mutex.lock() {
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
                    // 触发回调
                    if let Some(callback) = &state.callback {
                        callback();
                    }
                    // 返回 1 阻止系统进一步处理这个按键
                    return LRESULT(1);
                }
            }
        }
    }

    CallNextHookEx(None, code, wparam, lparam)
}

#[cfg(target_os = "windows")]
pub fn register_hotkey<F>(key_code: u32, ctrl: bool, alt: bool, shift: bool, win: bool, callback: F) -> Result<(), String>
where
    F: Fn() + Send + Sync + 'static,
{
    unsafe {
        // 先清理旧的钩子
        unregister_hotkey();

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
            callback: Some(Arc::new(callback)),
            key_code,
            ctrl,
            alt,
            shift,
            win,
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
pub fn register_hotkey<F>(_key_code: u32, _ctrl: bool, _alt: bool, _shift: bool, _win: bool, _callback: F) -> Result<(), String>
where
    F: Fn() + Send + Sync + 'static,
{
    Err("键盘钩子仅在 Windows 上支持".to_string())
}

#[cfg(not(target_os = "windows"))]
pub fn unregister_hotkey() {}

