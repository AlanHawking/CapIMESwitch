# ADR 0002:国际化语言扩展为 8 国语言

- 状态:已接受(2026-09)
- 关联决策:ADR 0001(中英双语基础)落地后,将界面语言扩展到 8 种

## 背景

ADR 0001 落地后,界面仅在中文/英文间切换。为覆盖更多地区用户,将界面语言扩展到 8 种:
简体中文、English、日本語(ja)、한국어(ko)、Français(fr)、Deutsch(de)、Español(es)、Русский(ru)。

约束沿用 ADR 0001:单二进制、无资源文件、依赖极简、纯逻辑可单测。

## 决策

1. **语言集合扩展**:`Lang` 枚举新增 `Ja`/`Ko`/`Fr`/`De`/`Es`/`Ru`,合计 8 项;
   `parse`/`code`/`display_name`/`all()`/`strings()` 同步扩展。
   `Lang::all()` 顺序即托盘子菜单与面板下拉框的选项顺序。

2. **移除 `Strings` 中的冗余语言名字段**:原 `lang_zh`/`lang_en` 字段在每个语言表里
   重复一份相同的母语名,与 `Lang::display_name()` 完全等价,属纯冗余。
   语言名统一由 `Lang::display_name()` 集中提供,每个语言的 `Strings` 只保留真正
   需要翻译的文案(26 项),不再随语言数增长复制语言名。

3. **`config.toml` 语言值域扩展**:`language` 字段合法值从 `auto|zh|en` 扩为
   `auto|zh|en|ja|ko|fr|de|es|ru`,`normalize_language()` 相应扩展。
   - 旧配置兼容:此前被当作未知值回退 `auto` 的字符串(如 `fr`)如今解析为新语言,
     不会触发配置损坏或字段冲突。
   - 面板语言下拉框与托盘语言子菜单均以**母语名**显示(`Lang::display_name()`)。

4. **托盘语言子菜单与路由改动态**:不再为单个语言硬编码命令 ID,
   子菜单项 ID = `ID_MENU_LANG_FIRST(=1004) + Lang::all() 索引`;
   `WM_COMMAND` 按 `[ID_MENU_LANG_FIRST, ID_MENU_LANG_FIRST + Lang::all().len())` 区间
   匹配后经索引取语言,新增语言无需改路由。

5. **LCID 跟随系统扩展**:`lang_from_lcid` 在主语言 ID(低 10 位)映射中新增
   0x11(日)/0x12(韩)/0x0C(法)/0x07(德)/0x0A(西)/0x19(俄),未知语言仍回退中文。

6. **翻译人力维护**:6 张新语言表由人工翻译并内联于 `src/i18n.rs`,
   编译期强制每张表字段齐全;沿用"纯逻辑可单测"约定。

## 影响

- `src/i18n.rs`:`Lang` 枚举、`parse`/`code`/`display_name`/`strings` 扩展;
  `Strings` 移除 `lang_zh`/`lang_en`;新增 JA/KO/FR/DE/ES/RU 六张表;测试扩展。
- `src/main.rs`:语言子菜单遍历生成、`WM_COMMAND` 动态路由、`lang_from_lcid` 扩展,
  相关 lcid 测试更新/新增。
- `src/config.rs`:测试中原误将 `fr` 当未知值的断言改用未支持语言 `pt`。
- `README.md` / `docs/glossary.md`:语言相关描述更新为 8 语言。

## 验证

- `cargo test`:48 项全绿(含 8 语言齐全性、code/parse roundtrip、LCID 映射
  日韩俄/法德西变体、全部语言表字段非空、跨语言文案差异)。
- `cargo build --release` 成功,无警告。

## 备注

- 新语言翻译为一次性人工翻译,后续维护语言以同方式叠加(`Lang::all()` 顺序与
  新语言表各加一项),路由与菜单无需改动。
- 安装器(Inno Setup)向导仍为中英双语,不在本次应用内国际化范围内。