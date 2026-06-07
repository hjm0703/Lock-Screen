mod hooks;

use base64::Engine;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use std::sync::OnceLock;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Emitter, Manager, State, WebviewUrl, WebviewWindowBuilder,
};

#[derive(Serialize, Deserialize, Clone)]
struct AppSettings {
    password_hash: Option<String>,
    password_hint: Option<String>,
    auto_hide: bool,
    breathing_light: bool,
    bg_mode: String,
    bg_image_file: Option<String>,
    bg_image_opacity_overlay: f64,
    bg_image_opacity_dimmed: f64,
    clock_visible: bool,
    welcome_screen: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            password_hash: None,
            password_hint: None,
            auto_hide: false,
            breathing_light: true,
            bg_mode: "none".to_string(),
            bg_image_file: None,
            bg_image_opacity_overlay: 1.0,
            bg_image_opacity_dimmed: 1.0,
            clock_visible: true,
            welcome_screen: false,
        }
    }
}

struct AppState {
    settings: Mutex<AppSettings>,
    hook_process: Mutex<Option<Child>>,
}

static APP_HANDLE: OnceLock<tauri::AppHandle> = OnceLock::new();

fn get_settings_path() -> PathBuf {
    let mut path = std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf();
    path.push("settings.json");
    path
}

fn get_exe_dir() -> PathBuf {
    std::env::current_exe()
        .unwrap_or_else(|_| PathBuf::from("."))
        .parent()
        .unwrap_or_else(|| std::path::Path::new("."))
        .to_path_buf()
}

fn get_hook_exe_path() -> PathBuf {
    let mut path = get_exe_dir();
    path.push("resources");
    path.push("keyhook.exe");
    path
}

fn get_images_dir() -> PathBuf {
    let mut path = get_exe_dir();
    path.push("images");
    path
}

fn read_image_as_data_url(path: &PathBuf) -> Option<String> {
    let ext = path.extension()?.to_str()?.to_lowercase();
    let mime = match ext.as_str() {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "bmp" => "image/bmp",
        "gif" => "image/gif",
        "webp" => "image/webp",
        _ => return None,
    };
    let bytes = fs::read(path).ok()?;
    let b64 = base64::prelude::BASE64_STANDARD.encode(&bytes);
    Some(format!("data:{};base64,{}", mime, b64))
}

fn load_settings() -> AppSettings {
    let path = get_settings_path();
    if let Ok(content) = fs::read_to_string(&path) {
        if let Ok(settings) = serde_json::from_str(&content) {
            return settings;
        }
    }
    AppSettings::default()
}

fn save_settings(settings: &AppSettings) -> Result<(), String> {
    let path = get_settings_path();
    let content = serde_json::to_string_pretty(settings)
        .map_err(|e| format!("序列化失败: {}", e))?;
    fs::write(&path, content)
        .map_err(|e| format!("写入失败: {}", e))?;
    Ok(())
}

fn hash_password(password: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(password.as_bytes());
    let result = hasher.finalize();
    hex::encode(result)
}

#[tauri::command]
fn set_password(
    password: String,
    old_password: Option<String>,
    state: State<AppState>,
) -> Result<(), String> {
    hooks::turn_off_caps_lock();

    let mut settings = state.settings.lock().unwrap();

    // 如果已设置密码，需要验证原密码
    if let Some(ref existing_hash) = settings.password_hash {
        let provided_old = old_password.ok_or("需要输入原密码")?;
        let provided_hash = hash_password(&provided_old);
        if &provided_hash != existing_hash {
            return Err("原密码错误".to_string());
        }
    }

    let hash = hash_password(&password);
    settings.password_hash = Some(hash);
    save_settings(&settings)?;
    Ok(())
}

#[tauri::command]
fn set_password_hint(hint: Option<String>, state: State<AppState>) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    settings.password_hint = hint.filter(|s| !s.is_empty());
    save_settings(&settings)?;
    Ok(())
}

#[tauri::command]
fn verify_password(password: String, state: State<AppState>) -> Result<bool, String> {
    hooks::turn_off_caps_lock();

    let settings = state.settings.lock().unwrap();
    if let Some(ref hash) = settings.password_hash {
        let input_hash = hash_password(&password);
        Ok(&input_hash == hash)
    } else {
        Ok(false)
    }
}

#[tauri::command]
fn has_password(state: State<AppState>) -> Result<bool, String> {
    let settings = state.settings.lock().unwrap();
    Ok(settings.password_hash.is_some())
}

#[tauri::command]
fn get_settings(state: State<AppState>) -> Result<AppSettings, String> {
    let settings = state.settings.lock().unwrap();
    Ok(settings.clone())
}

#[tauri::command]
fn update_setting(key: String, value: f64, state: State<AppState>) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    match key.as_str() {
        "auto_hide" => settings.auto_hide = value > 0.5,
        "breathing_light" => settings.breathing_light = value > 0.5,
        "bg_image_opacity_overlay" => settings.bg_image_opacity_overlay = value.clamp(0.0, 1.0),
        "bg_image_opacity_dimmed" => settings.bg_image_opacity_dimmed = value.clamp(0.0, 1.0),
        "clock_visible" => settings.clock_visible = value > 0.5,
        "welcome_screen" => settings.welcome_screen = value > 0.5,
        _ => return Err(format!("未知设置项: {}", key)),
    }
    save_settings(&settings)?;
    Ok(())
}

#[tauri::command]
fn set_bg_mode(mode: String, state: State<AppState>) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    settings.bg_mode = mode;
    save_settings(&settings)?;
    Ok(())
}

#[tauri::command]
fn set_bg_image_file(filename: Option<String>, state: State<AppState>) -> Result<(), String> {
    let mut settings = state.settings.lock().unwrap();
    settings.bg_image_file = filename;
    save_settings(&settings)?;
    Ok(())
}

#[tauri::command]
fn list_background_images() -> Result<Vec<String>, String> {
    let dir = get_images_dir();
    if !dir.exists() {
        return Ok(Vec::new());
    }
    let mut images = Vec::new();
    let entries = fs::read_dir(&dir).map_err(|e| format!("读取图片目录失败: {}", e))?;
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_file() {
            if let Some(ext) = path.extension() {
                let ext = ext.to_str().unwrap_or("").to_lowercase();
                if matches!(ext.as_str(), "png" | "jpg" | "jpeg" | "bmp" | "gif" | "webp") {
                    if let Some(name) = path.file_name() {
                        if let Some(name_str) = name.to_str() {
                            images.push(name_str.to_string());
                        }
                    }
                }
            }
        }
    }
    images.sort();
    Ok(images)
}

#[tauri::command]
fn import_wallpaper(file_name: String, bytes: Vec<u8>) -> Result<(), String> {
    let dir = get_images_dir();
    if !dir.exists() {
        fs::create_dir_all(&dir).map_err(|e| format!("创建 images 目录失败: {}", e))?;
    }
    let dest = dir.join(&file_name);
    fs::write(&dest, &bytes).map_err(|e| format!("写入图片失败: {}", e))?;
    Ok(())
}

#[derive(Deserialize)]
struct BingImageResponse {
    images: Vec<BingImage>,
}

#[derive(Deserialize)]
struct BingImage {
    urlbase: String,
}

async fn fetch_bing_wallpaper_url() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .build()
        .ok()?;

    let resp = client
        .get("https://www.bing.com/HPImageArchive.aspx?format=js&idx=0&n=1&mkt=zh-CN")
        .send()
        .await
        .ok()?;

    let data: BingImageResponse = resp.json().await.ok()?;
    let image = data.images.first()?;
    Some(format!("https://www.bing.com{}_UHD.jpg", image.urlbase))
}

async fn internal_start_lock(app: &tauri::AppHandle, settings: &AppSettings) -> Result<(), String> {
    let breathing_light = settings.breathing_light;
    let bg_image_file = settings.bg_image_file.clone();
    let bg_image_opacity_overlay = settings.bg_image_opacity_overlay;
    let bg_image_opacity_dimmed = settings.bg_image_opacity_dimmed;
    let clock_visible = settings.clock_visible;
    let bg_mode = settings.bg_mode.clone();

    let timestamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    hooks::set_lock_state(2);

    // 先杀掉可能残留的旧进程
    let state = app.state::<AppState>();
    if let Some(mut child) = state.hook_process.lock().unwrap().take() {
        let _ = child.kill();
        let _ = child.wait();
    }
    drop(state);

    // 启动外部钩子 EXE，封禁 Win 键等系统快捷键
    let hook_path = get_hook_exe_path();
    if hook_path.exists() {
        match Command::new(&hook_path).spawn() {
            Ok(child) => {
                let state = app.state::<AppState>();
                *state.hook_process.lock().unwrap() = Some(child);
                eprintln!("[LockScreen] 钩子进程已启动: {:?}", hook_path);
            }
            Err(e) => {
                eprintln!("[LockScreen] 启动钩子进程失败: {}", e);
            }
        }
    } else {
        eprintln!("[LockScreen] 钩子 EXE 未找到: {:?}", hook_path);
    }

    let mut bg_image_url = None;

    match bg_mode.as_str() {
        "custom" => {
            bg_image_url = bg_image_file.as_ref().and_then(|filename| {
                let img_path = get_images_dir().join(filename);
                if img_path.exists() {
                    read_image_as_data_url(&img_path)
                } else {
                    None
                }
            });
        }
        "bing" => {
            if let Some(url) = fetch_bing_wallpaper_url().await {
                bg_image_url = Some(url);
            }
        }
        _ => {}
    }

    let welcome_screen = settings.welcome_screen;

    let js = format!(
        "window.__breathingLight = {}; \
         window.__bgImageUrl = {}; \
         window.__bgImageOpacityOverlay = {}; window.__bgImageOpacityDimmed = {}; \
         window.__clockVisible = {}; \
         window.__lockTimestamp = {}; \
         window.__passwordHint = {}; \
         window.__welcomeScreen = {}; \
         const tsEl = document.getElementById('lock-timestamp'); \
         if (tsEl) {{ tsEl.textContent = String(window.__lockTimestamp); }} \
         const hintEl = document.getElementById('password-hint-display'); \
         if (hintEl) {{ hintEl.textContent = window.__passwordHint || ''; }} \
         const el = document.getElementById('lock-overlay'); \
         if (el) {{ \
             if (window.__breathingLight) {{ el.classList.add('breathing'); }} \
             else {{ el.classList.remove('breathing'); }} \
             if (!window.__clockVisible) {{ el.classList.add('clock-hidden'); }} \
             else {{ el.classList.remove('clock-hidden'); }} \
             const bgImg = document.getElementById('lock-bg-img'); \
             if (bgImg) {{ \
                 if (window.__bgImageUrl) {{ \
                     bgImg.src = window.__bgImageUrl; \
                     bgImg.style.setProperty('--bg-image-opacity-overlay', String(window.__bgImageOpacityOverlay)); \
                     bgImg.style.setProperty('--bg-image-opacity-dimmed', String(window.__bgImageOpacityDimmed)); \
                     el.classList.add('has-bg-image'); \
                     bgImg.classList.add('show'); \
                 }} else {{ \
                     el.classList.remove('has-bg-image'); \
                     bgImg.classList.remove('show'); \
                 }} \
             }} \
         }} \
         const welcomeEl = document.getElementById('welcome-screen'); \
         if (welcomeEl) {{ welcomeEl.classList.remove('active'); }}",
        breathing_light,
        match &bg_image_url {
            Some(url) => format!("'{}'", url),
            None => "null".to_string(),
        },
        bg_image_opacity_overlay,
        bg_image_opacity_dimmed,
        if clock_visible { "true" } else { "false" },
        timestamp,
        match &settings.password_hint {
            Some(hint) => format!("'{}'", hint.replace('\\', "\\\\").replace('\'', "\\'")),
            None => "null".to_string(),
        },
        if welcome_screen { "true" } else { "false" }
    );

    if let Some(window) = app.get_webview_window("lock") {
        let _ = window.eval(&js);
        let _ = window.show();
        let _ = window.set_focus();
        let _ = window.emit("lock-timestamp", timestamp);
        return Ok(());
    }

    let monitor = app
        .primary_monitor()
        .map_err(|e| format!("获取显示器失败: {}", e))?
        .ok_or("未找到主显示器")?;

    let size = monitor.size();

    let window = WebviewWindowBuilder::new(app, "lock", WebviewUrl::App("/lock.html".into()))
        .title("")
        .decorations(false)
        .transparent(true)
        .always_on_top(true)
        .skip_taskbar(true)
        .fullscreen(true)
        .inner_size(size.width as f64, size.height as f64)
        .position(0.0, 0.0)
        .visible(true)
        .build()
        .map_err(|e| format!("创建锁屏窗口失败: {}", e))?;

    let _ = window.eval(&js);
    let _ = window.emit("lock-timestamp", timestamp);

    Ok(())
}

#[tauri::command]
async fn start_lock_screen(app: tauri::AppHandle, state: State<'_, AppState>) -> Result<(), String> {
    let settings = state.settings.lock().unwrap().clone();
    if settings.password_hash.is_none() {
        return Err("请先设置密码后再使用锁屏功能".to_string());
    }
    internal_start_lock(&app, &settings).await
}

fn register_global_hotkey(_app: &tauri::AppHandle) {
    std::thread::spawn(move || {
        unsafe {
            use winapi::um::winuser::{GetMessageW, RegisterHotKey, MSG};
            const MOD_CONTROL: u32 = 0x0002;
            const MOD_ALT: u32 = 0x0001;
            const MOD_NOREPEAT: u32 = 0x4000;
            const WM_HOTKEY: u32 = 0x0312;

            let success = RegisterHotKey(
                std::ptr::null_mut(),
                1,
                MOD_CONTROL | MOD_ALT | MOD_NOREPEAT,
                0x4C, // 'L'
            );
            if success == 0 {
                eprintln!("[LockScreen] 注册全局快捷键 Ctrl+Alt+L 失败");
                return;
            }
            eprintln!("[LockScreen] 全局快捷键 Ctrl+Alt+L 已注册");

            let mut msg: MSG = std::mem::zeroed();
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {
                if msg.message == WM_HOTKEY && msg.wParam as i32 == 1 {
                    eprintln!("[LockScreen] 全局快捷键触发锁屏");
                    if let Some(app_handle) = APP_HANDLE.get() {
                        let app_handle = app_handle.clone();
                        let state = app_handle.state::<AppState>();
                        let settings = state.settings.lock().unwrap().clone();
                        drop(state);
                        if settings.password_hash.is_none() {
                            eprintln!("[LockScreen] 未设置密码，拒绝锁屏");
                            continue;
                        }
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let _ = rt.block_on(internal_start_lock(&app_handle, &settings));
                    }
                }
            }
        }
    });
}

#[tauri::command]
fn poll_mouse_click() -> bool {
    hooks::poll_mouse_click()
}

#[tauri::command]
fn ensure_caps_lock_off() {
    hooks::turn_off_caps_lock();
}

fn kill_hook_process(state: &State<AppState>) {
    if let Some(mut child) = state.hook_process.lock().unwrap().take() {
        eprintln!("[LockScreen] 正在终止钩子进程...");
        let _ = child.kill();
        let _ = child.wait();
        eprintln!("[LockScreen] 钩子进程已终止");
    }
}

#[cfg(windows)]
fn cleanup_duplicate_processes() {
    use std::ffi::OsString;
    use std::os::windows::ffi::OsStringExt;
    use winapi::shared::minwindef::{DWORD, FALSE, TRUE};
    use winapi::um::handleapi::CloseHandle;
    use winapi::um::processthreadsapi::{GetCurrentProcessId, OpenProcess};
    use winapi::um::tlhelp32::{
        CreateToolhelp32Snapshot, Process32FirstW, Process32NextW, PROCESSENTRY32W,
        TH32CS_SNAPPROCESS,
    };
    use winapi::um::winnt::PROCESS_TERMINATE;

    let current_pid = unsafe { GetCurrentProcessId() };

    // 获取当前进程名
    let current_exe = std::env::current_exe().unwrap_or_default();
    let current_name = current_exe
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("lock-screen.exe")
        .to_lowercase();

    eprintln!("[LockScreen] 当前进程名: {}, PID: {}", current_name, current_pid);

    unsafe {
        let snapshot = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0);
        if snapshot.is_null() {
            eprintln!("[LockScreen] CreateToolhelp32Snapshot 失败");
            return;
        }

        let mut entry: PROCESSENTRY32W = std::mem::zeroed();
        entry.dwSize = std::mem::size_of::<PROCESSENTRY32W>() as DWORD;

        if Process32FirstW(snapshot, &mut entry) == TRUE {
            loop {
                let pid = entry.th32ProcessID;
                let exe_file = OsString::from_wide(
                    &entry.szExeFile[..entry.szExeFile.iter().position(|&c| c == 0).unwrap_or(260)]
                );
                let exe_name = exe_file.to_string_lossy().to_lowercase();

                // 终止同名进程（排除当前进程）
                if exe_name == current_name && pid != current_pid {
                    eprintln!("[LockScreen] 发现同名进程: {} (PID: {})，准备终止", exe_name, pid);
                    let handle = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
                    if !handle.is_null() {
                        let result = winapi::um::processthreadsapi::TerminateProcess(handle, 1);
                        if result != 0 {
                            eprintln!("[LockScreen] 已终止同名进程 PID: {}", pid);
                        } else {
                            eprintln!("[LockScreen] 终止同名进程 PID: {} 失败", pid);
                        }
                        CloseHandle(handle);
                    }
                }

                // 终止所有 keyhook.exe
                if exe_name == "keyhook.exe" {
                    eprintln!("[LockScreen] 发现 keyhook.exe (PID: {})，准备终止", pid);
                    let handle = OpenProcess(PROCESS_TERMINATE, FALSE, pid);
                    if !handle.is_null() {
                        let result = winapi::um::processthreadsapi::TerminateProcess(handle, 1);
                        if result != 0 {
                            eprintln!("[LockScreen] 已终止 keyhook.exe PID: {}", pid);
                        } else {
                            eprintln!("[LockScreen] 终止 keyhook.exe PID: {} 失败", pid);
                        }
                        CloseHandle(handle);
                    }
                }

                if Process32NextW(snapshot, &mut entry) != TRUE {
                    break;
                }
            }
        }

        CloseHandle(snapshot);
    }

    eprintln!("[LockScreen] 进程清理完成");
}

#[cfg(not(windows))]
fn cleanup_duplicate_processes() {
    // 非 Windows 平台暂不支持
}

#[tauri::command]
fn unlock_screen(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    if let Some(window) = app.get_webview_window("lock") {
        let _ = window.hide();
    }
    hooks::set_lock_state(0);
    kill_hook_process(&state);
    Ok(())
}

#[tauri::command]
fn set_password_visible(visible: bool) -> Result<(), String> {
    if visible {
        hooks::set_lock_state(2);
    } else {
        hooks::set_lock_state(1);
    }
    Ok(())
}

fn setup_tray(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let show_item = MenuItem::with_id(app, "show", "显示窗口", true, None::<&str>)?;
    let quit_item = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&show_item, &quit_item])?;

    let _tray = TrayIconBuilder::new()
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(|app, event| match event.id.as_ref() {
            "show" => toggle_window_visibility(app),
            "quit" => {
                hooks::uninstall_hooks();
                app.exit(0);
            }
            _ => {}
        })
        .build(app)?;

    Ok(())
}

fn toggle_window_visibility(app: &tauri::AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let is_visible = window.is_visible().unwrap_or(true);
        if is_visible {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

fn setup_window_events(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(window) = app.get_webview_window("main") {
        let window_clone = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window_clone.hide();
            }
        });
    }
    Ok(())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    cleanup_duplicate_processes();

    let settings = load_settings();
    let app_state = AppState {
        settings: Mutex::new(settings),
        hook_process: Mutex::new(None),
    };

    let app = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            set_password,
            set_password_hint,
            set_bg_mode,
            verify_password,
            has_password,
            get_settings,
            update_setting,
            start_lock_screen,
            unlock_screen,
            set_password_visible,
            poll_mouse_click,
            list_background_images,
            set_bg_image_file,
            import_wallpaper,
            ensure_caps_lock_off
        ])
        .setup(|app| {
            let _ = APP_HANDLE.set(app.handle().clone());
            setup_tray(app)?;
            setup_window_events(app)?;
            hooks::install_keyboard_hook();
            register_global_hotkey(app.handle());
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application");

    app.run(|_app_handle, event| {
        if let tauri::RunEvent::ExitRequested { .. } = event {
            hooks::uninstall_hooks();
        }
    });
}
