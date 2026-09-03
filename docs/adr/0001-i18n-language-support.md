# ADR 0001:国际化语言支持(中文/英文)

- 状态:已接受(2026-09)
- 关联决策:grill-with-docs 设计树第 1/2 轮(经用户逐项确认)

## 背景

程序用户可见文案(托盘菜单、选项面板、错误对话框、托盘 tooltip)全部硬编码为中文。
目标:增加中文/英文双语支持,语言切换入口同时存在于**托盘右键菜单**与**选项面板**。

约束:
- 单二进制、无控制台(`#![windows_subsystem = "windows"]`),选项面板控件程序化创建,无资源文件。
- 配置持久化走 exe 同目录 `config.toml`,新增字段一律 `#[serde(default)]` 兼容旧文件。
- 依赖极简(当前 3 个运行时依赖 + windows-sys)。

## 决策

1. **语言偏好存 `config.toml` 新增 `language` 字段**,值域 `"auto" | "zh" | "en"`,
   默认 `"auto"`。`#[serde(default = "default_language")]` 保证旧配置无该字段时兼容;
   `sanitized()` 将未知值归一化为 `"auto"`。
   - 不选注册表:注册表仅用于 Windows 自启动,与 portable 定位不符。
   - `"auto"` 模式避免"启动即写盘"(保持"首次落盘由保存按钮触发"约定),且系统语言变化后仍可跟随。

2. **默认语言跟随系统**:`GetUserDefaultUILanguage()` 返回 LCID,按主语言 ID(低 10 位)
   映射:0x04(中文系)→ 中文,0x09(英文系)→ 英文,其余回退中文。
   - 需要给 windows-sys 启用 `Win32_Globalization` feature(已确认 0.59 提供该 API)。

3. **实现机制:手写静态表**,新增 `src/i18n.rs`:
   - `enum Lang { Zh, En }` + 每个语言一个 `static Strings` 字面量;
   - 两个语言表均为结构体字面量,**编译期强制字段齐全**;
   - `Lang::all()` 驱动托盘子菜单与面板下拉框的选项枚举,避免硬编码索引;
   - 零依赖,延续项目"纯逻辑可单测"约定(`cargo test` 覆盖键集一致性与解析逻辑)。
   - 不选 fluent/i18n-embed/gettext:全项目用户可见字符串仅约 15 条,外部资源文件与
     项目"无资源文件"哲学冲突,框架级方案属过度设计。

4. **本地化范围**:
   - 翻译:托盘菜单(含新增语言子菜单)、托盘 tooltip、选项面板全部文案、9 处错误对话框。
   - 不翻译:日志(开发者排查用,保持中文)、安装器(已中英双语)、README。
   - 语言选项在面板下拉框与托盘子菜单中均以**母语名**显示("简体中文"/"English")。

5. **托盘菜单形态:子菜单 + 互斥勾选**("语言 ▸" 下两项带 `MF_CHECKED`)。
   菜单每次打开时重建,切换即时生效于下一次打开。

6. **选项面板形态:第 4 行"语言"标签 + ComboBox(`CBS_DROPDOWNLIST | CBS_HASSTRINGS`)**
   (控件 ID `ID_OPT_COMBO_LANGUAGE = 2013`)。面板高度 300 → 330,说明栏/版本号/保存按钮下移。

7. **生效时机:面板内选择语言后点「保存」统一生效**(写配置 → 更新全局 → 关面板,下次打开为新语言)。
   托盘菜单切换即时生效(写入配置 + 刷新 tooltip)。
   不做面板内即时重绘:重建控件会带来输入值/焦点丢失风险,且与现有"保存后生效"交互模型一致。

## 影响

- `Cargo.toml`:版本 0.3.3 → 0.4.0,新增 `Win32_Globalization` feature。
- `config.rs`:`Config` 增加 `language: String` 字段。
- `src/i18n.rs`(新增):语言枚举、双语字符串表、归一化与测试。
- `main.rs`:`LANGUAGE` 全局状态、`resolve_language`/`system_language`/`set_language`、
  `update_tray_tooltip`,托盘语言子菜单、面板第 4 行 ComboBox、全部用户可见文案接入 i18n 表。
- 版本号同步:`installer/setup.iss` 的 `MyAppVersion` 更新为 0.4.0。

## 验证

- `cargo test`:35 项全绿(含 i18n 表完整性、config language 读写、LCID 映射、parse/normalize)。
- 端到端:配置 `language = "en"` 启动 → 二次启动错误框显示英文文案;
  不写 language(auto)→ 本机中文系统显示中文文案。
- `smoke.ps1`:进程存活、隐藏窗口创建、单实例提示、清理全部通过。

## 备注

托盘语言子菜单的渲染与面板 ComboBox 的视觉外观需手工验证(AGENTS.md 既有"手工验证"约定;
自动化 UI 输入注入在本环境被 UIPI 拦截,无法驱动)。
