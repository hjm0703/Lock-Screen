# Lock Screen 项目开发指南

> 本文档为 AI 助手提供参考，说明项目架构、开发规范和注意事项。

## 项目概述

本项目是一个基于 Tauri v2 的后台设置应用，采用后台运行架构。程序启动后在系统托盘运行，点击窗口关闭按钮时隐藏窗口而非退出程序。当前只保留后台系统和设置界面（含密码设置）。

## 技术栈

| 层 | 技术 |
|---|---|
| 后端 | Rust + Tauri v2 |
| 前端 | TypeScript + Vite（无框架，原生 DOM） |
| 构建 | Cargo + npm |
| 目标平台 | Windows（当前） |

## 项目结构

```
Lock Screen/
├── src-tauri/                    # Tauri 后端
│   ├── src/
│   │   ├── main.rs               # 入口点（仅调用 lib.rs）
│   │   ├── lib.rs                # 核心逻辑（托盘、窗口管理、Tauri 命令、钩子状态管理、进程清理）
│   │   └── hooks.rs              # 底层键盘钩子 + 鼠标钩子 + 大写锁定控制 + 钩子卸载
│   ├── capabilities/
│   │   ├── default.json          # 主窗口权限配置
│   │   └── lock.json             # 锁屏窗口权限配置
│   ├── resources/                # 打包时附带的外部资源
│   │   └── keyhook.exe           # 外部 Win 键拦截 EXE（打包时自动包含）
│   ├── icons/                    # 应用图标
│   ├── Cargo.toml                # Rust 依赖
│   ├── build.rs                  # Tauri 构建脚本
│   └── tauri.conf.json           # Tauri 配置
├── src/                          # 前端源码
│   ├── main.ts                   # TypeScript 入口（设置界面）
│   ├── lock.ts                   # 锁屏窗口逻辑
│   ├── styles.css                # 设置界面样式
│   └── lock.css                  # 锁屏窗口样式
├── index.html                    # HTML 入口（设置界面，包含 SVG 图标定义）
├── lock.html                     # 锁屏窗口 HTML
├── package.json                  # Node 依赖
└── vite.config.ts                # Vite 配置
```

## 核心架构设计

### 后台运行模式

程序采用"后台常驻"架构：

```
用户点击关闭按钮（前端或系统关闭）
    → 调用 window.hide()
    → 窗口隐藏到系统托盘
    → 程序继续在托盘运行

用户点击托盘"退出"
    → 调用 hooks::uninstall_hooks() 卸载系统钩子
    → 调用 app.exit(0)
    → 程序完全退出
```

### 窗口配置

#### 主窗口 (main)
- 无框设计（`decorations: false`），自定义标题栏包含最小化和关闭按钮
- **最小化按钮**：调用 `window.minimize()` 最小化窗口
- **关闭按钮**：调用 `window.hide()` 隐藏到系统托盘
- **托盘"显示窗口"菜单项**：切换窗口显示/隐藏

#### 锁屏窗口 (lock)
- 全屏覆盖，透明背景（`transparent: true`, `fullscreen: true`）
- 始终置顶（`alwaysOnTop: true`）
- 不显示在任务栏（`skipTaskbar: true`）
- **解锁时调用 `window.hide()` 而非 `window.close()`**，保持窗口实例复用，确保透明效果一致
- 窗口在 `tauri.conf.json` 的 `app.windows` 数组中预注册

### 数据持久化

- 配置数据存储在 **EXE 同级目录的 `settings.json`**
- 使用 `serde` 和 `serde_json` 进行序列化/反序列化
- 设置变更后立即保存到文件

### 密码管理

- 密码使用 **SHA-256** 算法 hash 后存储，不存储明文
- 密码 hash 保存在 `settings.json` 的 `password_hash` 字段
- **修改密码需要验证原密码**（如果已设置密码）
- **大写锁定强制关闭**：密码输入框获得焦点时自动关闭大写锁定，前端显示大写锁定状态提示

### 全局快捷键

注册系统级快捷键 `Ctrl + Alt + L` 快速触发锁屏：
- 使用 Windows `RegisterHotKey` API 注册
- 在独立后台线程中通过 `GetMessageW` 消息泵监听 `WM_HOTKEY`
- 快捷键触发时通过全局 `APP_HANDLE` 调用 `internal_start_lock()`
- 快捷键线程随应用启动而启动，应用退出时自动终止

### 键盘钩子系统 (hooks.rs)

底层使用两个全局钩子：`WH_KEYBOARD_LL`（键盘）和 `WH_MOUSE_LL`（鼠标），在独立线程中运行消息循环。

**状态定义：**
- `0` = 未锁屏（正常，不拦截任何键）
- `1` = 锁屏显示，密码框隐藏（拦截所有键，仅保留 ESC）
- `2` = 锁屏显示，密码框可见（只允许数字/字母/退格/回车/空格/方向键等，不包含 Shift/Tab）

**关键实现细节：**
- 使用 `GetModuleHandleW(null)` 作为 `SetWindowsHookExW` 的 hMod 参数
- 键盘钩子只拦截 `WM_KEYDOWN` / `WM_SYSKEYDOWN`，**必须放行 `WM_KEYUP` / `WM_SYSKEYUP`**（否则系统按键状态卡死）
- **Win 键不由 Rust 钩子拦截**，Win 键事件穿透到系统，由外部 `keyhook.exe` 负责封禁
- 鼠标钩子允许 `WM_MOUSEMOVE` 通过（光标可自由移动），但拦截所有鼠标点击事件（`WM_LBUTTONDOWN` 等），点击时设置 `MOUSE_CLICKED` 原子标志供前端轮询
- 钩子线程运行 `GetMessageW` 消息泵，这是系统分发钩子事件的必要条件
- 使用 `AtomicU8` 存储状态，`AtomicBool` 存储点击标志，`AtomicU8` 防止钩子重复安装
- **钩子卸载**：通过 `PostThreadMessageW(thread_id, WM_QUIT)` 向钩子线程发送退出消息，触发钩子卸载和线程终止

**日志：** 钩子运行日志写入 **EXE 同级目录的 `hook.log`**，包含安装状态、状态变更、每次按键的拦截/放行决策。

### 外部 Win 键拦截 (keyhook.exe)

Rust 钩子不负责拦截 Win 键，改为调用外部 EXE 进程：

```
锁屏 → start_lock_screen → spawn keyhook.exe（位于 EXE 目录下的 resources/）
解锁 → unlock_screen    → kill 该进程
```

EXE 需命名为 `keyhook.exe`，放在 `src-tauri/resources/` 目录下。打包时通过 `tauri.conf.json` 的 `bundle.resources` 自动包含。

### 输入法管理

锁屏时自动强制切换为英文输入，解锁时恢复原输入法：
- 锁屏调用 `set_lock_state(2)` 时 → 保存当前输入法 → `LoadKeyboardLayoutW("00000409")` + `ActivateKeyboardLayout` 强制切为英文
- 解锁调用 `set_lock_state(0)` 时 → 恢复锁屏前保存的输入法

### 大写锁定管理

- 后端提供 `is_caps_lock_on()` 检测大写锁定状态
- 后端提供 `turn_off_caps_lock()` 使用 `SendInput` API 强制关闭大写锁定
- 后端命令 `ensure_caps_lock_off()` 供前端调用
- `set_password` 和 `verify_password` 命令执行时自动关闭大写锁定
- 前端密码输入框 `focus` 事件触发 `ensure_caps_lock_off`
- 前端通过 `e.getModifierState("CapsLock")` 检测并显示大写锁定提示

### 进程清理（启动时）

程序启动时自动执行进程清理，防止重复实例和残留钩子进程：
- 使用 `CreateToolhelp32Snapshot` + `Process32FirstW`/`Process32NextW` 枚举所有进程
- 终止所有同名进程（排除当前进程自身）
- 终止所有 `keyhook.exe` 进程
- 保留当前新启动的程序实例

### 时间戳显示

每次开启锁屏时生成 Unix 时间戳（秒级），通过 Tauri `emit` 事件传递给前端：
- 后端 `internal_start_lock()` 生成时间戳
- 通过 `window.emit("lock-timestamp", timestamp)` 发送
- 前端 `listen<number>("lock-timestamp", ...)` 接收并显示在锁屏界面右下角
- 时间戳仅在密码框显示时可见（通过 CSS 控制）

### 代码组织结构

`lib.rs` 采用函数分离设计，每个功能独立封装：

```rust
struct AppSettings          // 配置数据结构
struct AppState             // 应用状态（包含 Mutex<AppSettings> + Mutex<Option<Child>> 钩子进程）

fn get_settings_path()      // 获取 EXE 同级目录 settings.json 路径
fn get_hook_exe_path()      // 获取 EXE 同级目录 resources/keyhook.exe 路径
fn get_images_dir()         // 获取 EXE 同级目录 images/ 路径
fn load_settings()          // 加载配置
fn save_settings()          // 保存配置
fn hash_password()          // 密码 SHA-256 hash
fn read_image_as_data_url() // 读取本地图片文件并转为 base64 data URL

async fn fetch_bing_wallpaper_url()  // 从 Bing API 获取每日壁纸 URL
async fn internal_start_lock()       // 启动锁屏（异步，获取 Bing 壁纸）

#[tauri::command]
fn set_password()           // 设置密码（hash 存储，修改需验证原密码，自动关闭大写锁定）
#[tauri::command]
fn verify_password()        // 验证密码（自动关闭大写锁定）
#[tauri::command]
fn has_password()           // 检查是否已设置密码
#[tauri::command]
fn get_settings()           // 获取所有配置
#[tauri::command]
fn update_setting()         // 更新单个配置项
#[tauri::command]
async fn start_lock_screen() // 启动锁屏窗口 + spawn keyhook.exe + 生成时间戳
#[tauri::command]
fn unlock_screen()          // 解锁：隐藏锁屏窗口 + 终止 keyhook.exe + 恢复输入法
#[tauri::command]
fn set_password_visible()   // 通知后端密码框显示/隐藏状态变更
#[tauri::command]
fn poll_mouse_click()       // 轮询鼠标点击标志
#[tauri::command]
fn list_background_images() // 扫描 EXE/images/ 目录下的图片文件
#[tauri::command]
fn set_bg_image_file()      // 设置当前使用的背景图片文件名
#[tauri::command]
fn import_wallpaper()       // 将图片文件写入 EXE/images/ 目录
#[tauri::command]
fn ensure_caps_lock_off()   // 强制关闭大写锁定

fn kill_hook_process()      // 终止 keyhook.exe 进程
fn cleanup_duplicate_processes() // 启动时清理同名进程和 keyhook.exe
fn setup_tray()             // 托盘图标和菜单设置（退出时卸载钩子）
fn setup_window_events()    // 窗口事件监听
fn toggle_window_visibility()  // 窗口显示/隐藏切换
fn register_global_hotkey() // 注册全局快捷键 Ctrl+Alt+L
pub fn run()                // 应用入口，组装各模块
```

**扩展原则**：新增功能应添加独立函数，在 `setup` 或 `run()` 中调用，保持结构清晰。

## 开发命令

```bash
# 仅检查 Rust 编译错误（忽略 MSVC 链接器缺失）
cargo check
# 对于 AI 只需要执行上面的指令确定是否编译错误，而不需要编译
```

## 开发注意事项

### 1. 编译环境

- **MSVC 链接器缺失是已知问题**，`cargo check` 报告的 `linker link.exe not found` 错误可忽略
- 只需关注 `error[E` 开头的 Rust 编译错误
- 当前开发环境为 Windows

### 2. Tauri v2 API 变更

本项目使用 Tauri v2，与 v1 有重大差异：

| v1 | v2 |
|---|---|
| `Window::get_window()` | `AppHandle::get_webview_window()` |
| `SystemTray` | `TrayIconBuilder` |
| `tauri::WindowEvent::CloseRequested { .. }` | 相同，但 API 调用方式不同 |
| `window.emit()` | 需要导入 `tauri::Emitter` trait |

**不要使用 Tauri v1 的 API 写法。**

### 3. 窗口控制

**关键规则**：
- 前端关闭窗口时调用 `window.hide()`，隐藏到托盘
- 前端最小化窗口时调用 `window.minimize()`
- 锁屏窗口解锁时调用 `window.hide()`，**不要调用 `window.close()`**
- 必须配置对应权限，否则会报权限错误

权限配置在 `src-tauri/capabilities/default.json`：
```json
{
  "permissions": [
    "core:default",
    "core:window:allow-hide",
    "core:window:allow-minimize",
    "opener:default"
  ]
}
```

### 4. 权限管理

- 权限配置在 `src-tauri/capabilities/` 目录下
- **每个窗口应有独立的权限文件**：
  - `default.json` — 主窗口权限
  - `lock.json` — 锁屏窗口权限
- 添加使用 Tauri API 的前端功能时，需在此文件中声明对应权限
- 常见权限格式：`core:window:allow-<action>`

### 5. 前端通信

- 使用 `@tauri-apps/api` v2 的 `invoke` 调用 Rust 命令
- 使用 `@tauri-apps/api` v2 的 `listen` 监听后端事件
- Rust 命令需用 `#[tauri::command]` 注解
- 命令需在 `invoke_handler` 中注册
- 状态管理使用 Tauri 的 `State` 机制（`AppState`）

### 6. Tauri 命令列表

当前已注册的命令：

| 命令 | 参数 | 返回值 | 说明 |
|---|---|---|---|
| `set_password` | `password: String, old_password: Option<String>` | `Result<(), String>` | 设置密码，已设置时需验证原密码，自动关闭大写锁定 |
| `verify_password` | `password: String` | `Result<bool, String>` | 验证密码，自动关闭大写锁定 |
| `has_password` | 无 | `Result<bool, String>` | 检查是否已设置密码 |
| `get_settings` | 无 | `Result<AppSettings, String>` | 获取所有配置 |
| `update_setting` | `key: String, value: f64` | `Result<(), String>` | 更新单个配置项（bool 类型传 1.0/0.0） |
| `start_lock_screen` | 无 | `Result<(), String>` | 启动锁屏窗口 + 启动 keyhook.exe + 生成时间戳 |
| `unlock_screen` | 无 | `Result<(), String>` | 隐藏锁屏窗口 + 终止 keyhook.exe + 恢复输入法 |
| `set_password_visible` | `visible: bool` | `Result<(), String>` | 通知后端密码框显示/隐藏状态 |
| `poll_mouse_click` | 无 | `bool` | 轮询鼠标点击标志（前端每 200ms 调用），返回 true 后清除 |
| `list_background_images` | 无 | `Result<Vec<String>, String>` | 扫描 EXE/images/ 目录下的图片文件 |
| `set_bg_image_file` | `filename: Option<String>` | `Result<(), String>` | 设置当前使用的背景图片文件名 |
| `import_wallpaper` | `file_name: String, bytes: Vec<u8>` | `Result<(), String>` | 将图片文件写入 EXE/images/ 目录 |
| `ensure_caps_lock_off` | 无 | `()` | 强制关闭大写锁定 |

### 7. 代码风格

- **Rust**: 遵循 Rust 官方规范，使用 `cargo fmt` 格式化
- **TypeScript**: 使用原生 DOM API，不引入额外框架
- **注释**: 仅在必要时添加注释，避免冗余注释
- **错误处理**: 使用 `Result` 和 `?` 操作符，避免 `unwrap()`（除非确定不会失败）

### 8. SVG 图标系统

项目使用 SVG `<symbol>` 定义简笔画风格图标，集中在 `index.html` 的 `<defs>` 中：
- 所有图标使用 `fill="none"` 和 `stroke="currentColor"`
- 通过 `<use href="#icon-name"/>` 引用
- 图标颜色通过 CSS `color` 属性控制
- 图标尺寸通过 CSS `width` / `height` 控制

### 9. 多窗口注意事项

当前有两个窗口：`main`（设置界面）和 `lock`（锁屏界面）。

- 在 `tauri.conf.json` 的 `app.windows` 数组中配置所有窗口
- 使用 `app.get_webview_window("窗口名")` 获取特定窗口
- **为不同窗口设置独立的权限文件**（`capabilities/*.json`）
- **锁屏窗口解锁时只隐藏不关闭**，避免重新创建导致透明效果丢失

### 10. 依赖说明

当前依赖：
- `tauri` v2 + `tray-icon` 特性
- `tauri-plugin-opener` v2
- `serde` + `derive` — 配置序列化
- `serde_json` — JSON 处理
- `sha2` — 密码 hash（SHA-256）
- `hex` — hash 结果十六进制编码
- `base64` — 图片 base64 编码（用于锁屏页注入背景图）
- `reqwest` — HTTP 客户端（获取 Bing 壁纸）
- `tokio` — 异步运行时
- `winapi` — Windows API 调用（Windows 平台），features 包含 `winuser`, `windef`, `wincon`, `handleapi`, `processthreadsapi`, `errhandlingapi`, `libloaderapi`, `debugapi`, `tlhelp32`

### 11. 锁屏界面设计

锁屏界面（`lock.html` + `lock.css` + `lock.ts`）包含以下元素：

- **时钟显示**：大号时间 + 日期（年月日 + 星期），可开关控制
- **背景图片**：支持从 `EXE/images/` 目录加载图片或 Bing 每日壁纸，可分别控制显示密码框时和隐藏密码框时的透明度
- **密码输入框**：毛玻璃效果卡片，深色半透明背景
- **呼吸灯**：底部小圆点呼吸动画，可开关
- **鼠标点击提示**：点击时显示提示文字
- **大写锁定提示**：密码输入框下方显示黄色提示文字
- **时间戳显示**：右下角显示 Unix 时间戳，仅在密码框显示时出现

**背景图片 CSS 逻辑**：
- `.lock-bg-img` 是 `<body>` 的直接子元素，`<img>` 标签
- `#lock-overlay` 是其兄弟元素，覆盖在图片上方
- 使用 `body:has(#lock-overlay:not(.dimmed)) .lock-bg-img.show` 选择器根据 overlay 状态切换图片 opacity
- 有背景图片时 `#lock-overlay` 变为透明；显示密码框时启用 `backdrop-filter: blur(16px)`（仅模糊无黑色遮罩），隐藏密码框时完全透明
- 壁纸始终显示，不再通过设置项控制显示/隐藏

### 12. 设置项与重启提示

以下设置修改后需要重启应用才能生效，前端会显示重启提示横幅：

- `breathing_light` — 呼吸灯效果
- `bg_image_enabled` — 背景图片开关
- `bg_image_opacity_overlay` — 显示密码框时图片透明度
- `bg_image_opacity_dimmed` — 隐藏密码框时图片透明度
- `clock_visible` — 显示时钟
- `bing_wallpaper_enabled` — Bing 每日壁纸

**透明度滑块逻辑**：滑块值越大图片越透明（0% = 完全不透明，100% = 完全透明）。前端保存时将滑块值取反后传给后端（`100 - val`）。

## 常见任务参考

### 添加新的 Tauri 命令

```rust
#[tauri::command]
fn my_command(param: String, state: State<AppState>) -> Result<String, String> {
    let settings = state.settings.lock().unwrap();
    Ok(format!("Received: {}", param))
}

// 在 run() 中注册
.invoke_handler(tauri::generate_handler![my_command])
```

### 添加新的 Tauri 插件

```toml
# Cargo.toml
[dependencies]
tauri-plugin-xxx = "2"
```

```rust
// lib.rs run() 中初始化
.plugin(tauri_plugin_xxx::init())
```

### 打包时附带外部文件

在 `tauri.conf.json` 中添加 `bundle.resources` 字段，打包时外部文件会放在 EXE 同级目录的对应路径下：

```json
"bundle": {
  "resources": ["resources/keyhook.exe"]
}
```

代码中通过 `std::env::current_exe()` 获取 EXE 目录来定位：

### 添加新的配置项

1. 在 `AppSettings` 结构体中添加字段
2. 在 `Default` 实现中设置默认值
3. 在 `update_setting` 中添加对应分支
4. 如果该配置需要重启才能生效，在 `main.ts` 的 `NEED_RESTART_KEYS` 数组中添加 key

### 前端调用 Rust 命令

```typescript
import { invoke } from "@tauri-apps/api/core";
const result = await invoke("my_command", { param: "value" });
```

### 前端监听后端事件

```typescript
import { listen } from "@tauri-apps/api/event";
void listen<number>("event-name", (event) => {
  const payload = event.payload;
  // 处理事件
});
```

### 使用 SVG 图标

```html
<!-- 在 index.html 的 <defs> 中定义 -->
<symbol id="icon-example" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
  <circle cx="12" cy="12" r="10"/>
</symbol>

<!-- 在页面中使用 -->
<svg class="my-icon"><use href="#icon-example"/></svg>
```

```css
/* CSS 中控制颜色和尺寸 */
.my-icon {
  width: 24px;
  height: 24px;
  color: var(--accent-color);
}
```

### 添加新窗口

1. 在 `tauri.conf.json` 的 `app.windows` 数组中添加窗口配置
2. 创建对应的 HTML 文件（如 `newpage.html`）
3. 创建对应的 TS/CSS 文件（如 `src/newpage.ts`, `src/newpage.css`）
4. 如需前端调用窗口操作，在 `capabilities/` 下新建权限文件

## 安全注意事项

- 密码使用 SHA-256 hash 存储，不存储明文
- 修改密码需要验证原密码
- 启动时自动清理同名进程和残留 keyhook.exe，防止重复实例
- 应用退出时自动卸载系统钩子，防止钩子残留
- 不要在代码中硬编码密钥或敏感信息
- CSP 配置在 `tauri.conf.json` 中，当前为 null（开发阶段）
- 生产环境应启用严格的 CSP 策略

## 版本信息

- Tauri: v2
- Rust Edition: 2021
- TypeScript: ~5.6.2
- Vite: ^6.0.3
