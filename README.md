# Lock Screen

Windows 屏幕锁定工具 — 基于 Tauri v2 构建的系统托盘常驻应用。锁定键盘输入、封禁 Win 键、强制英文输入法，提供完整的锁屏体验。

## 功能

- **一键锁屏** — 全屏覆盖 + 键盘拦截 + 鼠标限制，防止他人使用电脑
- **密码保护** — SHA-256 加密存储密码，支持设置/修改/验证
- **Win 键封禁** — 锁屏时自动启动外部钩子程序，彻底拦截 Win 键
- **输入法强制** — 锁屏自动切换英文输入法，解锁恢复原输入法
- **鼠标限制** — 锁屏时鼠标可移动但点击无效，点击时显示提示
- **无框无痕** — 无框窗口、透明背景、全屏覆盖、不显示在任务栏
- **后台常驻** — 窗口关闭隐藏到系统托盘，不退出程序
- **系统托盘** — 托盘菜单提供窗口切换、退出等功能

## 快速开始

### 前置要求

- Rust（edition 2021）
- Node.js
- Visual Studio（MSVC 工具链，仅构建时需要）

### 开发

```bash
# 安装前端依赖
npm install

# 运行开发模式（自动启动 Tauri 窗口）
npm run tauri dev

# 仅检查 Rust 编译错误
cd src-tauri && cargo check
```

### 构建发布

将你的 Win 键拦截 EXE 放到 `src-tauri/resources/keyhook.exe`，然后执行：

```bash
npm run tauri build
```

构建产物在 `src-tauri/target/release/` 目录下，包含：
- `lock-screen.exe` — 主程序
- `resources/keyhook.exe` — Win 键拦截程序（通过 bundle.resources 自动打包）

## 技术栈

| 层 | 技术 |
|---|---|
| 后端 | Rust + Tauri v2 |
| 前端 | TypeScript + Vite（无框架，原生 DOM） |
| 键盘钩子 | Windows WH_KEYBOARD_LL / WH_MOUSE_LL（全局钩子） |
| 密码 | SHA-256 哈希存储 |
| 打包 | Cargo + npm + Tauri bundle |

## 项目结构

```
Lock Screen/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs        # 入口
│   │   ├── lib.rs         # 核心逻辑（托盘、窗口管理、Tauri 命令）
│   │   └── hooks.rs       # 键盘钩子 + 鼠标钩子（WH_*_LL）
│   ├── resources/         # 打包附带的外部资源
│   │   └── keyhook.exe    # Win 键拦截程序
│   ├── capabilities/      # Tauri 权限配置
│   ├── Cargo.toml
│   └── tauri.conf.json
├── src/
│   ├── main.ts            # 设置界面
│   ├── lock.ts            # 锁屏界面
│   ├── styles.css
│   └── lock.css
├── index.html
├── lock.html
├── package.json
└── vite.config.ts
```

## 详细开发指南

参见 [AI开发指南.md](AI开发指南.md)。

---

> 本项目由 **AI 参与生成** — 使用 Trae IDE 中的 DeepSeek 模型辅助开发