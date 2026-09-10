//! 国际化:语言枚举与各语言用户可见字符串表。
//!
//! 设计要点:
//! - 纯逻辑,不依赖 Win32,可单元测试;系统语言检测在 main.rs(调 Win32 API)。
//! - 每个语言一个 `static Strings` 字面量,编译期保证所有语言字段齐全。
//! - 语言偏好存 config.toml:`auto`(跟随系统)/`zh`/`en`/`ja`/`ko`/`fr`/`de`/`es`/`ru`,
//!   由 config.rs 持久化。
//! - 语言母语名由 [`Lang::display_name`] 统一提供(不随当前 UI 语言翻译);
//!   每个语言的 `Strings` 只含真正需要翻译的文案,不含语言名,避免重复。
//! - 日志为开发者排查用,不参与本地化。

/// 支持的语言
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lang {
    /// 简体中文
    Zh,
    /// 英文
    En,
    /// 日语
    Ja,
    /// 韩语
    Ko,
    /// 法语
    Fr,
    /// 德语
    De,
    /// 西班牙语
    Es,
    /// 俄语
    Ru,
}

impl Lang {
    /// 全部语言(托盘菜单/选项面板下拉框按此顺序列出)
    pub const fn all() -> [Lang; 8] {
        [Lang::Zh, Lang::En, Lang::Ja, Lang::Ko, Lang::Fr, Lang::De, Lang::Es, Lang::Ru]
    }

    /// 解析配置中的语言值:`zh`/`en`/`ja`/`ko`/`fr`/`de`/`es`/`ru` 解析成功;
    /// `auto` 与未知值返回 None,由调用方按系统语言解析。
    pub fn parse(s: &str) -> Option<Lang> {
        match s {
            "zh" => Some(Lang::Zh),
            "en" => Some(Lang::En),
            "ja" => Some(Lang::Ja),
            "ko" => Some(Lang::Ko),
            "fr" => Some(Lang::Fr),
            "de" => Some(Lang::De),
            "es" => Some(Lang::Es),
            "ru" => Some(Lang::Ru),
            _ => None,
        }
    }

    /// 语言在 config.toml 中的存储值
    pub fn code(self) -> &'static str {
        match self {
            Lang::Zh => "zh",
            Lang::En => "en",
            Lang::Ja => "ja",
            Lang::Ko => "ko",
            Lang::Fr => "fr",
            Lang::De => "de",
            Lang::Es => "es",
            Lang::Ru => "ru",
        }
    }

    /// 语言母语显示名(不随当前 UI 语言翻译,各语言下均显示自身写法)
    pub fn display_name(self) -> &'static str {
        match self {
            Lang::Zh => "简体中文",
            Lang::En => "English",
            Lang::Ja => "日本語",
            Lang::Ko => "한국어",
            Lang::Fr => "Français",
            Lang::De => "Deutsch",
            Lang::Es => "Español",
            Lang::Ru => "Русский",
        }
    }

    /// 当前语言的全部用户可见字符串
    pub fn strings(self) -> &'static Strings {
        match self {
            Lang::Zh => &ZH,
            Lang::En => &EN,
            Lang::Ja => &JA,
            Lang::Ko => &KO,
            Lang::Fr => &FR,
            Lang::De => &DE,
            Lang::Es => &ES,
            Lang::Ru => &RU,
        }
    }
}

/// 归一化 config.toml 中的语言值:仅保留 auto 与支持的语言,其余回退 auto
pub fn normalize_language(s: &str) -> String {
    if s == "auto" || Lang::parse(s).is_some() {
        s.to_string()
    } else {
        "auto".to_string()
    }
}

/// 某个语言下的全部用户可见字符串(含格式化模板)
pub struct Strings {
    // 托盘菜单
    pub menu_autostart: &'static str,
    pub menu_options: &'static str,
    pub menu_language: &'static str,
    pub menu_exit: &'static str,
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

static JA: Strings = Strings {
    menu_autostart: "ログイン時に自動起動",
    menu_options: "オプション",
    menu_language: "言語",
    menu_exit: "終了",
    tray_tooltip: "CapIMESwitch - CapsLock スマート切り替え",
    panel_title: "CapIMESwitch オプション",
    label_autostart: "ログイン時に自動起動",
    label_delay: "長押し遅延(ミリ秒)",
    label_exclude: "除外プログラム",
    label_language: "言語",
    help_autostart: "ログイン時に自動起動します。状態はトレイ メニューと一致し、保存時に有効になります",
    help_delay: "短押しの CapsLock(このミリ秒未満で離す)で入力方式を切り替え、長押し(このミリ秒以上)で大文字/小文字を切り替えます",
    help_exclude: "これらのプログラムでは CapsLock が元の動作に戻り、入力方式を切り替えません。複数のプログラムはカンマまたは改行で区切ります",
    version_template: "バージョン {}",
    save: "保存",
    err_instance_lock: "インスタンス ロックの作成に失敗しました",
    err_already_running: "CapIMESwitch は既に実行中です。システム トレイから操作してください。",
    err_register_class: "ウィンドウ クラスの登録に失敗しました, GetLastError={}",
    err_register_options_class: "オプション ウィンドウ クラスの登録に失敗しました, GetLastError={}",
    err_create_tray_window: "トレイ ウィンドウの作成に失敗しました",
    err_add_tray_icon: "トレイ アイコンの追加に失敗しました",
    err_install_hook: "キーボード フックのインストールに失敗しました, GetLastError={}",
    err_create_options_window: "オプション ウィンドウの作成に失敗しました, GetLastError={}",
    err_toggle_autostart: "自動起動設定の変更に失敗しました",
    err_delay_range: "{} から {} の間のミリ秒数を入力してください",
    err_config_path: "設定ファイルの場所を特定できません",
    err_save_failed: "設定の保存に失敗しました",
    err_save_autostart: "自動起動設定の保存に失敗しました",
};

static KO: Strings = Strings {
    menu_autostart: "로그인 시 자동 시작",
    menu_options: "옵션",
    menu_language: "언어",
    menu_exit: "종료",
    tray_tooltip: "CapIMESwitch - CapsLock 스마트 전환",
    panel_title: "CapIMESwitch 옵션",
    label_autostart: "로그인 시 자동 시작",
    label_delay: "길게 누름 지연(밀리초)",
    label_exclude: "제외 프로그램",
    label_language: "언어",
    help_autostart: "로그인 시 자동으로 실행됩니다. 상태는 트레이 메뉴와 일치하며 저장 시 적용됩니다",
    help_delay: "짧게 누른 CapsLock(이 밀리초 미만에 놓기)은 입력기를 전환하고, 길게 누르면(이 밀리초 이상) 대문자/소문자를 전환합니다",
    help_exclude: "이 프로그램들에서는 CapsLock이 원래 동작으로 돌아가 입력기를 전환하지 않습니다. 여러 프로그램은 쉼표 또는 줄바꿈으로 구분합니다",
    version_template: "버전 {}",
    save: "저장",
    err_instance_lock: "인스턴스 잠금 생성 실패",
    err_already_running: "CapIMESwitch가 이미 실행 중입니다. 시스템 트레이에서 조작해 주세요.",
    err_register_class: "창 클래스 등록 실패, GetLastError={}",
    err_register_options_class: "옵션 창 클래스 등록 실패, GetLastError={}",
    err_create_tray_window: "트레이 창 생성 실패",
    err_add_tray_icon: "트레이 아이콘 추가 실패",
    err_install_hook: "키보드 후크 설치 실패, GetLastError={}",
    err_create_options_window: "옵션 창 생성 실패, GetLastError={}",
    err_toggle_autostart: "자동 시작 설정 변경 실패",
    err_delay_range: "{} - {} 사이의 밀리초를 입력해 주세요",
    err_config_path: "구성 파일 위치를 확인할 수 없습니다",
    err_save_failed: "설정 저장 실패",
    err_save_autostart: "자동 시작 설정 저장 실패",
};

static FR: Strings = Strings {
    menu_autostart: "Lancer au démarrage",
    menu_options: "Options",
    menu_language: "Langue",
    menu_exit: "Quitter",
    tray_tooltip: "CapIMESwitch - Permutation intelligente de CapsLock",
    panel_title: "Options de CapIMESwitch",
    label_autostart: "Lancer au démarrage",
    label_delay: "Délai d'appui long (ms)",
    label_exclude: "Programmes exclus",
    label_language: "Langue",
    help_autostart: "Lancement automatique à l'ouverture de session ; l'état correspond au menu de la barre d'état et s'applique lors de l'enregistrement",
    help_delay: "Un appui court sur CapsLock (relâché avant ce seuil en ms) permute la méthode de saisie ; un appui long (atteignant ce seuil) active/désactive les majuscules",
    help_exclude: "Dans ces programmes, CapsLock conserve son comportement d'origine et ne permute pas la méthode de saisie ; séparez les programmes par des virgules ou des sauts de ligne",
    version_template: "Version {}",
    save: "Enregistrer",
    err_instance_lock: "Échec de la création du verrou d'instance",
    err_already_running: "CapIMESwitch est déjà en cours d'exécution. Utilisez la barre d'état système.",
    err_register_class: "Échec de l'enregistrement de la classe de fenêtre, GetLastError={}",
    err_register_options_class: "Échec de l'enregistrement de la classe de fenêtre des options, GetLastError={}",
    err_create_tray_window: "Échec de la création de la fenêtre de barre d'état",
    err_add_tray_icon: "Échec de l'ajout de l'icône de barre d'état",
    err_install_hook: "Échec de l'installation du crochet de clavier, GetLastError={}",
    err_create_options_window: "Échec de la création de la fenêtre des options, GetLastError={}",
    err_toggle_autostart: "Échec de la mise à jour du réglage de démarrage automatique",
    err_delay_range: "Veuillez saisir un nombre de millisecondes entre {} et {}",
    err_config_path: "Impossible de déterminer l'emplacement du fichier de configuration",
    err_save_failed: "Échec de l'enregistrement des paramètres",
    err_save_autostart: "Échec de l'enregistrement du réglage de démarrage automatique",
};

static DE: Strings = Strings {
    menu_autostart: "Beim Anmelden starten",
    menu_options: "Optionen",
    menu_language: "Sprache",
    menu_exit: "Beenden",
    tray_tooltip: "CapIMESwitch - Intelligente CapsLock-Umschaltung",
    panel_title: "CapIMESwitch-Optionen",
    label_autostart: "Beim Anmelden starten",
    label_delay: "Drückverzögerung (ms)",
    label_exclude: "Ausgeschlossene Programme",
    label_language: "Sprache",
    help_autostart: "Automatisch bei der Anmeldung starten; der Status entspricht dem Tray-Menü und wird beim Speichern wirksam",
    help_delay: "Kurzer Druck auf CapsLock (vor dieser ms-Zahl loslassen) wechselt die Eingabemethode; langer Druck (diese ms-Zahl erreichen) schaltet Groß-/Kleinschreibung um",
    help_exclude: "In diesen Programmen behält CapsLock sein ursprüngliches Verhalten und wechselt die Eingabemethode nicht; Programme mit Kommas oder Zeilenumbrüchen trennen",
    version_template: "Version {}",
    save: "Speichern",
    err_instance_lock: "Instance-Sperre konnte nicht erstellt werden",
    err_already_running: "CapIMESwitch läuft bereits. Bitte verwenden Sie die Taskleiste.",
    err_register_class: "Fensterklasse konnte nicht registriert werden, GetLastError={}",
    err_register_options_class: "Options-Fensterklasse konnte nicht registriert werden, GetLastError={}",
    err_create_tray_window: "Tray-Fenster konnte nicht erstellt werden",
    err_add_tray_icon: "Tray-Symbol konnte nicht hinzugefügt werden",
    err_install_hook: "Tastatur-Hook konnte nicht installiert werden, GetLastError={}",
    err_create_options_window: "Optionsfenster konnte nicht erstellt werden, GetLastError={}",
    err_toggle_autostart: "Automatische Starteinstellung konnte nicht aktualisiert werden",
    err_delay_range: "Bitte geben Sie einen Wert zwischen {} und {} Millisekunden ein",
    err_config_path: "Speicherort der Konfigurationsdatei kann nicht ermittelt werden",
    err_save_failed: "Einstellungen konnten nicht gespeichert werden",
    err_save_autostart: "Automatische Starteinstellung konnte nicht gespeichert werden",
};

static ES: Strings = Strings {
    menu_autostart: "Iniciar al iniciar sesión",
    menu_options: "Opciones",
    menu_language: "Idioma",
    menu_exit: "Salir",
    tray_tooltip: "CapIMESwitch - Cambio inteligente de CapsLock",
    panel_title: "Opciones de CapIMESwitch",
    label_autostart: "Iniciar al iniciar sesión",
    label_delay: "Retardo de pulsación larga (ms)",
    label_exclude: "Programas excluidos",
    label_language: "Idioma",
    help_autostart: "Se inicia automáticamente al iniciar sesión; el estado coincide con el menú de la bandeja y se aplica al guardar",
    help_delay: "La pulsación corta de CapsLock (soltar antes de este número de ms) cambia el método de entrada; la pulsación larga (alcanzar este número de ms) activa/desactiva las mayúsculas",
    help_exclude: "En estos programas, CapsLock conserva su comportamiento original y no cambia el método de entrada; separe varios programas con comas o saltos de línea",
    version_template: "Versión {}",
    save: "Guardar",
    err_instance_lock: "Error al crear el bloqueo de instancia",
    err_already_running: "CapIMESwitch ya está en ejecución. Use la bandeja del sistema.",
    err_register_class: "Error al registrar la clase de ventana, GetLastError={}",
    err_register_options_class: "Error al registrar la clase de ventana de opciones, GetLastError={}",
    err_create_tray_window: "Error al crear la ventana de bandeja",
    err_add_tray_icon: "Error al añadir el icono de bandeja",
    err_install_hook: "Error al instalar el gancho de teclado, GetLastError={}",
    err_create_options_window: "Error al crear la ventana de opciones, GetLastError={}",
    err_toggle_autostart: "Error al actualizar el ajuste de inicio automático",
    err_delay_range: "Introduzca un valor entre {} y {} milisegundos",
    err_config_path: "No se puede determinar la ubicación del archivo de configuración",
    err_save_failed: "Error al guardar los ajustes",
    err_save_autostart: "Error al guardar el ajuste de inicio automático",
};

static RU: Strings = Strings {
    menu_autostart: "Запускать при входе",
    menu_options: "Настройки",
    menu_language: "Язык",
    menu_exit: "Выход",
    tray_tooltip: "CapIMESwitch - Умное переключение CapsLock",
    panel_title: "Настройки CapIMESwitch",
    label_autostart: "Запускать при входе",
    label_delay: "Задержка длинного нажатия (мс)",
    label_exclude: "Исключённые программы",
    label_language: "Язык",
    help_autostart: "Автозапуск при входе в систему; состояние соответствует меню в трее и применяется при сохранении",
    help_delay: "Короткое нажатие CapsLock (отпустить раньше этого числа мс) переключает метод ввода; длинное (достичь этого числа мс) включает/выключает заглавные буквы",
    help_exclude: "В этих программах CapsLock сохраняет исходное поведение и не переключает метод ввода; разделяйте программы запятыми или переводами строк",
    version_template: "Версия {}",
    save: "Сохранить",
    err_instance_lock: "Не удалось создать блокировку экземпляра",
    err_already_running: "CapIMESwitch уже запущен. Используйте системный трей.",
    err_register_class: "Не удалось зарегистрировать класс окна, GetLastError={}",
    err_register_options_class: "Не удалось зарегистрировать класс окна настроек, GetLastError={}",
    err_create_tray_window: "Не удалось создать окно трея",
    err_add_tray_icon: "Не удалось добавить значок трея",
    err_install_hook: "Не удалось установить перехват клавиатуры, GetLastError={}",
    err_create_options_window: "Не удалось создать окно настроек, GetLastError={}",
    err_toggle_autostart: "Не удалось обновить настройку автозапуска",
    err_delay_range: "Введите значение в миллисекундах от {} до {}",
    err_config_path: "Не удалось определить расположение файла конфигурации",
    err_save_failed: "Не удалось сохранить настройки",
    err_save_autostart: "Не удалось сохранить настройку автозапуска",
};

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_recognizes_concrete_langs() {
        assert_eq!(Lang::parse("zh"), Some(Lang::Zh));
        assert_eq!(Lang::parse("en"), Some(Lang::En));
        assert_eq!(Lang::parse("ja"), Some(Lang::Ja));
        assert_eq!(Lang::parse("ko"), Some(Lang::Ko));
        assert_eq!(Lang::parse("fr"), Some(Lang::Fr));
        assert_eq!(Lang::parse("de"), Some(Lang::De));
        assert_eq!(Lang::parse("es"), Some(Lang::Es));
        assert_eq!(Lang::parse("ru"), Some(Lang::Ru));
    }

    #[test]
    fn parse_rejects_auto_and_unknown() {
        assert_eq!(Lang::parse("auto"), None);
        assert_eq!(Lang::parse("zh-CN"), None);
        assert_eq!(Lang::parse("xx"), None);
        assert_eq!(Lang::parse(""), None);
    }

    #[test]
    fn code_and_parse_roundtrip() {
        for lang in Lang::all() {
            assert_eq!(Lang::parse(lang.code()), Some(lang));
        }
    }

    #[test]
    fn all_has_eight_languages() {
        assert_eq!(Lang::all().len(), 8);
        // 语言编码两两唯一
        let codes: Vec<&str> = Lang::all().iter().map(|l| l.code()).collect();
        let mut uniq = codes.clone();
        uniq.sort_unstable();
        uniq.dedup();
        assert_eq!(uniq.len(), codes.len(), "语言编码不得重复");
    }

    #[test]
    fn normalize_keeps_valid_and_falls_back() {
        assert_eq!(normalize_language("auto"), "auto");
        for lang in Lang::all() {
            assert_eq!(normalize_language(lang.code()), lang.code());
        }
        assert_eq!(normalize_language("pt"), "auto");
        assert_eq!(normalize_language(""), "auto");
    }

    #[test]
    fn language_names_are_native_forms() {
        assert_eq!(Lang::Zh.display_name(), "简体中文");
        assert_eq!(Lang::En.display_name(), "English");
        assert_eq!(Lang::Ja.display_name(), "日本語");
        assert_eq!(Lang::Ko.display_name(), "한국어");
        assert_eq!(Lang::Fr.display_name(), "Français");
        assert_eq!(Lang::De.display_name(), "Deutsch");
        assert_eq!(Lang::Es.display_name(), "Español");
        assert_eq!(Lang::Ru.display_name(), "Русский");
        for lang in Lang::all() {
            assert!(!lang.display_name().is_empty(), "{lang:?} 语言母语名为空");
        }
    }

    #[test]
    fn all_languages_have_all_nonempty_strings() {
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
        // 核心 UI 文案在任意两种语言下必须区别明显(中英/中日等共用汉字的字段除外),
        // 证明各表均已接线而非互为别名。取区分度高的字段横向对比。
        for i in 0..Lang::all().len() {
            for j in (i + 1)..Lang::all().len() {
                let a = Lang::all()[i].strings();
                let b = Lang::all()[j].strings();
                // 拉丁系语言间这些词可能相似,但菜单选项/标题/保存按钮应各自本地化。
                assert_ne!(a.menu_exit, b.menu_exit, "{i}/{j} menu_exit 相同");
                assert_ne!(a.tray_tooltip, b.tray_tooltip, "{i}/{j} tray_tooltip 相同");
                assert_ne!(a.panel_title, b.panel_title, "{i}/{j} panel_title 相同");
                assert_ne!(a.help_delay, b.help_delay, "{i}/{j} help_delay 相同");
            }
        }
    }
}