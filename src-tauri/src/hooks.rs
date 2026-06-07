use std::ffi::CString;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU8, Ordering};
use std::sync::mpsc::{channel, Sender};
use std::sync::Mutex;
use std::thread;
use winapi::shared::minwindef::{LPARAM, LRESULT, WPARAM};
use winapi::um::debugapi::OutputDebugStringA;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::{
    CallNextHookEx, DispatchMessageW, GetKeyState, GetMessageW,
    PostThreadMessageW, SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx,
    KBDLLHOOKSTRUCT, KLF_ACTIVATE, LoadKeyboardLayoutW, MSG, VK_BACK, VK_CAPITAL, VK_DELETE,
    VK_DOWN, VK_END, VK_ESCAPE, VK_HOME, VK_INSERT, VK_LEFT, VK_NEXT, VK_PRIOR, VK_RETURN,
    VK_RIGHT, VK_SPACE, VK_UP, WH_KEYBOARD_LL, WH_MOUSE_LL, WM_KEYDOWN, WM_KEYUP,
    WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MBUTTONDOWN, WM_MBUTTONUP, WM_MOUSEMOVE, WM_QUIT,
    WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SYSKEYDOWN, WM_SYSKEYUP,
};

/// 0 = 未锁屏（正常）
/// 1 = 锁屏显示，密码框隐藏
/// 2 = 锁屏显示，密码框可见
static LOCK_STATE: AtomicU8 = AtomicU8::new(0);
static HOOK_INSTALLED: AtomicU8 = AtomicU8::new(0);
static MOUSE_CLICKED: AtomicBool = AtomicBool::new(false);
static HOOK_THREAD_ID: Mutex<Option<u32>> = Mutex::new(None);

// 异步日志通道，避免在钩子回调中进行文件 I/O（会导致钩子超时）
static LOG_SENDER: Mutex<Option<Sender<String>>> = Mutex::new(None);

fn get_log_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    path.push("hook.log");
    path
}

fn init_async_log() {
    let (tx, rx) = channel::<String>();
    *LOG_SENDER.lock().unwrap() = Some(tx);
    thread::spawn(move || {
        let path = get_log_path();
        while let Ok(line) = rx.recv() {
            let _ = OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .and_then(|mut f| f.write_all(line.as_bytes()));
        }
    });
}

fn debug_log(msg: &str) {
    let mut full_msg = String::from(msg);
    full_msg.push('\n');
    if let Ok(c_msg) = CString::new(full_msg) {
        unsafe {
            OutputDebugStringA(c_msg.as_ptr());
        }
    }
}

fn log(msg: &str) {
    if let Ok(sender) = LOG_SENDER.try_lock() {
        if let Some(ref tx) = *sender {
            let now = std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .unwrap_or_default()
                .as_secs();
            let line = format!("[{}] {}\n", now, msg);
            let _ = tx.send(line);
        }
    }
}

pub fn set_lock_state(state: u8) {
    log(&format!("set_lock_state: {}", state));
    LOCK_STATE.store(state, Ordering::SeqCst);
    
    // 锁屏时强制切换为英文输入法，解锁时恢复原输入法
    unsafe {
        if state != 0 {
            force_english_input();
        } else {
            restore_input_layout();
        }
    }
}

static mut ORIGINAL_LAYOUT: isize = 0;

unsafe fn force_english_input() {
    // 保存当前输入法，供解锁时恢复
    ORIGINAL_LAYOUT = winapi::um::winuser::GetKeyboardLayout(0) as isize;
    // 加载英语(美国) 键盘布局
    let layout_name: Vec<u16> = "00000409\0".encode_utf16().collect();
    let eng_layout = LoadKeyboardLayoutW(layout_name.as_ptr(), KLF_ACTIVATE);
    if !eng_layout.is_null() {
        winapi::um::winuser::ActivateKeyboardLayout(eng_layout, 0);
        log("Force switched to English keyboard layout");
    } else {
        log("Failed to load English keyboard layout");
    }
}

unsafe fn restore_input_layout() {
    if ORIGINAL_LAYOUT != 0 {
        let layout = ORIGINAL_LAYOUT as winapi::shared::minwindef::HKL;
        winapi::um::winuser::ActivateKeyboardLayout(layout, 0);
        log("Restored original keyboard layout");
    }
}

pub fn poll_mouse_click() -> bool {
    MOUSE_CLICKED.swap(false, Ordering::SeqCst)
}

pub fn is_caps_lock_on() -> bool {
    unsafe {
        let state = GetKeyState(VK_CAPITAL as i32);
        (state & 0x0001) != 0
    }
}

pub fn turn_off_caps_lock() {
    unsafe {
        if is_caps_lock_on() {
            use winapi::um::winuser::{SendInput, INPUT, INPUT_KEYBOARD, KEYEVENTF_KEYUP};
            let mut inputs: [INPUT; 2] = std::mem::zeroed();

            // 按下 Caps Lock
            inputs[0].type_ = INPUT_KEYBOARD;
            let ki = inputs[0].u.ki_mut();
            ki.wVk = VK_CAPITAL as u16;
            ki.wScan = 0x3A;
            ki.dwFlags = 0;

            // 释放 Caps Lock
            inputs[1].type_ = INPUT_KEYBOARD;
            let ki = inputs[1].u.ki_mut();
            ki.wVk = VK_CAPITAL as u16;
            ki.wScan = 0x3A;
            ki.dwFlags = KEYEVENTF_KEYUP;

            SendInput(2, inputs.as_mut_ptr(), std::mem::size_of::<INPUT>() as i32);
            log("Turned off Caps Lock via SendInput");
        }
    }
}

pub fn install_keyboard_hook() {
    if HOOK_INSTALLED.swap(1, Ordering::SeqCst) == 1 {
        log("install_keyboard_hook: already installed");
        return;
    }
    log("install_keyboard_hook: starting thread");

    thread::spawn(|| {
        let thread_id = unsafe { winapi::um::processthreadsapi::GetCurrentThreadId() };
        *HOOK_THREAD_ID.lock().unwrap() = Some(thread_id);
        init_async_log();

        unsafe {
            // 获取当前模块句柄
            let hmod = GetModuleHandleW(std::ptr::null_mut());

            // 安装键盘钩子
            let kb_hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), hmod, 0);
            if kb_hook.is_null() {
                log("install_keyboard_hook: SetWindowsHookExW (keyboard) failed");
                HOOK_INSTALLED.store(0, Ordering::SeqCst);
                *HOOK_THREAD_ID.lock().unwrap() = None;
                return;
            }
            log("install_keyboard_hook: keyboard hook installed successfully");

            // 安装鼠标钩子
            let ms_hook = SetWindowsHookExW(WH_MOUSE_LL, Some(mouse_proc), hmod, 0);
            if ms_hook.is_null() {
                log("install_keyboard_hook: SetWindowsHookExW (mouse) failed");
                let _ = UnhookWindowsHookEx(kb_hook);
                HOOK_INSTALLED.store(0, Ordering::SeqCst);
                *HOOK_THREAD_ID.lock().unwrap() = None;
                return;
            }
            log("install_keyboard_hook: mouse hook installed successfully");

            let mut msg: MSG = std::mem::zeroed();
            // 保持线程存活并处理消息
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                TranslateMessage(&msg);
                DispatchMessageW(&msg);
            }

            log("install_keyboard_hook: message loop ended, uninstalling hooks");
            let _ = UnhookWindowsHookEx(kb_hook);
            let _ = UnhookWindowsHookEx(ms_hook);
            HOOK_INSTALLED.store(0, Ordering::SeqCst);
            *HOOK_THREAD_ID.lock().unwrap() = None;
        }
    });
}

pub fn uninstall_hooks() {
    log("uninstall_hooks: requesting hook thread to quit");
    if let Ok(guard) = HOOK_THREAD_ID.lock() {
        if let Some(thread_id) = *guard {
            unsafe {
                let result = PostThreadMessageW(thread_id, WM_QUIT, 0, 0);
                log(&format!("uninstall_hooks: PostThreadMessageW result = {}", result));
            }
        } else {
            log("uninstall_hooks: no hook thread running");
        }
    }
}

fn is_allowed_key(vk: i32) -> bool {
    (vk >= 0x30 && vk <= 0x39) // 主键盘 0-9
        || (vk >= 0x41 && vk <= 0x5A) // A-Z
        || (vk >= 0x60 && vk <= 0x69) // 小键盘 0-9
        || vk == VK_BACK as i32
        || vk == VK_RETURN as i32
        || vk == VK_SPACE as i32
        || vk == VK_CAPITAL as i32
        || vk == VK_DELETE as i32
        || vk == VK_HOME as i32
        || vk == VK_END as i32
        || vk == VK_LEFT as i32
        || vk == VK_RIGHT as i32
        || vk == VK_UP as i32
        || vk == VK_DOWN as i32
        || vk == VK_INSERT as i32
        || vk == VK_PRIOR as i32
        || vk == VK_NEXT as i32
        || vk == VK_ESCAPE as i32
}

unsafe extern "system" fn keyboard_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code < 0 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }

    let kb = &*(l_param as *const KBDLLHOOKSTRUCT);
    let vk = kb.vkCode;
    let wp = w_param as u32;
    let state = LOCK_STATE.load(Ordering::SeqCst);

    // 未锁屏：完全不拦截，极速返回
    if state == 0 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }

    // 所有按键释放事件必须放行，否则系统按键状态会卡死
    // （参考：HookManager.cpp 第 104-106 行注释 "必须传递，否则按键状态会卡住"）
    if wp == WM_KEYUP as u32 || wp == WM_SYSKEYUP as u32 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }

    // 只处理按键按下事件
    if wp == WM_KEYDOWN as u32 || wp == WM_SYSKEYDOWN as u32 {
        // 1. state == 1：密码框隐藏，拦截所有键（除了 ESC 用于切换）
        if state == 1 {
            if vk == VK_ESCAPE as u32 {
                return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
            }
            return 1;
        }

        // 2. state == 2：密码框显示，白名单模式放行允许输入的键
        if state == 2 {
            if is_allowed_key(vk as i32) {
                return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
            }
            return 1;
        }
    }

    // 默认放行
    CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param)
}

unsafe extern "system" fn mouse_proc(n_code: i32, w_param: WPARAM, l_param: LPARAM) -> LRESULT {
    if n_code < 0 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }
    
    let state = LOCK_STATE.load(Ordering::SeqCst);
    if state == 0 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }

    let wp = w_param as u32;

    // 允许鼠标移动
    if wp == WM_MOUSEMOVE as u32 {
        return CallNextHookEx(std::ptr::null_mut(), n_code, w_param, l_param);
    }

    // 拦截鼠标点击，并设置标志位通知前端显示提示
    if wp == WM_LBUTTONDOWN as u32
        || wp == WM_RBUTTONDOWN as u32
        || wp == WM_MBUTTONDOWN as u32
        || wp == WM_LBUTTONUP as u32
        || wp == WM_RBUTTONUP as u32
        || wp == WM_MBUTTONUP as u32
    {
        MOUSE_CLICKED.store(true, Ordering::SeqCst);
        return 1;
    }

    // 拦截其余鼠标事件（滚轮等）
    1
}