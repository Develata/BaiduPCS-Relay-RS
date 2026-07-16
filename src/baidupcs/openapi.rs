//! 百度网盘开放平台 - token 刷新能力

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tracing::debug;

use crate::AppState;

/// 刷新 access_token。
///
/// 注意：这里不再写回 config.toml。token 的持久化应由调用者或部署系统负责，
/// 能力层只返回本次刷新结果，避免业务流程隐式修改配置文件。
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

    debug!("刷新百度 access_token");
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
        return Err(anyhow!(
            "刷新 access_token 失败: HTTP {}, body: {}",
            status,
            text
        ));
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

    Ok(token.access_token)
}
