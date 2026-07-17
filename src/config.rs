//! 配置文件加载

use anyhow::{anyhow, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Config {
    pub baidu: BaiduConfig,
    #[serde(default)]
    pub web: WebConfig,
    #[serde(default)]
    pub baidu_open: BaiduOpenConfig,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct BaiduConfig {
    #[serde(default)]
    pub cookie_bduss: String,
    #[serde(default)]
    pub cookie_stoken: String,
    #[serde(default = "default_save_path")]
    pub save_path: String,
    #[serde(default = "default_http_timeout_secs")]
    pub http_timeout_secs: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct WebConfig {
    #[serde(default = "default_access_token")]
    pub access_token: String,
    /// 本地直链签名密钥，用于生成 /d/download?...sign= 链接。
    #[serde(default = "default_sign_secret")]
    pub sign_secret: String,
    /// v1 暂不提供 ZIP 能力；保留该字段只为配置兼容。
    #[serde(default = "default_max_zip_size")]
    pub max_zip_size: u64,
}

/// 百度开放平台 / OAuth 相关配置。
#[derive(Debug, Clone, Deserialize, Serialize, Default)]
pub struct BaiduOpenConfig {
    #[serde(default)]
    pub client_id: String,
    #[serde(default)]
    pub client_secret: String,
    #[serde(default)]
    pub redirect_uri: String,
    #[serde(default)]
    pub refresh_token: String,
    #[serde(default)]
    pub access_token: String,
}

impl Config {
    pub fn load(path: &str) -> Result<Self> {
        let mut config = if Path::new(path).exists() {
            let content = fs::read_to_string(path)?;
            toml::from_str(&content)?
        } else {
            Config::default()
        };

        config.apply_env_overrides();
        Ok(config)
    }

    pub fn config_path_from_env_or_arg(arg: Option<String>) -> String {
        std::env::var("CONFIG_PATH")
            .ok()
            .filter(|s| !s.is_empty())
            .or(arg)
            .unwrap_or_else(|| "config.toml".to_string())
    }

    pub fn validate_web(&self) -> Result<()> {
        if self.web.access_token.is_empty() || self.web.access_token == "change-me" {
            return Err(anyhow!(
                "WEB_ACCESS_TOKEN/[web].access_token 不能为空或使用默认值 change-me"
            ));
        }
        if !is_bearer_token_safe(&self.web.access_token) {
            return Err(anyhow!(
                "WEB_ACCESS_TOKEN/[web].access_token 只能包含 Bearer token 允许的 ASCII 字符，不能包含空白或控制字符"
            ));
        }
        if self.web.sign_secret.is_empty() || self.web.sign_secret == "change-me-sign" {
            return Err(anyhow!(
                "WEB_SIGN_SECRET/[web].sign_secret 不能为空或使用默认值 change-me-sign"
            ));
        }
        let has_access_token = !self.baidu_open.access_token.is_empty();
        let has_refresh_source = !self.baidu_open.refresh_token.is_empty()
            && !self.baidu_open.client_id.is_empty()
            && !self.baidu_open.client_secret.is_empty();
        let can_start_oauth = !self.baidu_open.client_id.is_empty()
            && !self.baidu_open.client_secret.is_empty()
            && is_supported_oauth_redirect(&self.baidu_open.redirect_uri);
        if !has_access_token && !has_refresh_source && !can_start_oauth {
            return Err(anyhow!(
                "Web 直链下载需要配置 BAIDU_ACCESS_TOKEN；或配置 BAIDU_REFRESH_TOKEN、BAIDU_CLIENT_ID、BAIDU_CLIENT_SECRET；或配置 BAIDU_CLIENT_ID、BAIDU_CLIENT_SECRET，并将 BAIDU_REDIRECT_URI 设为 oob 或 http(s) 回调地址后通过前端授权"
            ));
        }
        Ok(())
    }

    fn apply_env_overrides(&mut self) {
        override_string(&mut self.baidu.cookie_bduss, "BDUSS");
        override_string(&mut self.baidu.cookie_stoken, "STOKEN");
        override_string(&mut self.baidu.save_path, "SAVE_PATH");
        override_u64(&mut self.baidu.http_timeout_secs, "HTTP_TIMEOUT_SECS");

        override_string(&mut self.web.access_token, "WEB_ACCESS_TOKEN");
        override_string(&mut self.web.sign_secret, "WEB_SIGN_SECRET");
        override_u64(&mut self.web.max_zip_size, "MAX_ZIP_SIZE");

        override_string(&mut self.baidu_open.client_id, "BAIDU_CLIENT_ID");
        override_string(&mut self.baidu_open.client_secret, "BAIDU_CLIENT_SECRET");
        override_string(&mut self.baidu_open.redirect_uri, "BAIDU_REDIRECT_URI");
        override_string(&mut self.baidu_open.refresh_token, "BAIDU_REFRESH_TOKEN");
        override_string(&mut self.baidu_open.access_token, "BAIDU_ACCESS_TOKEN");
    }

    pub fn app_ua() -> &'static str {
        "netdisk;2.2.51.6;netdisk;10.0.63;PC;android-android"
    }

    pub fn browser_ua() -> &'static str {
        "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0.0.0 Safari/537.36"
    }

    pub fn dlink_ua() -> &'static str {
        "pan.baidu.com"
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            baidu: BaiduConfig {
                cookie_bduss: String::new(),
                cookie_stoken: String::new(),
                save_path: default_save_path(),
                http_timeout_secs: default_http_timeout_secs(),
            },
            web: WebConfig {
                access_token: default_access_token(),
                sign_secret: default_sign_secret(),
                max_zip_size: default_max_zip_size(),
            },
            baidu_open: BaiduOpenConfig::default(),
        }
    }
}

fn default_save_path() -> String {
    "/我的资源".to_string()
}

fn default_http_timeout_secs() -> u64 {
    30
}

fn default_access_token() -> String {
    "change-me".to_string()
}

fn default_sign_secret() -> String {
    "change-me-sign".to_string()
}

fn default_max_zip_size() -> u64 {
    2 * 1024 * 1024 * 1024
}

fn is_bearer_token_safe(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            byte.is_ascii_alphanumeric()
                || matches!(byte, b'-' | b'.' | b'_' | b'~' | b'+' | b'/' | b'=')
        })
}

fn is_supported_oauth_redirect(value: &str) -> bool {
    value == "oob" || value.starts_with("http://") || value.starts_with("https://")
}

fn override_string(target: &mut String, key: &str) {
    if let Ok(value) = std::env::var(key) {
        if !value.is_empty() {
            *target = value;
        }
    }
}

fn override_u64(target: &mut u64, key: &str) {
    if let Ok(value) = std::env::var(key) {
        if let Ok(parsed) = value.parse::<u64>() {
            *target = parsed;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Mutex;

    static ENV_LOCK: Mutex<()> = Mutex::new(());

    #[test]
    fn loads_from_env_without_config_file() {
        let _guard = ENV_LOCK.lock().unwrap();
        clear_env();
        set_env("BDUSS", "x".repeat(80));
        set_env("STOKEN", "y".repeat(40));
        set_env("SAVE_PATH", "/target");
        set_env("HTTP_TIMEOUT_SECS", "77");
        set_env("WEB_ACCESS_TOKEN", "token");
        set_env("WEB_SIGN_SECRET", "secret");
        set_env("BAIDU_ACCESS_TOKEN", "baidu-token");

        let config = Config::load("/tmp/baidupcs-relay-nonexistent-config.toml").unwrap();
        assert_eq!(config.baidu.cookie_bduss, "x".repeat(80));
        assert_eq!(config.baidu.cookie_stoken, "y".repeat(40));
        assert_eq!(config.baidu.save_path, "/target");
        assert_eq!(config.baidu.http_timeout_secs, 77);
        assert_eq!(config.web.access_token, "token");
        assert_eq!(config.web.sign_secret, "secret");
        assert_eq!(config.baidu_open.access_token, "baidu-token");

        clear_env();
    }

    #[test]
    fn rejects_default_web_secrets() {
        let config = Config::default();
        let err = config.validate_web().unwrap_err();
        assert!(err.to_string().contains("change-me"));
    }

    #[test]
    fn rejects_web_access_token_that_cannot_be_sent_as_bearer_token() {
        let mut config = Config::default();
        config.web.access_token = "token with spaces".to_string();
        config.web.sign_secret = "sign-secret".to_string();
        config.baidu_open.access_token = "baidu-token".to_string();

        let err = config.validate_web().unwrap_err();
        assert!(err.to_string().contains("Bearer token"));
    }

    #[test]
    fn rejects_missing_baidu_open_token_for_web() {
        let mut config = Config::default();
        config.web.access_token = "web-token".to_string();
        config.web.sign_secret = "sign-secret".to_string();

        let err = config.validate_web().unwrap_err();
        assert!(err.to_string().contains("BAIDU_ACCESS_TOKEN"));
    }

    #[test]
    fn accepts_static_baidu_access_token_for_web() {
        let mut config = Config::default();
        config.web.access_token = "web-token".to_string();
        config.web.sign_secret = "sign-secret".to_string();
        config.baidu_open.access_token = "baidu-token".to_string();

        config.validate_web().unwrap();
    }

    #[test]
    fn accepts_refresh_token_source_for_web() {
        let mut config = Config::default();
        config.web.access_token = "web-token".to_string();
        config.web.sign_secret = "sign-secret".to_string();
        config.baidu_open.client_id = "client-id".to_string();
        config.baidu_open.client_secret = "client-secret".to_string();
        config.baidu_open.refresh_token = "refresh-token".to_string();

        config.validate_web().unwrap();
    }

    #[test]
    fn accepts_oauth_client_without_existing_token() {
        let mut config = Config::default();
        config.web.access_token = "web-token".to_string();
        config.web.sign_secret = "sign-secret".to_string();
        config.baidu_open.client_id = "client-id".to_string();
        config.baidu_open.client_secret = "client-secret".to_string();
        config.baidu_open.redirect_uri = "http://127.0.0.1:5200/oauth/callback".to_string();

        config.validate_web().unwrap();
    }

    #[test]
    fn accepts_oob_oauth_client_without_existing_token() {
        let mut config = Config::default();
        config.web.access_token = "web-token".to_string();
        config.web.sign_secret = "sign-secret".to_string();
        config.baidu_open.client_id = "client-id".to_string();
        config.baidu_open.client_secret = "client-secret".to_string();
        config.baidu_open.redirect_uri = "oob".to_string();

        config.validate_web().unwrap();
    }

    #[test]
    fn uses_baidu_dlink_user_agent() {
        assert_eq!(Config::dlink_ua(), "pan.baidu.com");
    }

    fn set_env(key: &str, value: impl AsRef<str>) {
        std::env::set_var(key, value.as_ref());
    }

    fn clear_env() {
        for key in [
            "BDUSS",
            "STOKEN",
            "SAVE_PATH",
            "HTTP_TIMEOUT_SECS",
            "WEB_ACCESS_TOKEN",
            "WEB_SIGN_SECRET",
            "BAIDU_CLIENT_ID",
            "BAIDU_CLIENT_SECRET",
            "BAIDU_REDIRECT_URI",
            "BAIDU_REFRESH_TOKEN",
            "BAIDU_ACCESS_TOKEN",
        ] {
            std::env::remove_var(key);
        }
    }
}
