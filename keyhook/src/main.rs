#![windows_subsystem = "windows"]

use std::collections::VecDeque;
use std::ptr::null_mut;
use std::sync::Mutex;
use std::time::{Duration, Instant};
use std::ffi::OsStr;
use std::iter::once;
use std::os::windows::ffi::OsStrExt;
use winapi::shared::minwindef::{DWORD, HKEY, TRUE};
use winapi::shared::windef::HHOOK;
use winapi::um::libloaderapi::GetModuleHandleW;
use winapi::um::winuser::{
    CallNextHookEx, DispatchMessageW, GetMessageW, HC_ACTION, KBDLLHOOKSTRUCT, MSG,
    SetWindowsHookExW, TranslateMessage, UnhookWindowsHookEx, WH_KEYBOARD_LL,
    WM_KEYDOWN, WM_QUIT, WM_SYSKEYDOWN,
};
use winapi::um::winreg::{
    RegCloseKey, RegCreateKeyExW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, RegDeleteValueW,
    HKEY_CURRENT_USER,
};
use lazy_static::lazy_static;

// 常量定义
const KEY_READ: u32 = 0x20019;
const KEY_WRITE: u32 = 0x20006;
const REG_DWORD: u32 = 4;
const REPEAT_WINDOW_MS: u64 = 400;     // 时间窗口（毫秒）
const REQUIRED_REPEATS: usize = 5;     // 连按次数

// 需要检测辅助功能快捷键的键值
const STICKY_KEYS: &[u32] = &[0xA0, 0xA1]; // 左右 Shift
const FILTER_KEYS: &[u32] = &[0xA2, 0xA3]; // 左右 Ctrl (筛选键默认是右Shift长按，但一般也可拦截)
const TOGGLE_KEYS: &[u32] = &[0xA4, 0xA5]; // 左右 Alt

lazy_static! {
    static ref KEY_QUEUE: Mutex<VecDeque<(u32, Instant)>> = Mutex::new(VecDeque::new());
}

static mut HOOK_HANDLE: HHOOK = null_mut();

// ---------- 注册表操作 ----------
fn to_wide(s: &str) -> Vec<u16> {
    OsStr::new(s).encode_wide().chain(once(0)).collect()
}

fn read_reg_dword(key: HKEY, sub_key: &str, value_name: &str) -> Option<u32> {
    let sub_key_wide = to_wide(sub_key);
    let value_name_wide = to_wide(value_name);
    let mut hkey = null_mut();
    let result = unsafe {
        RegOpenKeyExW(key, sub_key_wide.as_ptr(), 0, KEY_READ, &mut hkey)
    };
    if result != 0 {
        return None;
    }
    let mut data: DWORD = 0;
    let mut size = std::mem::size_of::<DWORD>() as u32;
    let result = unsafe {
        RegQueryValueExW(
            hkey,
            value_name_wide.as_ptr(),
            null_mut(),
            null_mut(),
            &mut data as *mut _ as *mut u8,
            &mut size,
        )
    };
    unsafe { RegCloseKey(hkey); }
    if result == 0 { Some(data) } else { None }
}

fn set_reg_dword(key: HKEY, sub_key: &str, value_name: &str, value: u32) -> bool {
    let sub_key_wide = to_wide(sub_key);
    let value_name_wide = to_wide(value_name);
    let mut hkey = null_mut();
    let mut disp: u32 = 0;
    let result = unsafe {
        RegCreateKeyExW(
            key, sub_key_wide.as_ptr(), 0, null_mut(), 0,
            KEY_WRITE, null_mut(), &mut hkey, &mut disp,
        )
    };
    if result != 0 {
        return false;
    }
    let data = value;
    let size = std::mem::size_of::<DWORD>() as u32;
    let result = unsafe {
        RegSetValueExW(
            hkey, value_name_wide.as_ptr(), 0, REG_DWORD,
            &data as *const _ as *const u8, size,
        )
    };
    unsafe { RegCloseKey(hkey); }
    result == 0
}

fn delete_reg_value(key: HKEY, sub_key: &str, value_name: &str) {
    let sub_key_wide = to_wide(sub_key);
    let value_name_wide = to_wide(value_name);
    let mut hkey = null_mut();
    if unsafe { RegOpenKeyExW(key, sub_key_wide.as_ptr(), 0, KEY_WRITE, &mut hkey) } == 0 {
        unsafe {
            RegDeleteValueW(hkey, value_name_wide.as_ptr());
            RegCloseKey(hkey);
        }
    }
}

struct AccessibilityBackup {
    sticky_keys_orig: Option<u32>,
    filter_keys_orig: Option<u32>,
    toggle_keys_orig: Option<u32>,
    mouse_keys_orig: Option<u32>,
    narrator_orig: Option<u32>,
}

impl AccessibilityBackup {
    fn new_and_disable() -> Self {
        let sticky_orig = read_reg_dword(HKEY_CURRENT_USER, r"Control Panel\Accessibility\StickyKeys", "Flags");
        set_reg_dword(HKEY_CURRENT_USER, r"Control Panel\Accessibility\StickyKeys", "Flags", 0x3A);
        
        let filter_orig = read_reg_dword(HKEY_CURRENT_USER, r"Control Panel\Accessibility\FilterKeys", "Flags");
        set_reg_dword(HKEY_CURRENT_USER, r"Control Panel\Accessibility\FilterKeys", "Flags", 0x3A);
        
        let toggle_orig = read_reg_dword(HKEY_CURRENT_USER, r"Control Panel\Accessibility\ToggleKeys", "Flags");
        set_reg_dword(HKEY_CURRENT_USER, r"Control Panel\Accessibility\ToggleKeys", "Flags", 0x3A);
        
        let mouse_orig = read_reg_dword(HKEY_CURRENT_USER, r"Control Panel\Accessibility\MouseKeys", "Flags");
        set_reg_dword(HKEY_CURRENT_USER, r"Control Panel\Accessibility\MouseKeys", "Flags", 0x3A);
        
        let narrator_orig = read_reg_dword(HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Narrator\NoRoam", "WinEnterLaunchEnabled");
        set_reg_dword(HKEY_CURRENT_USER, r"SOFTWARE\Microsoft\Narrator\NoRoam", "WinEnterLaunchEnabled", 0);
        
        AccessibilityBackup {
            sticky_keys_orig: sticky_orig,
            filter_keys_orig: filter_orig,
            toggle_keys_orig: toggle_orig,
            mouse_keys_orig: mouse_orig,
            narrator_orig: narrator_orig,
        }
    }
    
    fn restore(&self) {
        let subkey = r"Control Panel\Accessibility\StickyKeys";
        if let Some(val) = self.sticky_keys_orig {
            set_reg_dword(HKEY_CURRENT_USER, subkey, "Flags", val);
        } else {
            delete_reg_value(HKEY_CURRENT_USER, subkey, "Flags");
        }
        
        let subkey = r"Control Panel\Accessibility\FilterKeys";
        if let Some(val) = self.filter_keys_orig {
            set_reg_dword(HKEY_CURRENT_USER, subkey, "Flags", val);
        } else {
            delete_reg_value(HKEY_CURRENT_USER, subkey, "Flags");
        }
        
        let subkey = r"Control Panel\Accessibility\ToggleKeys";
        if let Some(val) = self.toggle_keys_orig {
            set_reg_dword(HKEY_CURRENT_USER, subkey, "Flags", val);
        } else {
            delete_reg_value(HKEY_CURRENT_USER, subkey, "Flags");
        }
        
        let subkey = r"Control Panel\Accessibility\MouseKeys";
        if let Some(val) = self.mouse_keys_orig {
            set_reg_dword(HKEY_CURRENT_USER, subkey, "Flags", val);
        } else {
            delete_reg_value(HKEY_CURRENT_USER, subkey, "Flags");
        }
        
        let subkey = r"SOFTWARE\Microsoft\Narrator\NoRoam";
        if let Some(val) = self.narrator_orig {
            set_reg_dword(HKEY_CURRENT_USER, subkey, "WinEnterLaunchEnabled", val);
        } else {
            delete_reg_value(HKEY_CURRENT_USER, subkey, "WinEnterLaunchEnabled");
        }
    }
}

// ---------- 连按检测（辅助功能快捷键拦截）----------
fn check_and_block_accessibility(vk_code: u32) -> bool {
    // 只针对可能触发辅助功能的键
    let is_suspect = STICKY_KEYS.contains(&vk_code) 
                     || FILTER_KEYS.contains(&vk_code) 
                     || TOGGLE_KEYS.contains(&vk_code);
    if !is_suspect {
        return false;
    }

    let now = Instant::now();
    let mut queue = KEY_QUEUE.lock().unwrap();
    
    // 清理过期记录
    while let Some(&(_, time)) = queue.front() {
        if now.duration_since(time) > Duration::from_millis(REPEAT_WINDOW_MS) {
            queue.pop_front();
        } else {
            break;
        }
    }
    
    // 检查是否与上一个按键相同（连续按同一键）
    let same_as_last = queue.back().map_or(false, |&(code, _)| code == vk_code);
    if same_as_last {
        queue.push_back((vk_code, now));
    } else {
        // 不同键，清空并重新开始（或者不清空，但避免不同键混合计数）
        queue.clear();
        queue.push_back((vk_code, now));
    }
    
    // 如果队列长度达到阈值，则拦截
    if queue.len() >= REQUIRED_REPEATS {
        queue.clear(); // 拦截后清除，避免连续拦截
        return true;
    }
    
    false
}

fn is_allowed_key(vk_code: u32) -> bool {
    match vk_code {
        0x08 | 0x0D | 0x20 | 0x2E | 0xBE => true, // Back, Enter, Space, Del, '.'
        0x1B => true, // ESC 键
        0x14 => true, // Caps Lock
        0x30..=0x39 => true, // 数字
        0x41..=0x5A => true, // A-Z
        _ => false,
    }
}

unsafe extern "system" fn keyboard_hook_proc(
    code: i32,
    w_param: usize,
    l_param: isize,
) -> isize {
    if code == HC_ACTION {
        let kb_struct = &*(l_param as *const KBDLLHOOKSTRUCT);
        let vk_code = kb_struct.vkCode;
        
        if w_param == WM_KEYDOWN as usize || w_param == WM_SYSKEYDOWN as usize {
            // 1. 优先检测辅助功能连按（粘滞键等）
            if check_and_block_accessibility(vk_code) {
                return 1; // 拦截
            }
            
            // 2. 只允许输入类按键
            if !is_allowed_key(vk_code) {
                return 1;
            }
        }
    }
    CallNextHookEx(HOOK_HANDLE, code, w_param, l_param)
}

fn main() {
    // 禁用注册表中的辅助功能选项（辅助手段）
    let backup = AccessibilityBackup::new_and_disable();
    
    unsafe {
        let h_instance = GetModuleHandleW(null_mut());
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_hook_proc), h_instance, 0);
        if hook.is_null() {
            backup.restore();
            return;
        }
        HOOK_HANDLE = hook;
        
        let mut msg: MSG = std::mem::zeroed();
        while GetMessageW(&mut msg, null_mut(), 0, 0) != 0 {
            if msg.message == WM_QUIT {
                break;
            }
            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
        
        UnhookWindowsHookEx(HOOK_HANDLE);
    }
    
    backup.restore();
}