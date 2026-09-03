//! 国际化:语言枚举与双语用户可见字符串表。
//!
//! 设计要点:
//! - 纯逻辑,不依赖 Win32,可单元测试;系统语言检测在 main.rs(调 Win32 API)。
//! - 每个语言一个 `static Strings` 字面量,编译期强制两语言字段齐全。
//! - 语言偏好存 config.toml:`auto`(跟随系统)/`zh`/`en`,由 config.rs 持久化。
//! - 日志为开发者排查用,不参与本地化。

/// 支持的语言
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// 简体中文
    Zh,
    /// 英文
    En,
}

impl Lang {
    /// 全部语言(托盘菜单/选项面板下拉框按此顺序列出)
    pub const fn all() -> [Lang; 2] {
        [Lang::Zh, Lang::En]
    }

    /// 解析配置中的语言值:仅 `zh`/`en` 解析成功;
    /// `auto` 与未知值返回 None,由调用方按系统语言解析。
    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "zh" => Some(Lang::Zh),
            "en" => Some(Lang::En),
            _ => None,
        }
    }

    /// 语言在 config.toml 中的存储值
    pub fn code(self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
        }
    }

    /// 语言母语显示名(不随当前 UI 语言翻译,两种语言下均显示自身写法)
    pub fn display_name(self) -> &'static str {
        match self {
            Lang::Zh => "简体中文",
            Lang::En => "English",
        }
    }

    /// 当前语言的全部用户可见字符串
    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::Zh => &ZH,
            Lang::En => &EN,
        }
    }
}

/// 归一化 config.toml 中的语言值:仅保留 auto/zh/en,其余回退 auto
pub fn normalize_language(s: &str) -> String {
    match s {
        "auto" | "zh" | "en" => s.to_string(),
        _ => "auto".to_string(),
    }
}

/// 某个语言下的全部用户可见字符串(含格式化模板)
pub struct Strings {
    // 托盘菜单
    pub menu_autostart: &'static str,
    pub menu_options: &'static str,
    pub menu_language: &'static str,
    pub menu_exit: &'static str,
    // 语言母语名(两语言相同,列出而非翻译)
    pub lang_zh: &'static str,
    pub lang_en: &'static str,
    // 托盘 tooltip
    pub tray_tooltip: &'static str,
    // 选项面板
    pub panel_title: &'static str,
    pub label_autostart: &'static str,
    pub label_delay: &'static str,
    pub label_exclude: &'static str,
    pub label_language: &'static str,
    pub help_autostart: &'static str,
    pub help_delay: &'static str,
    pub help_exclude: &'static str,
    /// 版本号模板(传入版本号)
    pub version_template: &'static str,
    pub save: &'static str,
    // 错误对话框
    pub err_instance_lock: &'static str,
    pub err_already_running: &'static str,
    /// 注册窗口类失败(传入 GetLastError)
    pub err_register_class: &'static str,
    /// 注册选项窗口类失败(传入 GetLastError)
    pub err_register_options_class: &'static str,
    pub err_create_tray_window: &'static str,
    pub err_add_tray_icon: &'static str,
    /// 安装键盘钩子失败(传入 GetLastError)
    pub err_install_hook: &'static str,
    /// 创建选项窗口失败(传入 GetLastError)
    pub err_create_options_window: &'static str,
    pub err_toggle_autostart: &'static str,
    /// 延迟输入范围错误(传入最小/最大值)
    pub err_delay_range: &'static str,
    pub err_config_path: &'static str,
    pub err_save_failed: &'static str,
    pub err_save_autostart: &'static str,
}

static ZH: Strings = Strings {
    menu_autostart: "开机启动",
    menu_options: "选项",
    menu_language: "语言",
    menu_exit: "退出",
    lang_zh: "简体中文",
    lang_en: "English",
    tray_tooltip: "CapIMESwitch - CapsLock 智能切换",
    panel_title: "CapIMESwitch 选项",
    label_autostart: "开机启动",
    label_delay: "长按延迟(毫秒)",
    label_exclude: "排除程序",
    label_language: "语言",
    help_autostart: "勾选后开机自动运行本程序,状态与托盘菜单一致,保存时生效",
    help_delay: "短按 CapsLock(未达该毫秒数松开)切换输入法;长按(达到该毫秒数)切换大小写",
    help_exclude: "这些程序中 CapsLock 恢复原始行为,不切换输入法;多个程序用逗号或换行分隔",
    version_template: "版本 {}",
    save: "保存",
    err_instance_lock: "创建实例锁失败",
    err_already_running: "CapIMESwitch 已在运行中,请从系统托盘操作。",
    err_register_class: "注册窗口类失败, GetLastError={}",
    err_register_options_class: "注册选项窗口类失败, GetLastError={}",
    err_create_tray_window: "创建托盘窗口失败",
    err_add_tray_icon: "添加托盘图标失败",
    err_install_hook: "安装键盘钩子失败, GetLastError={}",
    err_create_options_window: "创建选项窗口失败, GetLastError={}",
    err_toggle_autostart: "修改开机自启动设置失败",
    err_delay_range: "请输入 {} - {} 之间的毫秒数",
    err_config_path: "无法确定配置文件位置",
    err_save_failed: "保存设置失败",
    err_save_autostart: "保存开机自启动设置失败",
};

static EN: Strings = Strings {
    menu_autostart: "Autostart at login",
    menu_options: "Options",
    menu_language: "Language",
    menu_exit: "Exit",
    lang_zh: "简体中文",
    lang_en: "English",
    tray_tooltip: "CapIMESwitch - Smart CapsLock switching",
    panel_title: "CapIMESwitch Options",
    label_autostart: "Autostart at login",
    label_delay: "Long press delay (ms)",
    label_exclude: "Excluded programs",
    label_language: "Language",
    help_autostart: "Launch automatically at login; state matches the tray menu and takes effect on save",
    help_delay: "Short press CapsLock (release before this many ms) switches IME; long press (reach this many ms) toggles CapsLock",
    help_exclude: "CapsLock keeps its original behavior in these programs and does not switch IME; separate programs with commas or newlines",
    version_template: "Version {}",
    save: "Save",
    err_instance_lock: "Failed to create instance lock",
    err_already_running: "CapIMESwitch is already running. Please use the system tray.",
    err_register_class: "Failed to register window class, GetLastError={}",
    err_register_options_class: "Failed to register options window class, GetLastError={}",
    err_create_tray_window: "Failed to create tray window",
    err_add_tray_icon: "Failed to add tray icon",
    err_install_hook: "Failed to install keyboard hook, GetLastError={}",
    err_create_options_window: "Failed to create options window, GetLastError={}",
    err_toggle_autostart: "Failed to update autostart setting",
    err_delay_range: "Please enter a value between {} and {} ms",
    err_config_path: "Cannot determine config file location",
    err_save_failed: "Failed to save settings",
    err_save_autostart: "Failed to save autostart setting",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_recognizes_concrete_langs() {
        assert_eq!(Lang::parse("zh"), Some(Lang::Zh));
        assert_eq!(Lang::parse("en"), Some(Lang::En));
    }

    #[test]
    fn parse_rejects_auto_and_unknown() {
        assert_eq!(Lang::parse("auto"), None);
        assert_eq!(Lang::parse("zh-CN"), None);
        assert_eq!(Lang::parse("fr"), None);
        assert_eq!(Lang::parse(""), None);
    }

    #[test]
    fn code_and_parse_roundtrip() {
        for lang in Lang::all() {
            assert_eq!(Lang::parse(lang.code()), Some(lang));
        }
    }

    #[test]
    fn normalize_keeps_valid_and_falls_back() {
        assert_eq!(normalize_language("auto"), "auto");
        assert_eq!(normalize_language("zh"), "zh");
        assert_eq!(normalize_language("en"), "en");
        assert_eq!(normalize_language("fr"), "auto");
        assert_eq!(normalize_language(""), "auto");
    }

    #[test]
    fn language_names_are_native_forms() {
        assert_eq!(Lang::Zh.display_name(), "简体中文");
        assert_eq!(Lang::En.display_name(), "English");
        for lang in Lang::all() {
            assert_eq!(lang.strings().lang_zh, "简体中文");
            assert_eq!(lang.strings().lang_en, "English");
        }
    }

    #[test]
    fn both_languages_have_all_nonempty_strings() {
        for lang in Lang::all() {
            let s = lang.strings();
            for field in [
                s.menu_autostart,
                s.menu_options,
                s.menu_language,
                s.menu_exit,
                s.tray_tooltip,
                s.panel_title,
                s.label_autostart,
                s.label_delay,
                s.label_exclude,
                s.label_language,
                s.help_autostart,
                s.help_delay,
                s.help_exclude,
                s.version_template,
                s.save,
                s.err_instance_lock,
                s.err_already_running,
                s.err_register_class,
                s.err_register_options_class,
                s.err_create_tray_window,
                s.err_add_tray_icon,
                s.err_install_hook,
                s.err_create_options_window,
                s.err_toggle_autostart,
                s.err_delay_range,
                s.err_config_path,
                s.err_save_failed,
                s.err_save_autostart,
            ] {
                assert!(!field.is_empty(), "{lang:?} 存在空字符串");
            }
        }
    }

    #[test]
    fn translated_ui_strings_differ_between_languages() {
        // 核心 UI 文案在两种语言下必须不同,证明两张表均已接线而非互为别名
        let zh = Lang::Zh.strings();
        let en = Lang::En.strings();
        assert_ne!(zh.menu_autostart, en.menu_autostart);
        assert_ne!(zh.menu_exit, en.menu_exit);
        assert_ne!(zh.panel_title, en.panel_title);
        assert_ne!(zh.save, en.save);
        assert_ne!(zh.label_delay, en.label_delay);
        assert_ne!(zh.tray_tooltip, en.tray_tooltip);
    }
}
