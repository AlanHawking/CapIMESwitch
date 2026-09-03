# CapIMESwitch

CapsLock 一键两用的 Windows 系统托盘工具：短按切换输入法，长按切换大小写。

输入法切换（`Win+Space`）与 CapsLock 大小写锁定的高频冲突，通过将 CapsLock 重映射为「短按 = 切输入法、长按 = 大小写」一次解决两个痛点。

## 功能特性

- **短按（默认 < 500ms 松开）**：模拟 `Win+Space`，切换输入法
- **长按（≥ 阈值，默认 500ms）**：合成 CapsLock 按下/释放，切换大小写（LED 同步）
- **冲突取消**：长按期间按下其他键则取消本次动作，避免误触发
- **系统托盘常驻**：隐藏窗口运行，无任务栏、无窗口；右键托盘菜单提供「开机启动」勾选、「选项」面板与「退出」
- **选项面板**：开机启动勾选（与菜单共享注册表状态，保存后生效）；长按阈值可调 100-5000ms，保存即生效
- **中英文界面**：托盘菜单与选项面板均可切换语言（默认跟随系统语言），切换即时生效并持久化
- **配置持久化**：exe 同目录 `config.toml`（可人工编辑、随目录迁移），缺失或损坏时自动回退默认值
- **开机自启动**：通过 `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` 注册，卸载时自动清理
- **单实例保护**：命名互斥体防止重复运行，二次启动时提示并退出
- **资源管理器重启恢复**：监听 `TaskbarCreated` 消息自动重建托盘图标
- **自绘图标**：构建期用纯 Rust 程序化生成多尺寸（16/32/48）ICO，嵌入 exe 资源，零外部资源文件

## 工作原理

程序基于 `WH_KEYBOARD_LL` 全局低级键盘钩子拦截原始 CapsLock 事件，由 `caps.rs` 中的纯逻辑状态机判定长短按：

```
CapsLock 按下
  ├─ 按住期间按下其他键 → 取消（Cancelled）
  ├─ 阈值（默认 500ms）内松开 → 短按 → 注入 Win+Space
  └─ 保持至定时器到期（≥ 阈值）→ 长按 → 注入 CapsLock toggle（LongFired）
```

状态机与 Win32 完全解耦，可通过时间线驱动进行单元测试。

## 技术栈

| 分类 | 技术 |
|---|---|
| 语言 | Rust（edition 2021） |
| Windows API | `windows-sys` 0.59（Win32 键盘钩子、托盘、注册表、定时器） |
| 配置解析 | `serde` + `toml` 0.8（exe 同目录 `config.toml` 读写） |
| 并发原语 | `parking_lot` |
| 资源嵌入 | `winres`（构建期图标嵌入 + `include_bytes!`） |
| 图标生成 | 纯 Rust 程序化绘制（零图像处理依赖） |
| 安装器 | Inno Setup（中英文双语） |
| 目标平台 | Windows 10 / 11，x86_64 |

## 构建

需要 Rust 工具链（stable，MSVC target）。

```bash
# 发布构建（strip 优化）
cargo build --release

# 运行单元测试（状态机、注册表读写、图标解析）
cargo test
```

产物：`target\release\cap-ime-switch.exe`

## 安装

- **免安装**：直接运行 `cap-ime-switch.exe`，程序常驻托盘
- **安装器**：用 Inno Setup 编译 `installer\setup.iss`，或直接使用 `installer\dist` 下的安装包：

```bash
ISCC.exe installer\setup.iss
```

安装器支持中英文界面，默认安装到 `%LOCALAPPDATA%\Programs\CapIMESwitch`，无需管理员权限。

> 注：杀毒软件可能对全局键盘钩子程序报误报，属正常现象。

## 使用

1. 运行程序后在托盘出现图标，即可开始使用
2. **短按 CapsLock**：切换输入法（等效 `Win+Space`）
3. **长按 CapsLock（默认约半秒，可在选项面板调整）**：切换大小写
4. 右键托盘图标 → 「开机启动」随系统启动；「选项」打开设置面板（开机启动勾选、长按延迟毫秒数，右下角「保存」后生效，未保存直接关闭则丢弃）；「退出」完全退出

## 测试与验证

- `cargo test`：`caps.rs` 长短按状态机、`main.rs` 注册表读写与 ICO 解析、`config.rs` 配置读写与校验的单元测试
- `smoke.ps1`：冒烟测试，验证进程启动、隐藏窗口创建、单实例互斥与清理

## 目录结构

```
├── src/
│   ├── main.rs      # 入口：托盘、钩子、动作注入、自启动注册、选项面板
│   ├── caps.rs      # CapsLock 长短按状态机（纯逻辑，可单测）
│   ├── config.rs    # config.toml 配置读写（缺失/损坏回退默认值）
│   └── i18n.rs      # 语言枚举与中英文用户可见字符串表（纯逻辑，可单测）
├── docs/
│   ├── adr/         # 架构决策记录
│   └── glossary.md  # 术语表
├── build.rs         # 程序化生成并嵌入多尺寸 ICO
├── installer/
│   ├── setup.iss    # Inno Setup 安装脚本（中英文）
│   ├── app.ico      # 安装器图标
│   └── ChineseSimplified.isl
├── smoke.ps1        # 冒烟测试脚本
└── Cargo.toml
```

## 许可证

MIT