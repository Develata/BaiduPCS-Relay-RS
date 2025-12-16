//! 最小应用状态（仅用于百度网盘转存）

use anyhow::{anyhow, Result};
use reqwest::{cookie::Jar, Client, Url};
use std::sync::Arc;

use crate::config::Config;

pub struct AppState {
    pub config: Config,
    pub client: Client,
}

impl AppState {
    pub fn new(config: Config) -> Result<Self> {
        if config.baidu.cookie_bduss.is_empty() || config.baidu.cookie_bduss.len() < 50 {
            return Err(anyhow!(
                "BDUSS 未配置或长度不足，请在 config.toml 中设置完整的 BDUSS"
            ));
        }
        if config.baidu.cookie_stoken.is_empty() || config.baidu.cookie_stoken.len() < 30 {
            return Err(anyhow!(
                "STOKEN 未配置或长度不足，请在 config.toml 中设置完整的 STOKEN"
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

        let client = Client::builder()
            .cookie_provider(jar)
            .timeout(std::time::Duration::from_secs(
                config.baidu.http_timeout_secs,
            ))
            .build()?;

        Ok(Self { config, client })
    }
}
