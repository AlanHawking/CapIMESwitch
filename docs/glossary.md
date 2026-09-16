# 术语表

CapIMESwitch 项目通用术语,用于跨模块沟通与文档一致。

| 术语 | 定义 |
|---|---|
| 语言偏好 | 用户选择的界面语言设置,存于 `config.toml` 的 `language` 字段 |
| 语言值 | `language` 字段的取值:`"auto"`(跟随系统)/ `"zh"` / `"en"` / `"ja"` / `"ko"` / `"fr"` / `"de"` / `"es"` / `"ru"` |
| `auto` 模式 | 语言偏好未显式指定时,启动时按系统 UI 语言(`GetUserDefaultUILanguage`)解析 |
| LCID | Windows 区域语言标识(16 位),低 10 位为主语言 ID:0x04=中文,0x09=英文,0x11=日文,0x12=韩文,0x0C=法文,0x07=德文,0x0A=西班牙文,0x19=俄文,未知回退中文 |
| 母语名 | 语言选项的显示写法,不随当前 UI 语言翻译,由 `Lang::display_name()` 集中提供:"简体中文"/"English"/"日本語"/"한국어"/"Français"/"Deutsch"/"Español"/"Русский" |
| i18n 表 | `src/i18n.rs` 中每个语言一个 `Strings` 结构体字面量,编译期保证字段齐全 |
| 托盘语言子菜单 | 托盘右键菜单中"语言 ▸"弹出子菜单,按 `Lang::all()` 顺序逐项列出,各语言带互斥勾选 |
| 面板语言下拉框 | 选项面板第 4 行 ComboBox(`ID_OPT_COMBO_LANGUAGE`=2013),选项按 `Lang::all()` 顺序 |
| 运行时语言 | 全局 `LANGUAGE` 状态,保存/切换后即时更新,供托盘菜单、面板、错误框、tooltip 读取
| 短按 / 长按 | CapsLock 按下后在长按阈值(默认 500ms)内松开 = 短按(切换输入法);达到阈值 = 长按(切换大小写) |
| 排除程序 | `exclude_processes` 列表,这些程序中 CapsLock 恢复原始行为 |
| 隐藏窗口 | 类名 `CapIMESwitchTrayWindow`,HWND_MESSAGE 父窗口,接收托盘回调与 TaskbarCreated |
| 单实例互斥体 | 命名互斥体 `CapIMESwitch_SingleInstance`,重复启动时弹错误框退出 |
| 自启动注册表值 | `HKCU\...\Run` 下 `CapIMESwitch` 值,指向当前 exe 路径 |
| UIPI | 用户界面特权隔离:低完整性(非提权)进程不能观察/注入高完整性(提权)进程的输入,导致管理员窗口中键盘钩子失效 |
| 完整性级别 | Windows 进程安全级别:普通进程 Medium,管理员(UAC 提权)进程 High;UIPI 只允许同/高级别向下观察 |
| 提权重启 | 以管理员权限重新启动自身(经 UAC 确认,`ShellExecuteW` runas);用于让钩子覆盖管理员窗口 |
| 降权重启 | 从提权实例以普通权限重新启动自身:经 `/RL LIMITED` 一次性计划任务(`CapIMESwitchDeElevateTemp`)中转,由任务计划程序服务以普通令牌启动 `--restart` 实例 |
| 开机启动计划任务 | 任务名 `CapIMESwitchElevated`,`schtasks /SC ONLOGON /RL HIGHEST` 创建,登录时静默提权启动 |
| 最高权限(RL HIGHEST) | 计划任务运行级别,任务以管理员权限运行且登录时不弹 UAC;只能由管理员上下文创建/删除 |
