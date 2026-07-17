//! 应用状态与 HTTP 客户端

use anyhow::{anyhow, Result};
use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use reqwest::{cookie::Jar, Client, Url};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::baidupcs::OAuthTokenSet;
use crate::config::Config;

const MAX_PENDING_OAUTH_STATES: usize = 64;

pub struct AppState {
    pub config: Config,
    pub client: Client,
    pub no_redirect_client: Client,
    pub oauth_client: Client,
    oauth_token_cache: Mutex<Option<OAuthTokenSet>>,
    oauth_states: Mutex<HashMap<String, u64>>,
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
        // OAuth token 请求包含 Client Secret，不携带网盘 Cookie，也不跟随重定向。
        let oauth_client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(timeout)
            .build()?;

        Ok(Self {
            config,
            client,
            no_redirect_client,
            oauth_client,
            oauth_token_cache: Mutex::new(None),
            oauth_states: Mutex::new(HashMap::new()),
        })
    }

    pub fn cached_access_token(&self) -> Option<String> {
        let now = unix_timestamp();
        self.oauth_token_cache
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
            .filter(|tokens| tokens.expires_at == 0 || tokens.expires_at > now.saturating_add(60))
            .map(|tokens| tokens.access_token)
    }

    pub fn cache_oauth_tokens(&self, tokens: OAuthTokenSet) -> Result<()> {
        let mut guard = self
            .oauth_token_cache
            .lock()
            .map_err(|_| anyhow!("OAuth token provider 不可用"))?;
        *guard = Some(tokens);
        Ok(())
    }

    pub fn oauth_tokens(&self) -> Option<OAuthTokenSet> {
        self.oauth_token_cache
            .lock()
            .ok()
            .and_then(|guard| guard.clone())
    }

    pub fn current_refresh_token(&self) -> Option<String> {
        self.oauth_tokens()
            .map(|tokens| tokens.refresh_token)
            .filter(|token| !token.is_empty())
            .or_else(|| {
                (!self.config.baidu_open.refresh_token.is_empty())
                    .then(|| self.config.baidu_open.refresh_token.clone())
            })
    }

    pub fn issue_oauth_state(&self, now: u64, ttl_secs: u64) -> Result<String> {
        let mut bytes = [0u8; 32];
        getrandom::fill(&mut bytes).map_err(|error| anyhow!("生成 OAuth state 失败: {error}"))?;
        let state = URL_SAFE_NO_PAD.encode(bytes);

        let mut states = self
            .oauth_states
            .lock()
            .map_err(|_| anyhow!("OAuth state 存储不可用"))?;
        states.retain(|_, expires_at| *expires_at >= now);
        if states.len() >= MAX_PENDING_OAUTH_STATES {
            return Err(anyhow!("待处理 OAuth 授权请求过多，请稍后重试"));
        }
        states.insert(state.clone(), now.saturating_add(ttl_secs));
        Ok(state)
    }

    pub fn consume_oauth_state(&self, state: &str, now: u64) -> bool {
        let Ok(mut states) = self.oauth_states.lock() else {
            return false;
        };
        states.retain(|_, expires_at| *expires_at >= now);
        states.remove(state).is_some()
    }
}

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_state() -> AppState {
        let mut config = Config::default();
        config.baidu.cookie_bduss = "b".repeat(80);
        config.baidu.cookie_stoken = "s".repeat(40);
        AppState::new(config).unwrap()
    }

    #[test]
    fn oauth_state_is_single_use() {
        let state = test_state();
        let nonce = state.issue_oauth_state(100, 600).unwrap();

        assert!(state.consume_oauth_state(&nonce, 101));
        assert!(!state.consume_oauth_state(&nonce, 102));
    }

    #[test]
    fn oauth_state_expires() {
        let state = test_state();
        let nonce = state.issue_oauth_state(100, 600).unwrap();

        assert!(!state.consume_oauth_state(&nonce, 701));
    }

    #[test]
    fn latest_refresh_token_comes_from_memory() {
        let mut state = test_state();
        state.config.baidu_open.refresh_token = "configured".to_string();
        state
            .cache_oauth_tokens(OAuthTokenSet {
                access_token: "access".to_string(),
                refresh_token: "rotated".to_string(),
                expires_at: 0,
                scope: "netdisk".to_string(),
            })
            .unwrap();

        assert_eq!(state.current_refresh_token().as_deref(), Some("rotated"));
    }

    #[test]
    fn pending_oauth_state_store_is_bounded() {
        let state = test_state();
        for _ in 0..MAX_PENDING_OAUTH_STATES {
            state.issue_oauth_state(100, 600).unwrap();
        }

        assert!(state.issue_oauth_state(100, 600).is_err());
    }
}
