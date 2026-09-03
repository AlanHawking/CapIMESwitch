//! 应用配置持久化:exe 同目录 `config.toml`。
//!
//! 设计要点:
//! - 配置可人工编辑、随目录整体迁移(portable),并为后续功能预留扩展空间。
//! - 新增字段一律加 `#[serde(default)]`,旧配置文件缺失字段自动取默认值。
//! - 文件缺失或损坏时回退默认值,不自动写回;首次落盘由选项面板的保存按钮触发。

use serde::{Deserialize, Serialize};
use std::path::Path;

/// 长按判定阈值默认值(毫秒)
pub const DEFAULT_LONG_PRESS_MS: u32 = 500;
/// 长按判定阈值允许范围
pub const MIN_LONG_PRESS_MS: u32 = 100;
pub const MAX_LONG_PRESS_MS: u32 = 5000;
/// 语言偏好默认值(跟随系统 UI 语言)
pub const DEFAULT_LANGUAGE: &str = "auto";

/// 应用配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// 长按 CapsLock 切换大小写所需毫秒数(短于此值松开则切换输入法)
    #[serde(default = "default_long_press_ms")]
    pub long_press_ms: u32,
    /// 排除程序列表(exe 文件名,小写):在这些程序中 CapsLock 恢复原始行为
    #[serde(default)]
    pub exclude_processes: Vec<String>,
    /// 语言偏好:auto(跟随系统 UI 语言)/zh/en;缺失或非法回退 auto
    #[serde(default = "default_language")]
    pub language: String,
}

fn default_long_press_ms() -> u32 {
    DEFAULT_LONG_PRESS_MS
}

fn default_language() -> String {
    DEFAULT_LANGUAGE.to_string()
}

impl Default for Config {
    fn default() -> Self {
        Self {
            long_press_ms: DEFAULT_LONG_PRESS_MS,
            exclude_processes: Vec::new(),
            language: DEFAULT_LANGUAGE.to_string(),
        }
    }
}

impl Config {
    /// 归一化非法值(越界等)到默认值,保证运行时阈值永远落在有效范围内
    pub fn sanitized(mut self) -> Self {
        if !(MIN_LONG_PRESS_MS..=MAX_LONG_PRESS_MS).contains(&self.long_press_ms) {
            self.long_press_ms = DEFAULT_LONG_PRESS_MS;
        }
        // 统一 trim + 转小写 + 去空项,保证与 UI 保存时写入的格式一致
        self.exclude_processes = parse_exclude_list(&self.exclude_processes.join(","));
        // 语言值仅保留 auto/zh/en,其余回退 auto
        self.language = crate::i18n::normalize_language(&self.language);
        self
    }
}

/// 解析排除程序输入:逗号/全角逗号/换行分隔,逐项 trim 并转小写,丢弃空项
pub fn parse_exclude_list(s: &str) -> Vec<String> {
    s.split([',', '，', '\n', '\r'])
        .map(str::trim)
        .filter(|item| !item.is_empty())
        .map(str::to_lowercase)
        .collect()
}

/// 判断 exe 文件名是否命中排除列表(大小写不敏感精确匹配,不匹配前缀)
pub fn is_excluded(exe: &str, list: &[String]) -> bool {
    list.iter().any(|item| item.eq_ignore_ascii_case(exe))
}

/// 从路径加载配置:缺失、解析失败或值非法时回退默认
pub fn load(path: &Path) -> Config {
    std::fs::read_to_string(path)
        .ok()
        .and_then(|s| toml::from_str::<Config>(&s).ok())
        .map(Config::sanitized)
        .unwrap_or_default()
}

/// 保存配置到路径,返回是否成功
pub fn save(cfg: &Config, path: &Path) -> bool {
    match toml::to_string(cfg) {
        Ok(text) => std::fs::write(path, text).is_ok(),
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;

    fn temp_path(name: &str) -> std::path::PathBuf {
        std::env::temp_dir().join(format!("cap_ime_switch_{name}.toml"))
    }

    #[test]
    fn load_missing_file_returns_default() {
        let p = temp_path("missing");
        let _ = fs::remove_file(&p);
        assert_eq!(load(&p), Config::default());
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn save_then_load_roundtrip() {
        let p = temp_path("roundtrip");
        let _ = fs::remove_file(&p);
        let cfg = Config {
            long_press_ms: 800,
            exclude_processes: vec!["a.exe".to_string(), "b.exe".to_string()],
            language: "en".to_string(),
        };
        assert!(save(&cfg, &p));
        assert_eq!(load(&p), cfg);
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn language_defaults_to_auto() {
        let p = temp_path("lang");
        let _ = fs::remove_file(&p);
        fs::write(&p, "long_press_ms = 600\n").unwrap();
        let cfg = load(&p);
        assert_eq!(cfg.language, "auto");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn invalid_language_falls_back_to_auto() {
        let p = temp_path("langbad");
        let _ = fs::remove_file(&p);
        fs::write(&p, "language = \"fr\"\n").unwrap();
        assert_eq!(load(&p).language, "auto");
        fs::write(&p, "language = \"zh\"\n").unwrap();
        assert_eq!(load(&p).language, "zh");
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn load_corrupted_file_returns_default() {
        let p = temp_path("corrupt");
        let _ = fs::remove_file(&p);
        fs::write(&p, "long_press_ms = 不是数字{{{").unwrap();
        assert_eq!(load(&p), Config::default());
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn load_out_of_range_returns_default() {
        let p = temp_path("range");
        let _ = fs::remove_file(&p);
        fs::write(&p, "long_press_ms = 99999\n").unwrap();
        assert_eq!(load(&p), Config::default());
        fs::write(&p, "long_press_ms = 50\n").unwrap();
        assert_eq!(load(&p), Config::default());
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn missing_field_gets_default() {
        let p = temp_path("field");
        let _ = fs::remove_file(&p);
        fs::write(&p, "# 空配置\n").unwrap();
        assert_eq!(load(&p), Config::default());
        let _ = fs::remove_file(&p);
    }

    #[test]
    fn parse_exclude_list_splits_and_normalizes() {
        let list = parse_exclude_list(" Notepad.exe , CODE.EXE,  , notepad.exe\nvscode.exe");
        assert_eq!(
            list,
            vec![
                "notepad.exe",
                "code.exe",
                "notepad.exe",
                "vscode.exe"
            ]
        );
    }

    #[test]
    fn parse_exclude_list_supports_fullwidth_comma_and_blank() {
        let list = parse_exclude_list("wechat.exe，qq.exe");
        assert_eq!(list, vec!["wechat.exe", "qq.exe"]);
        assert!(parse_exclude_list(" , ，\n ").is_empty());
        assert!(parse_exclude_list("").is_empty());
    }

    #[test]
    fn is_excluded_matches_case_insensitively() {
        assert!(is_excluded("NotePad.EXE", &["notepad.exe".to_string()]));
        assert!(is_excluded("notepad.exe", &["NOTEPAD.EXE".to_string()]));
    }

    #[test]
    fn is_excluded_rejects_partial_matches_and_empty_list() {
        assert!(!is_excluded("notepad2.exe", &["notepad.exe".to_string()]));
        assert!(!is_excluded("notepad.exe", &[]));
    }

    #[test]
    fn sanitized_normalizes_exclude_list() {
        let cfg = Config {
            long_press_ms: 500,
            exclude_processes: vec![" Notepad.EXE ".to_string()],
            language: "auto".to_string(),
        };
        let cfg = cfg.sanitized();
        assert_eq!(cfg.exclude_processes, vec!["notepad.exe".to_string()]);
        assert_eq!(cfg.long_press_ms, 500);
        assert_eq!(cfg.language, "auto");
    }

    #[test]
    fn sanitized_normalizes_invalid_language() {
        let cfg = Config {
            long_press_ms: 500,
            exclude_processes: Vec::new(),
            language: "fr".to_string(),
        };
        assert_eq!(cfg.sanitized().language, "auto");
    }
}