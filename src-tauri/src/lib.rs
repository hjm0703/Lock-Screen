mod hooks;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;
use std::process::{Child, Command};
use std::sync::Mutex;
use tauri::{
    menu::{Menu, MenuItem},
    tray::TrayIconBuilder,
    Manager, State, WebviewUrl, WebviewWindowBuilder,
};

#[derive(Serialize, Deserialize, Clone)]
struct AppSettings {
    password_hash: Option<String>,
    auto_hide: bool,
    overlay_opacity: f64,
    dimmed_opacity: f64,
    breathing_light: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            password_hash: None,
            auto_hide: false,
            overlay_opacity: 0.55,
            dimmed_opacity: 0.85,
            breathing_light: true,
        }
    }
}

struct AppState {
    settings: Mutex<AppSettings>,
    hook_process: Mutex<Option<Child>>,
}

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
fn verify_password(password: String, state: State<AppState>) -> Result<bool, String> {
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
        "overlay_opacity" => settings.overlay_opacity = value.clamp(0.0, 1.0),
        "dimmed_opacity" => settings.dimmed_opacity = value.clamp(0.0, 1.0),
        "breathing_light" => settings.breathing_light = value > 0.5,
        _ => return Err(format!("未知设置项: {}", key)),
    }
    save_settings(&settings)?;
    Ok(())
}

#[tauri::command]
fn start_lock_screen(app: tauri::AppHandle, state: State<AppState>) -> Result<(), String> {
    let settings = state.settings.lock().unwrap();
    let overlay_opacity = settings.overlay_opacity;
    let dimmed_opacity = settings.dimmed_opacity;
    let breathing_light = settings.breathing_light;
    drop(settings);

    hooks::set_lock_state(2);

    // 先杀掉可能残留的旧进程
    kill_hook_process(&state);

    // 启动外部钩子 EXE，封禁 Win 键等系统快捷键
    let hook_path = get_hook_exe_path();
    if hook_path.exists() {
        match Command::new(&hook_path).spawn() {
            Ok(child) => {
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

    let js = format!(
        "window.__overlayOpacity = {}; window.__dimmedOpacity = {}; window.__breathingLight = {}; \
         const el = document.getElementById('lock-overlay'); \
         if (el) {{ \
             el.style.setProperty('--overlay-opacity', '{}'); \
             el.style.setProperty('--dimmed-opacity', '{}'); \
             if (window.__breathingLight) {{ el.classList.add('breathing'); }} \
             else {{ el.classList.remove('breathing'); }} \
         }}",
        overlay_opacity, dimmed_opacity, breathing_light,
        overlay_opacity, dimmed_opacity
    );

    if let Some(window) = app.get_webview_window("lock") {
        let _ = window.eval(&js);
        let _ = window.show();
        let _ = window.set_focus();
        return Ok(());
    }

    let monitor = app
        .primary_monitor()
        .map_err(|e| format!("获取显示器失败: {}", e))?
        .ok_or("未找到主显示器")?;

    let size = monitor.size();

    let window = WebviewWindowBuilder::new(&app, "lock", WebviewUrl::App("/lock.html".into()))
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

    Ok(())
}

#[tauri::command]
fn poll_mouse_click() -> bool {
    hooks::poll_mouse_click()
}

fn kill_hook_process(state: &State<AppState>) {
    if let Some(mut child) = state.hook_process.lock().unwrap().take() {
        eprintln!("[LockScreen] 正在终止钩子进程...");
        let _ = child.kill();
        let _ = child.wait();
        eprintln!("[LockScreen] 钩子进程已终止");
    }
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
            "quit" => app.exit(0),
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
    let settings = load_settings();
    let app_state = AppState {
        settings: Mutex::new(settings),
        hook_process: Mutex::new(None),
    };

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .manage(app_state)
        .invoke_handler(tauri::generate_handler![
            set_password,
            verify_password,
            has_password,
            get_settings,
            update_setting,
            start_lock_screen,
            unlock_screen,
            set_password_visible,
            poll_mouse_click
        ])
        .setup(|app| {
            setup_tray(app)?;
            setup_window_events(app)?;
            hooks::install_keyboard_hook();
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
