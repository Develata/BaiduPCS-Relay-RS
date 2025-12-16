//! 配置文件加载

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub baidu: BaiduConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BaiduConfig {
    pub cookie_bduss: String,
    pub cookie_stoken: String,
    #[serde(default = "default_save_path")]
    pub save_path: String,
    #[serde(default = "default_http_timeout_secs")]
    pub http_timeout_secs: u64,
}

fn default_save_path() -> String {
    "/我的资源".to_string()
}

fn default_http_timeout_secs() -> u64 {
    30
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    /// 获取应用 User-Agent
    pub fn app_ua() -> &'static str {
        "netdisk;2.2.51.6;netdisk;10.0.63;PC;android-android"
    }

    /// 获取浏览器 User-Agent
    pub fn browser_ua() -> &'static str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    }
}
