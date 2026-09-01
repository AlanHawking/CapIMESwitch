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

/// 应用配置
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    /// 长按 CapsLock 切换大小写所需毫秒数(短于此值松开则切换输入法)
    #[serde(default = "default_long_press_ms")]
    pub long_press_ms: u32,
}

fn default_long_press_ms() -> u32 {
    DEFAULT_LONG_PRESS_MS
}

impl Default for Config {
    fn default() -> Self {
        Self {
            long_press_ms: DEFAULT_LONG_PRESS_MS,
        }
    }
}

impl Config {
    /// 归一化非法值(越界等)到默认值,保证运行时阈值永远落在有效范围内
    pub fn sanitized(mut self) -> Self {
        if !(MIN_LONG_PRESS_MS..=MAX_LONG_PRESS_MS).contains(&self.long_press_ms) {
            self.long_press_ms = DEFAULT_LONG_PRESS_MS;
        }
        self
    }
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
        };
        assert!(save(&cfg, &p));
        assert_eq!(load(&p), cfg);
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
}