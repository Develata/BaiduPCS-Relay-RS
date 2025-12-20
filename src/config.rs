//! 配置文件加载

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub baidu: BaiduConfig,
    #[serde(default)]
    pub web: WebConfig,
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

#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct WebConfig {
    #[serde(default)]
    pub password: String,
}

impl Config {
    /// 从文件加载配置，支持环境变量覆盖
    /// 优先级：环境变量 > 配置文件
    pub fn load(path: &str) -> Result<Self> {
        let mut config = if std::path::Path::new(path).exists() {
        let content = fs::read_to_string(path)?;
            toml::from_str(&content)?
        } else {
            // 如果文件不存在，从环境变量创建默认配置
            Config {
                baidu: BaiduConfig {
                    cookie_bduss: String::new(),
                    cookie_stoken: String::new(),
                    save_path: default_save_path(),
                    http_timeout_secs: default_http_timeout_secs(),
                },
                web: WebConfig {
                    password: String::new(),
                },
            }
        };

        // 环境变量覆盖（优先级更高）
        if let Ok(bduss) = std::env::var("BDUSS") {
            if !bduss.is_empty() {
                config.baidu.cookie_bduss = bduss;
            }
        }
        if let Ok(stoken) = std::env::var("STOKEN") {
            if !stoken.is_empty() {
                config.baidu.cookie_stoken = stoken;
            }
        }
        if let Ok(save_path) = std::env::var("SAVE_PATH") {
            if !save_path.is_empty() {
                config.baidu.save_path = save_path;
            }
        }
        if let Ok(timeout) = std::env::var("HTTP_TIMEOUT_SECS") {
            if let Ok(secs) = timeout.parse::<u64>() {
                config.baidu.http_timeout_secs = secs;
            }
        }
        if let Ok(password) = std::env::var("WEB_PASSWORD") {
            config.web.password = password;
        }

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
