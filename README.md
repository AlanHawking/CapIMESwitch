# CapIMESwitch

> **CapsLock remap: short press switches IME (Win+Space), long press toggles CapsLock.**
> **CapsLock 一键两用：短按切换输入法（Win+Space），长按切换大小写。**

CapsLock, as the system input-method hotkey (`Win+Space`), conflicts with its native Caps Lock toggle. CapIMESwitch resolves both in one keystroke by remapping CapsLock to: **short press = switch input method, long press = toggle case**.

CapsLock 作为系统输入法快捷键「Win+Space」与原生大小写锁定功能高频冲突。CapIMESwitch 将 CapsLock 重映射为「**短按 = 切换输入法、长按 = 大小写**」，一次解决两个痛点。

- English / 中文
- MIT License

## Background / 项目背景

The project was created to unify the input-method switching experience across macOS and Windows. On macOS, `Ctrl+Shift+Space` toggles the input method regardless of the current language; Windows instead relies on `Win+Space`, which is a system-reserved shortcut and cannot be individually remapped. This forces users to internalize two different, platform-specific keystrokes when switching between the two systems.

创建该项目是为了统一 macOS 与 Windows 的输入法切换方式。macOS 上通过 `Ctrl+Shift+Space` 即可切换输入法，不受当前语言影响；而 Windows 依赖系统保留快捷键 `Win+Space`，无法单独重映射。这迫使习惯在两种系统间切换的用户必须记住两套不同的按键操作。

CapIMESwitch maps the Windows side to **CapsLock (short press) for input-method switching**, giving Windows a single, uniform and truly system-level switching gesture that is as effortless on Windows as `Ctrl+Shift+Space` is on macOS — so the switching habit stays consistent no matter which system you are on.

CapIMESwitch 将 Windows 侧统一为「**短按 CapsLock 切换输入法**」，使 Windows 拥有与 macOS `Ctrl+Shift+Space` 同样顺手、同样系统级的统一切换手势——无论在哪套系统上，切换输入法的习惯保持一致。

## Requirements / 系统要求

| | |
|---|---|
| OS | **Windows 10 / 11**（x86_64）|
| Architecture | x86_64 only |

> **Note / 说明**: The program uses only classic Win32 APIs (`WH_KEYBOARD_LL` global low-level keyboard hook, tray notifications `NOTIFYICON_VERSION_4`, `SendInput`, registry), with **no high-version-specific or DPI-dependent APIs**. It is technically capable of running on older systems, but is **designed and tested for Windows 10/11** only — no cross-platform conditional compilation.

> **备注**：程序仅使用经典 Win32 API（`WH_KEYBOARD_LL` 全局低级键盘钩子、托盘通知 `NOTIFYICON_VERSION_4`、`SendInput`、注册表），**无高版本专属或 DPI 相关 API**。技术层面可兼容更老系统，但**仅针对 Windows 10/11 设计与测试**，无跨平台条件编译。

## Features / 功能特性

- **Short press (release < threshold, default 500ms)**: simulates `Win+Space` to switch input method<br>**短按（默认 < 500ms 松开）**：模拟 `Win+Space`，切换输入法
- **Long press (≥ threshold, default 500ms)**: synthesizes CapsLock press/release to toggle case (LED sync)<br>**长按（≥ 阈值，默认 500ms）**：合成 CapsLock 按下/释放，切换大小写（LED 同步）
- **Conflict cancel**: pressing another key during a long-press cancels the action, avoiding misfires<br>**冲突取消**：长按期间按下其他键则取消本次动作，避免误触发
- **System-tray resident**: runs as a hidden window — no taskbar, no window; right-click tray menu offers "Start on boot" toggle, "Options" panel, and "Exit"<br>**系统托盘常驻**：隐藏窗口运行，无任务栏、无窗口；右键托盘菜单提供「开机启动」勾选、「选项」面板与「退出」
- **Options panel**: start-on-boot checkbox (shares registry state with menu, applied on save); adjustable long-press threshold 100–5000ms, applied immediately<br>**选项面板**：开机启动勾选（与菜单共享注册表状态，保存后生效）；长按阈值可调 100–5000ms，保存即生效
- **Bilingual UI**: tray menu and option panel switch between Chinese and English (follows system language by default), applied instantly and persisted<br>**中英文界面**：托盘菜单与选项面板均可切换语言（默认跟随系统语言），切换即时生效并持久化
- **Config persistence**: edits `config.toml` beside the exe (hand-editable, migrates with the folder); falls back to defaults if missing or corrupted<br>**配置持久化**：exe 同目录 `config.toml`（可人工编辑、随目录迁移），缺失或损坏时自动回退默认值
- **Auto-start**: registered at `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, auto-cleaned on uninstall<br>**开机自启动**：通过 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 注册，卸载时自动清理
- **Single-instance protection**: named mutex prevents duplicate launches; a second launch prompts and exits<br>**单实例保护**：命名互斥体防止重复运行，二次启动时提示并退出
- **Explorer-restart recovery**: listens for `TaskbarCreated` to rebuild the tray icon<br>**资源管理器重启恢复**：监听 `TaskbarCreated` 消息自动重建托盘图标
- **Self-drawn icon**: multi-size (16/32/48) ICO generated programmatically in pure Rust at build time, embedded into the exe — zero external resource files<br>**自绘图标**：构建期用纯 Rust 程序化生成多尺寸（16/32/48）ICO，嵌入 exe 资源，零外部资源文件

## How it works / 工作原理

Based on the `WH_KEYBOARD_LL` global low-level keyboard hook, the program intercepts raw CapsLock events and a pure-logic state machine in `caps.rs` decides short/long press:

程序基于 `WH_KEYBOARD_LL` 全局低级键盘钩子拦截原始 CapsLock 事件，由 `caps.rs` 中的纯逻辑状态机判定长短按：

```
CapsLock press / CapsLock 按下
  ├─ another key pressed while held → cancels / 按住期间按下其他键 → 取消（Cancelled）
  ├─ released within threshold (default 500ms) → short press → inject Win+Space / 阈值内松开 → 短按 → 注入 Win+Space
  └─ held until timer fires (≥ threshold) → long press → inject CapsLock toggle / 保持至定时器到期 → 长按 → 注入 CapsLock toggle（LongFired）
```

The state machine is fully decoupled from Win32 and unit-tested via timeline driving.

状态机与 Win32 完全解耦，可通过时间线驱动进行单元测试。

## Tech stack / 技术栈

| Category / 分类 | Tech / 技术 |
|---|---|
| Language / 语言 | Rust（edition 2021） |
| Windows API | `windows-sys` 0.59（Win32 键盘钩子、托盘、注册表、定时器） |
| Config parsing / 配置解析 | `serde` + `toml` 0.8（exe 同目录 `config.toml` 读写） |
| Concurrency / 并发原语 | `parking_lot` |
| Resource embedding / 资源嵌入 | `winres`（构建期图标嵌入 + `include_bytes!`） |
| Icon generation / 图标生成 | 纯 Rust 程序化绘制（零图像处理依赖） |
| Installer / 安装器 | Inno Setup（中英文双语） |
| Target platform / 目标平台 | Windows 10 / 11，x86_64 |

## Build / 构建

Requires the Rust toolchain (stable, MSVC target). / 需要 Rust 工具链（stable，MSVC target）。

```bash
# Release build (stripped) / 发布构建（strip 优化）
cargo build --release

# Unit tests (state machine, registry, icon parsing) / 单元测试（状态机、注册表读写、图标解析）
cargo test
```

Output / 产物：`target\release\cap-ime-switch.exe`

## Installation / 安装

- **Portable / 免安装**：run `cap-ime-switch.exe` directly；直接运行，程序常驻托盘
- **Installer / 安装器**：compile `installer\setup.iss` with Inno Setup, or use the package under `installer\dist`；用 Inno Setup 编译，或直接使用 `installer\dist` 下的安装包：

```bash
ISCC.exe installer\setup.iss
```

The installer supports bilingual (Chinese/English) UI and installs to `%LOCALAPPDATA%\Programs\CapIMESwitch` by default — no admin rights required.

安装器支持中英文界面，默认安装到 `%LOCALAPPDATA%\Programs\CapIMESwitch`，无需管理员权限。

> Note: antivirus software may flag global keyboard hooks as false positives — normal / 注：杀毒软件可能对全局键盘钩子程序报误报，属正常现象。

## Usage / 使用

1. Run the program; a tray icon appears and it starts working / 运行程序后在托盘出现图标，即可开始使用
2. **Short-press CapsLock**：switch input method (equivalent to `Win+Space`)/ **短按 CapsLock**：切换输入法（等效 `Win+Space`）
3. **Long-press CapsLock (default ~half second, adjustable in Options)**：toggle case / **长按 CapsLock（默认约半秒，可在选项面板调整）**：切换大小写
4. Right-click the tray icon → "Start on boot" for auto-start; "Options" opens the settings panel (start-on-boot checkbox, long-press delay in ms, "Save" at bottom-right applies — closing unsaved discards changes); "Exit" quits / 右键托盘图标 → 「开机启动」随系统启动；「选项」打开设置面板（开机启动勾选、长按延迟毫秒数，右下角「保存」后生效，未保存直接关闭则丢弃）；「退出」完全退出

## Testing / 测试与验证

- `cargo test`：unit tests for the `caps.rs` short/long-press state machine, `main.rs` registry read/write and ICO parsing, and `config.rs` config read/write & validation / `caps.rs` 长短按状态机、`main.rs` 注册表读写与 ICO 解析、`config.rs` 配置读写与校验的单元测试
- `smoke.ps1`：smoke test verifying process launch, hidden-window creation, single-instance mutex, and cleanup / 冒烟测试，验证进程启动、隐藏窗口创建、单实例互斥与清理

## Directory structure / 目录结构

```
├── src/
│   ├── main.rs      # Entry: tray, hook, action injection, auto-start, options panel / 入口：托盘、钩子、动作注入、自启动注册、选项面板
│   ├── caps.rs      # CapsLock short/long-press state machine (pure logic, unit-testable) / 长短按状态机（纯逻辑，可单测）
│   ├── config.rs    # config.toml read/write (missing/corrupted → defaults) / 配置读写（缺失/损坏回退默认值）
│   └── i18n.rs      # Language enum & bilingual user-facing string table (pure logic, unit-testable) / 语言枚举与中英文用户可见字符串表（纯逻辑，可单测）
├── docs/
│   ├── adr/         # Architecture decision records / 架构决策记录
│   └── glossary.md  # Glossary / 术语表
├── build.rs         # Programmatically generates and embeds multi-size ICO / 程序化生成并嵌入多尺寸 ICO
├── installer/
│   ├── setup.iss    # Inno Setup install script (bilingual) / 安装脚本（中英文）
│   ├── app.ico      # Installer icon / 安装器图标
│   └── ChineseSimplified.isl
├── smoke.ps1        # Smoke test script / 冒烟测试脚本
└── Cargo.toml
```

## License / 许可证

MIT