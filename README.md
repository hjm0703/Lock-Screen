# Lock Screen

Windows 屏幕锁定工具 — 基于 Tauri v2 构建的系统托盘常驻应用。锁定键盘输入、封禁 Win 键、强制英文输入法，提供完整的锁屏体验。

## 功能

- **一键锁屏** — 全屏覆盖 + 键盘拦截 + 鼠标限制，防止他人使用电脑
- **密码保护** — SHA-256 加密存储密码，支持设置/修改/验证
- **全局快捷键** — 按 `Ctrl + Alt + L` 快速触发锁屏，无需打开设置窗口
- **Win 键封禁** — 锁屏时自动启动外部钩子程序，彻底拦截 Win 键
- **输入法强制** — 锁屏自动切换英文输入法，解锁恢复原输入法
- **背景模式** — 三选一：无背景图 / Bing 每日壁纸 / 自定义图片，单选切换
- **呼吸灯效果** — 锁屏时底部显示呼吸灯动画，可开关
- **解锁欢迎画面** — 密码正确后显示欢迎动画，可开关控制
- **进程防重复** — 启动时自动清理同名进程和残留 keyhook.exe

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

## 使用说明

### 锁屏

- **方式一**：打开设置窗口，在「开始锁屏」页面点击「立即锁屏」（需先设置密码）
- **方式二**：按全局快捷键 `Ctrl + Alt + L` 快速锁屏（需先设置密码）

### 解锁

- 按 `Esc` 键显示密码输入框
- 在密码框中输入密码，按 Enter 或点击「解锁」按钮
- 再次按 `Esc` 可隐藏密码框（隐藏时壁纸更清晰）

### 设置密码

1. 打开设置窗口，进入「密码设置」
2. 输入新密码和确认密码
3. 点击「保存密码」
4. 密码输入框会自动关闭大写锁定，若大写锁定开启会有黄色提示

### 自定义壁纸

1. 打开设置窗口，进入「通用设置」
2. 在「锁屏外观」卡片中选择「背景模式」为 Bing 每日壁纸或自定义图片
3. 选择自定义图片后，点击「导入图片」按钮选择本地图片（支持 png、jpg、bmp、gif、webp）
4. 图片会自动复制到 `EXE/images/` 目录
5. 在「选择图片」下拉框中选择要使用的壁纸
6. 可分别调整「显示密码框时」和「隐藏密码框时」的透明度
7. **修改后需要重启应用才能生效**

### 设置项说明

| 设置项 | 说明 |
|---|---|
| 全局锁屏快捷键 | `Ctrl + Alt + L`，系统级快捷键 |
| 启动时自动隐藏窗口 | 程序启动后自动隐藏到托盘 |
| 呼吸灯效果 | 锁屏时底部显示呼吸灯动画 |
| 显示时钟 | 锁屏时显示当前时间和日期 |
| 背景模式 | 三选一：无背景图 / Bing 每日壁纸 / 自定义图片 |
| 隐藏密码框时显示 | 按 Esc 隐藏密码框后仍显示壁纸 |
| 显示密码框时显示 | 密码框可见时显示壁纸 |
| 图片透明度 | 滑块值越大壁纸越透明（0% = 完全不透明，100% = 完全透明） |
| 解锁欢迎画面 | 密码正确后显示欢迎动画再进入桌面 |

## 技术栈

| 层 | 技术 |
|---|---|
| 后端 | Rust + Tauri v2 |
| 前端 | TypeScript + Vite（无框架，原生 DOM） |
| 键盘钩子 | Windows WH_KEYBOARD_LL / WH_MOUSE_LL（全局钩子） |
| 全局快捷键 | Windows RegisterHotKey API |
| 进程管理 | Windows Toolhelp32 API |
| 密码 | SHA-256 哈希存储 |
| 打包 | Cargo + npm + Tauri bundle |

## 项目结构

```
Lock Screen/
├── src-tauri/
│   ├── src/
│   │   ├── main.rs        # 入口
│   │   ├── lib.rs         # 核心逻辑（托盘、窗口管理、Tauri 命令、全局快捷键、进程清理）
│   │   └── hooks.rs       # 键盘钩子 + 鼠标钩子 + 大写锁定控制 + 钩子卸载
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

## 安全特性

- **密码加密**：使用 SHA-256 哈希存储，不保存明文
- **进程防重复**：启动时自动检测并终止同名进程和残留 keyhook.exe
- **钩子自动卸载**：应用退出时自动卸载 WH_KEYBOARD_LL / WH_MOUSE_LL 钩子
- **大写锁定控制**：密码输入时自动关闭大写锁定，防止误输入

---

> 本项目由 **AI 参与生成** — 使用 Trae IDE 中的 DeepSeek 模型辅助开发
