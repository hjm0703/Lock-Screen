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
│   │   └── lib.rs                # 核心逻辑（托盘、窗口管理、Tauri 命令）
│   ├── capabilities/
│   │   └── default.json          # 权限配置
│   ├── icons/                    # 应用图标
│   ├── Cargo.toml                # Rust 依赖
│   ├── build.rs                  # Tauri 构建脚本
│   └── tauri.conf.json           # Tauri 配置
├── src/                          # 前端源码
│   ├── main.ts                   # TypeScript 入口
│   └── styles.css                # 样式
├── index.html                    # HTML 入口（包含 SVG 图标定义）
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
    → 调用 app.exit(0)
    → 程序完全退出
```

### 窗口配置

窗口使用无框设计（`decorations: false`），自定义标题栏包含最小化和关闭按钮：
- **最小化按钮**：调用 `window.minimize()` 最小化窗口
- **关闭按钮**：调用 `window.hide()` 隐藏到系统托盘
- **托盘"显示窗口"菜单项**：切换窗口显示/隐藏

### 数据持久化

- 配置数据存储在 **EXE 同级目录的 `settings.json`**
- 使用 `serde` 和 `serde_json` 进行序列化/反序列化
- 设置变更后立即保存到文件

### 密码管理

- 密码使用 **SHA-256** 算法 hash 后存储，不存储明文
- 密码 hash 保存在 `settings.json` 的 `password_hash` 字段
- **修改密码需要验证原密码**（如果已设置密码）

### 代码组织结构

`lib.rs` 采用函数分离设计，每个功能独立封装：

```rust
struct AppSettings          // 配置数据结构
struct AppState             // 应用状态（包含 Mutex<AppSettings>）

fn get_settings_path()      // 获取 EXE 同级目录 settings.json 路径
fn load_settings()          // 加载配置
fn save_settings()          // 保存配置
fn hash_password()          // 密码 SHA-256 hash

#[tauri::command]
fn set_password()           // 设置密码（hash 存储，修改需验证原密码）
#[tauri::command]
fn verify_password()        // 验证密码
#[tauri::command]
fn has_password()           // 检查是否已设置密码
#[tauri::command]
fn get_settings()           // 获取所有配置
#[tauri::command]
fn update_setting()         // 更新单个配置项

fn setup_tray()             // 托盘图标和菜单设置
fn setup_window_events()    // 窗口事件监听
fn toggle_window_visibility()  // 窗口显示/隐藏切换
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

**不要使用 Tauri v1 的 API 写法。**

### 3. 窗口控制

**关键规则**：
- 前端关闭窗口时调用 `window.hide()`，隐藏到托盘
- 前端最小化窗口时调用 `window.minimize()`
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

- 权限配置在 `src-tauri/capabilities/default.json`
- 添加使用 Tauri API 的前端功能时，需在此文件中声明对应权限
- 常见权限格式：`core:window:allow-<action>`

### 5. 前端通信

- 使用 `@tauri-apps/api` v2 的 `invoke` 调用 Rust 命令
- Rust 命令需用 `#[tauri::command]` 注解
- 命令需在 `invoke_handler` 中注册
- 状态管理使用 Tauri 的 `State` 机制（`AppState`）

### 6. Tauri 命令列表

当前已注册的命令：

| 命令 | 参数 | 返回值 | 说明 |
|---|---|---|---|
| `set_password` | `password: String, old_password: Option<String>` | `Result<(), String>` | 设置密码，已设置时需验证原密码 |
| `verify_password` | `password: String` | `Result<bool, String>` | 验证密码 |
| `has_password` | 无 | `Result<bool, String>` | 检查是否已设置密码 |
| `get_settings` | 无 | `Result<AppSettings, String>` | 获取所有配置 |
| `update_setting` | `key: String, value: bool` | `Result<(), String>` | 更新单个配置项 |

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

当前只有一个 `main` 窗口。如需添加多窗口：

- 在 `tauri.conf.json` 中配置额外窗口
- 使用 `app.get_webview_window("窗口名")` 获取特定窗口
- 为不同窗口设置独立的权限

### 10. 依赖说明

当前依赖：
- `tauri` v2 + `tray-icon` 特性
- `tauri-plugin-opener` v2
- `serde` + `derive` — 配置序列化
- `serde_json` — JSON 处理
- `sha2` — 密码 hash（SHA-256）
- `hex` — hash 结果十六进制编码

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

### 添加新的配置项

1. 在 `AppSettings` 结构体中添加字段
2. 在 `Default` 实现中设置默认值
3. 在 `update_setting` 中添加对应分支

### 前端调用 Rust 命令

```typescript
import { invoke } from "@tauri-apps/api/core";
const result = await invoke("my_command", { param: "value" });
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

## 安全注意事项

- 密码使用 SHA-256 hash 存储，不存储明文
- 修改密码需要验证原密码
- 不要在代码中硬编码密钥或敏感信息
- CSP 配置在 `tauri.conf.json` 中，当前为 null（开发阶段）
- 生产环境应启用严格的 CSP 策略

## 版本信息

- Tauri: v2
- Rust Edition: 2021
- TypeScript: ~5.6.2
- Vite: ^6.0.3
