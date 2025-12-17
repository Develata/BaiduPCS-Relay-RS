//! 百度网盘开放平台 - 只保留 token 刷新功能

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tracing::{debug, info, warn};
use std::fs;

use crate::{AppState, config::Config};

/// 刷新 access_token
pub async fn refresh_token(state: &AppState) -> Result<String> {
    let opencfg = &state.config.baidu_open;

    if opencfg.client_id.is_empty() || opencfg.client_secret.is_empty() {
        return Err(anyhow!("未配置 client_id 和 client_secret"));
    }

    if opencfg.refresh_token.is_empty() {
        return Err(anyhow!("未配置 refresh_token"));
    }

    let url = format!(
        "https://openapi.baidu.com/oauth/2.0/token?grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
        urlencoding::encode(&opencfg.refresh_token),
        urlencoding::encode(&opencfg.client_id),
        urlencoding::encode(&opencfg.client_secret),
    );

    debug!("🔄 刷新 access_token...");

    let resp = state.client.get(&url).send().await?;
    let status = resp.status();
    let text = resp.text().await?;

    #[derive(Deserialize)]
    struct TokenResponse {
        #[serde(default)]
        access_token: String,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        error_description: Option<String>,
    }

    if !status.is_success() {
        return Err(anyhow!("刷新 access_token 失败: HTTP {}, body: {}", status, text));
    }

    let token: TokenResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 refresh_token 响应失败: {}, body: {}", e, text))?;

    if let Some(err) = token.error {
        return Err(anyhow!(
            "refresh_token 失败: {}, {}",
            err,
            token.error_description.unwrap_or_default()
        ));
    }

    if token.access_token.is_empty() {
        return Err(anyhow!("refresh_token 返回空 access_token"));
    }

    if let Err(e) = save_access_token(&token.access_token) {
        warn!("⚠️ 保存 access_token 失败: {}", e);
    }

    Ok(token.access_token)
}

fn save_access_token(new_token: &str) -> Result<()> {
    let path = "config.toml";
    let content = fs::read_to_string(path)?;
    
    let mut cfg: Config = toml::from_str(&content)?;
    cfg.baidu_open.access_token = new_token.to_string();
    
    let new_content = toml::to_string_pretty(&cfg)?;
    fs::write(path, new_content)?;
    
    info!("✅ 已更新 access_token 到配置文件");
    Ok(())
}
