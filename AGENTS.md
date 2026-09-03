# AGENTS.md — CapIMESwitch 项目开发指南

面向 AI 代理与本项目开发者的项目说明。开始开发前请阅读本文,并遵循 `<repo-rules>` 中已加载的全局规范。

## 项目概述

Windows 系统托盘常驻工具:将 CapsLock 重映射为「短按 = 切换输入法(Win+Space)」「长按(默认 ≥500ms,可在选项面板调整)= 切换大小写」。基于 `WH_KEYBOARD_LL` 全局低级键盘钩子,隐藏窗口 + 托盘图标运行,无 GUI 窗口。

- 入口: `src/main.rs`(`#![windows_subsystem = "windows"]`,无控制台)
- 状态机: `src/caps.rs`(纯逻辑,与 Win32 解耦,时间线驱动可单测)
- 配置: `src/config.rs`(exe 同目录 `config.toml`,缺失/损坏回退默认值)
- 选项面板: 内嵌于 `src/main.rs`,控件程序化创建,无资源文件

## 技术栈

| 分类 | 技术 | 说明 |
|---|---|---|
| 语言 | Rust edition 2021 | MSVC target,仅支持 Windows |
|Windows API|`windows-sys` 0.59|键盘钩子、托盘、注册表、SetTimer、隐藏窗口|
|配置解析|`serde` 1 + `toml` 0.8|exe 同目录 `config.toml` 读写|
| 并发原语 | `parking_lot` 0.12 | 全局静态 `Mutex`(单线程模型,锁仅作防御) |
| 构建期图标 | `build.rs` + `winres` | 纯 Rust 程序化绘制 ICO(16/32/48),嵌入 exe 资源 |
| 安装器 | Inno Setup | 中英文双语,`installer/setup.iss` |
| 脚本 | PowerShell | `smoke.ps1` 冒烟验证 |

## 开发环境

- **OS**: Windows 10/11(x86_64),仅 Windows 平台可用,无跨平台条件编译
- **Rust**: stable + MSVC toolchain(默认 `x86_64-pc-windows-msvc`)
- **构建工具**: cargo(无需安装 Visual Studio 完整版,需 MSVC Build Tools / Windows SDK,因使用 windows-sys 无需额外链接库)
- **打包工具**(可选): Inno Setup ≥ 6(`ISCC.exe` 需在 PATH 或指定完整路径)
- 当前 `JAVA_HOME` 指向 JDK 8,与本项目无关,勿触发 Maven/Java 构建约定

## 常用命令

```bash
# 构建(必需)
cargo build --release

# 单元测试(状态机 / 注册表读写 / ICO 解析)
cargo test

# 冒烟测试(需先构建 release)
powershell -ExecutionPolicy Bypass -File smoke.ps1
```

## 打包与资源文件

### 构建产物

| 产物 | 生成方式 | 位置 |
|---|---|---|
| 程序 exe | `cargo build --release` | `target/release/cap-ime-switch.exe` |
| 托盘/程序 ICO | `build.rs` 程序化生成,`include_bytes!` 内嵌 | `OUT_DIR/icon.ico`(构建期,不入库) |
| 安装包 | `ISCC.exe installer/setup.iss` | `installer/dist/CapIMESwitch-Setup-<version>.exe` |

### 源文件与 .gitignore 约定

| 文件 | 作用 | 入库状态 |
|---|---|---|
| `build.rs` | 程序化生成多尺寸 ICO 并嵌入 exe 资源(失败仅告警,不致命) | ✅ 入库 |
| `installer/setup.iss` | Inno Setup 安装脚本(`SetupIconFile=app.ico`,打包 `target/release` 下的 exe) | ✅ 入库 |
| `installer/ChineseSimplified.isl` | 简体中文语言包,Setup 编译时合并 | ✅ 入库 |
|`installer/app.ico`|安装器界面图标,由 build.rs 生成后复制入库;图标设计变更时需重新从 `OUT_DIR/icon.ico` 提取并更新|✅ 入库|
| `installer/dist/` | 安装包输出目录 | ❌ gitignore |
| `/target` | cargo 构建产物 | ❌ gitignore |

### 打包流程

1. `cargo build --release` 生成 exe
2. 确认 `installer/app.ico` 存在(缺失会导致 ISCC 编译失败)
3. `ISCC.exe installer\setup.iss` → 输出 `installer/dist/CapIMESwitch-Setup-<version>.exe`

## 关键实现约定

- **无窗口应用**: `#![windows_subsystem = "windows"]`;隐藏窗口类名 `CapIMESwitchTrayWindow`,接收托盘回调与 `TaskbarCreated`(资源管理器重启后重建托盘图标)
- **单实例**: 命名互斥体 `CapIMESwitch_SingleInstance`,重复启动时弹错误框退出;setup.iss 的 `AppMutex` 与之一致,安装器可感知运行中状态
- **状态机**: 短按 <阈值(默认 500ms,选项面板可调 100-5000ms)→ `SwitchIme`;长按 ≥阈值(SetTimer 到期)→ `ToggleCapsLock`;按住期间按其他键 → `Cancelled`,注入事件不进入状态机(以 `INJECTED` 标志过滤)
- **自启动**: `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 值名 `CapIMESwitch`,托盘菜单勾选切换;卸载时 setup.iss 的 `[Code]` 段删除该值
- **选项面板**: 托盘菜单「选项」打开设置窗口(类名 `CapIMESwitchOptionsWindow`)。开机启动勾选框与右键菜单共享 Run 键状态(保存时才写入,两个入口显示一致);长按阈值 `long_press_ms` 存 exe 同目录 `config.toml`(范围 100-5000,默认 500,缺失/损坏回退默认)。点「保存」写入并即时生效,未保存关闭则丢弃;面板打开期间屏蔽托盘交互(`OPTIONS_OPEN` 标志)
- **按键注入**: `INPUT` 结构合成 Win+Space(CapsLock 保持状态不发生)+ CapsLock toggle(硬件状态同步)

## 修改约束

- 修改键盘钩子逻辑 / 状态机时,必须同步更新 `caps.rs` 单元测试并跑通 `cargo test`
- 修改窗口类名 / 互斥体名 / 注册表值名时,需同步检查 `smoke.ps1`、`setup.iss`(AppMutex、卸载清理)中的硬编码引用;修改 `config.toml` 字段时注意 `setup.iss` 的 `[UninstallDelete]`(卸载删 `config.toml`)与 `config.rs` 的 `#[serde(default)]` 兼容
- 修改托盘菜单命令 ID 或新增菜单项时,注意 `wnd_proc` 与 `show_tray_menu` 的对应关系(WM_COMMAND 路由)
- 新增 windows-sys feature 时先在 `Cargo.toml` 的 `[dependencies]` 中登记,勿使用 `--features` 命令行临时开启

## 验证

- 逻辑变更:`cargo test`
- 完整验证:release 构建 → `smoke.ps1`(进程存活、隐藏窗口创建、二次启动弹出提示、清理)
- 手工验证:运行后短按/长按 CapsLock 观察输入法切换与 CapsLock LED