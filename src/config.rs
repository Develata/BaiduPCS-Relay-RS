//! 配置文件加载

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fs;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub baidu: BaiduConfig,
    #[serde(default)]  // ✅ 如果配置文件没有 [web] 就用默认值
    pub web: WebConfig,
    #[serde(default)]  // ✅ 百度开放平台 / 本地签名相关配置（预留）
    pub baidu_open: BaiduOpenConfig,
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

// ✅ 新增 Web 配置
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WebConfig {
    #[serde(default = "default_access_token")]
    pub access_token: String,
    /// 本地直链签名密钥，用于生成 /d/...?...sign= 链接
    #[serde(default = "default_sign_secret")]
    pub sign_secret: String,
    /// ZIP 压缩包最大大小限制 (字节)，超过此大小会返回错误。默认 2GB
    #[serde(default = "default_max_zip_size")]
    pub max_zip_size: u64,
}

fn default_save_path() -> String {
    "/我的资源".to_string()
}

fn default_http_timeout_secs() -> u64 {
    30
}

fn default_access_token() -> String {
    // ✅ 优先使用环境变量，如果没有就用默认值
    std::env::var("WEB_ACCESS_TOKEN").unwrap_or_else(|_| "change-me".to_string())
}

fn default_sign_secret() -> String {
    std::env::var("WEB_SIGN_SECRET").unwrap_or_else(|_| "change-me-sign".to_string())
}

fn default_max_zip_size() -> u64 {
    // 默认 2GB，可通过 MAX_ZIP_SIZE 环境变量覆盖（单位：字节）
    // 例如: MAX_ZIP_SIZE=1073741824 (1GB) 或 MAX_ZIP_SIZE=2147483648 (2GB)
    std::env::var("MAX_ZIP_SIZE")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(2 * 1024 * 1024 * 1024) // 2GB default
}

/// 百度开放平台 / OAuth 相关配置（当前主要用于对齐 OpenList 策略，后续可扩展）
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct BaiduOpenConfig {
    /// 百度开放平台应用的 Client ID / API Key
    #[serde(default)]
    pub client_id: String,
    /// 百度开放平台应用的 Client Secret
    #[serde(default)]
    pub client_secret: String,
    /// OAuth 回调地址（如果你在别处完成授权，可留空）
    #[serde(default)]
    pub redirect_uri: String,
    /// 长期有效的 refresh_token（推荐）或 access_token（如果你已有）
    #[serde(default)]
    pub refresh_token: String,
    /// 备用：手动填写的 access_token（优先使用 refresh_token 刷新）
    #[serde(default)]
    pub access_token: String,
}


impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }

    pub fn app_ua() -> &'static str {
        "netdisk;2.2.51.6;netdisk;10.0.63;PC;android-android"
    }

    pub fn browser_ua() -> &'static str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    }
}
