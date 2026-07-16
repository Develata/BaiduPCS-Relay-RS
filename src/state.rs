//! 应用状态与 HTTP 客户端

use anyhow::{anyhow, Result};
use reqwest::{cookie::Jar, Client, Url};
use std::sync::{Arc, Mutex};

use crate::config::Config;

pub struct AppState {
    pub config: Config,
    pub client: Client,
    pub no_redirect_client: Client,
    access_token_cache: Mutex<Option<String>>,
}

impl AppState {
    pub fn new(config: Config) -> Result<Self> {
        if config.baidu.cookie_bduss.is_empty() || config.baidu.cookie_bduss.len() < 50 {
            return Err(anyhow!(
                "BDUSS 未配置或长度不足，请通过 config.toml 或环境变量 BDUSS 设置完整的 BDUSS"
            ));
        }
        if config.baidu.cookie_stoken.is_empty() || config.baidu.cookie_stoken.len() < 30 {
            return Err(anyhow!(
                "STOKEN 未配置或长度不足，请通过 config.toml 或环境变量 STOKEN 设置完整的 STOKEN"
            ));
        }

        let jar = Arc::new(Jar::default());
        let domains = [
            "https://baidu.com",
            "https://pan.baidu.com",
            "https://passport.baidu.com",
        ];

        for domain in domains {
            let url = domain.parse::<Url>()?;
            jar.add_cookie_str(
                &format!(
                    "BDUSS={}; Domain=.baidu.com; Path=/",
                    config.baidu.cookie_bduss
                ),
                &url,
            );
            jar.add_cookie_str(
                &format!(
                    "STOKEN={}; Domain=.baidu.com; Path=/",
                    config.baidu.cookie_stoken
                ),
                &url,
            );
        }

        let timeout = std::time::Duration::from_secs(config.baidu.http_timeout_secs);
        let client = Client::builder()
            .cookie_provider(Arc::clone(&jar))
            .timeout(timeout)
            .build()?;
        let no_redirect_client = Client::builder()
            .cookie_provider(jar)
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()?;

        Ok(Self {
            config,
            client,
            no_redirect_client,
            access_token_cache: Mutex::new(None),
        })
    }

    pub fn cached_access_token(&self) -> Option<String> {
        self.access_token_cache
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub fn cache_access_token(&self, token: String) {
        if let Ok(mut guard) = self.access_token_cache.lock() {
            *guard = Some(token);
        }
    }
}
