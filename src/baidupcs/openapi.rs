//! 百度 OAuth 2.0 适配层

use anyhow::{anyhow, Result};
use reqwest::{StatusCode, Url};
use serde::Deserialize;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::debug;

use crate::baidupcs::types::OAuthTokenSet;
use crate::config::BaiduOpenConfig;
use crate::AppState;

const AUTHORIZE_ENDPOINT: &str = "https://openapi.baidu.com/oauth/2.0/authorize";
const TOKEN_ENDPOINT: &str = "https://openapi.baidu.com/oauth/2.0/token";
const NETDISK_SCOPE: &str = "netdisk";

pub fn is_oob_redirect(config: &BaiduOpenConfig) -> bool {
    config.redirect_uri == "oob"
}

/// 构造百度官方授权码模式 URL。Secret Key 不参与前端授权 URL。
pub fn authorization_url(config: &BaiduOpenConfig, state: &str) -> Result<String> {
    validate_oauth_client(config)?;
    if state.is_empty() {
        return Err(anyhow!("OAuth state 不能为空"));
    }

    let mut url = Url::parse(AUTHORIZE_ENDPOINT)?;
    url.query_pairs_mut()
        .append_pair("response_type", "code")
        .append_pair("client_id", &config.client_id)
        .append_pair("redirect_uri", &config.redirect_uri)
        .append_pair("scope", NETDISK_SCOPE)
        .append_pair("display", "popup")
        .append_pair("state", state);
    Ok(url.into())
}

/// 使用一次性 authorization code 换取 access_token 和 refresh_token。
pub async fn exchange_authorization_code(state: &AppState, code: &str) -> Result<OAuthTokenSet> {
    let config = &state.config.baidu_open;
    validate_oauth_client(config)?;
    if code.is_empty() {
        return Err(anyhow!("authorization code 不能为空"));
    }

    debug!("使用 authorization code 换取百度 OAuth token");
    request_token(
        state,
        &[
            ("grant_type", "authorization_code"),
            ("code", code),
            ("client_id", &config.client_id),
            ("client_secret", &config.client_secret),
            ("redirect_uri", &config.redirect_uri),
        ],
        None,
    )
    .await
}

/// 刷新 access_token，并保留百度返回的新 refresh_token。
pub async fn refresh_token_set(state: &AppState) -> Result<OAuthTokenSet> {
    let config = &state.config.baidu_open;
    if config.client_id.is_empty() || config.client_secret.is_empty() {
        return Err(anyhow!("未配置 BAIDU_CLIENT_ID 和 BAIDU_CLIENT_SECRET"));
    }
    let refresh_token = state
        .current_refresh_token()
        .ok_or_else(|| anyhow!("未配置或获取 BAIDU_REFRESH_TOKEN"))?;

    debug!("刷新百度 access_token");
    request_token(
        state,
        &[
            ("grant_type", "refresh_token"),
            ("refresh_token", &refresh_token),
            ("client_id", &config.client_id),
            ("client_secret", &config.client_secret),
        ],
        Some(&refresh_token),
    )
    .await
}

/// 兼容旧调用者，只返回 access_token；不会写入配置文件。
pub async fn refresh_token(state: &AppState) -> Result<String> {
    let tokens = refresh_token_set(state).await?;
    let access_token = tokens.access_token.clone();
    state.cache_oauth_tokens(tokens)?;
    Ok(access_token)
}

fn validate_oauth_client(config: &BaiduOpenConfig) -> Result<()> {
    if config.client_id.is_empty() || config.client_secret.is_empty() {
        return Err(anyhow!("未配置 BAIDU_CLIENT_ID 和 BAIDU_CLIENT_SECRET"));
    }
    if config.redirect_uri.is_empty() {
        return Err(anyhow!("未配置 BAIDU_REDIRECT_URI"));
    }
    if is_oob_redirect(config) {
        return Ok(());
    }
    let redirect = Url::parse(&config.redirect_uri)
        .map_err(|error| anyhow!("BAIDU_REDIRECT_URI 不是合法 URL: {error}"))?;
    if !matches!(redirect.scheme(), "http" | "https") {
        return Err(anyhow!("BAIDU_REDIRECT_URI 必须使用 http 或 https"));
    }
    Ok(())
}

async fn request_token(
    state: &AppState,
    query: &[(&str, &str)],
    fallback_refresh_token: Option<&str>,
) -> Result<OAuthTokenSet> {
    let response = state
        .oauth_client
        .get(TOKEN_ENDPOINT)
        .query(query)
        .send()
        .await
        .map_err(|_| anyhow!("百度 OAuth token 请求失败"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|_| anyhow!("读取百度 OAuth token 响应失败"))?;
    parse_token_response(status, &body, fallback_refresh_token, unix_timestamp())
}

#[derive(Deserialize)]
struct TokenResponse {
    #[serde(default)]
    access_token: String,
    #[serde(default)]
    refresh_token: String,
    #[serde(default)]
    expires_in: u64,
    #[serde(default)]
    scope: String,
    #[serde(default)]
    error: Option<String>,
}

fn parse_token_response(
    status: StatusCode,
    body: &str,
    fallback_refresh_token: Option<&str>,
    now: u64,
) -> Result<OAuthTokenSet> {
    let response: TokenResponse = serde_json::from_str(body)
        .map_err(|error| anyhow!("解析百度 OAuth token 响应失败: HTTP {}, {error}", status))?;

    if let Some(error) = response.error {
        // 官方错误描述可能原样包含 authorization code，不能传播到日志或前端。
        return Err(anyhow!("百度 OAuth 失败: {error}"));
    }
    if !status.is_success() {
        return Err(anyhow!("百度 OAuth 请求失败: HTTP {status}"));
    }
    if response.access_token.is_empty() {
        return Err(anyhow!("百度 OAuth 返回空 access_token"));
    }
    if response.expires_in == 0 {
        return Err(anyhow!("百度 OAuth 返回无效 expires_in"));
    }

    let refresh_token = if response.refresh_token.is_empty() {
        fallback_refresh_token
            .ok_or_else(|| anyhow!("百度 OAuth 返回空 refresh_token"))?
            .to_string()
    } else {
        response.refresh_token
    };
    Ok(OAuthTokenSet {
        access_token: response.access_token,
        refresh_token,
        expires_at: now.saturating_add(response.expires_in),
        scope: response.scope,
    })
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

    #[test]
    fn builds_official_authorization_code_url_without_secret() {
        let config = BaiduOpenConfig {
            client_id: "client id".to_string(),
            client_secret: "do-not-leak".to_string(),
            redirect_uri: "http://127.0.0.1:5200/oauth/callback".to_string(),
            ..BaiduOpenConfig::default()
        };

        let url = authorization_url(&config, "csrf-state").unwrap();
        let parsed = Url::parse(&url).unwrap();
        let params = parsed
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(parsed.as_str().split('?').next(), Some(AUTHORIZE_ENDPOINT));
        assert_eq!(
            params.get("response_type").map(|v| v.as_ref()),
            Some("code")
        );
        assert_eq!(
            params.get("client_id").map(|v| v.as_ref()),
            Some("client id")
        );
        assert_eq!(params.get("scope").map(|v| v.as_ref()), Some("netdisk"));
        assert_eq!(params.get("state").map(|v| v.as_ref()), Some("csrf-state"));
        assert!(!url.contains("do-not-leak"));
    }

    #[test]
    fn builds_oob_authorization_url() {
        let config = BaiduOpenConfig {
            client_id: "client-id".to_string(),
            client_secret: "do-not-leak".to_string(),
            redirect_uri: "oob".to_string(),
            ..BaiduOpenConfig::default()
        };

        let url = authorization_url(&config, "csrf-state").unwrap();
        let parsed = Url::parse(&url).unwrap();
        let params = parsed
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(params.get("redirect_uri").map(|v| v.as_ref()), Some("oob"));
        assert!(is_oob_redirect(&config));
        assert!(!url.contains("do-not-leak"));
    }

    #[test]
    fn rejects_unsupported_redirect_scheme() {
        let config = BaiduOpenConfig {
            client_id: "client-id".to_string(),
            client_secret: "client-secret".to_string(),
            redirect_uri: "file:///tmp/callback".to_string(),
            ..BaiduOpenConfig::default()
        };

        assert!(authorization_url(&config, "csrf-state").is_err());
    }

    #[test]
    fn parses_authorization_token_response() {
        let tokens = parse_token_response(
            StatusCode::OK,
            r#"{"access_token":"access","refresh_token":"refresh","expires_in":3600,"scope":"basic netdisk"}"#,
            None,
            100,
        )
        .unwrap();

        assert_eq!(tokens.access_token, "access");
        assert_eq!(tokens.refresh_token, "refresh");
        assert_eq!(tokens.expires_at, 3700);
    }

    #[test]
    fn keeps_existing_refresh_token_when_refresh_response_omits_it() {
        let tokens = parse_token_response(
            StatusCode::OK,
            r#"{"access_token":"access","expires_in":3600}"#,
            Some("existing-refresh"),
            100,
        )
        .unwrap();

        assert_eq!(tokens.refresh_token, "existing-refresh");
    }

    #[test]
    fn maps_oauth_error_without_echoing_tokens() {
        let error = parse_token_response(
            StatusCode::BAD_REQUEST,
            r#"{"error":"invalid_grant","error_description":"secret-code-value"}"#,
            None,
            100,
        )
        .err()
        .expect("expected OAuth error");

        assert!(error.to_string().contains("invalid_grant"));
        assert!(!error.to_string().contains("secret-code-value"));
    }

    #[test]
    fn rejects_authorization_response_without_refresh_token() {
        let error = parse_token_response(
            StatusCode::OK,
            r#"{"access_token":"access","expires_in":3600}"#,
            None,
            100,
        )
        .err()
        .expect("expected missing refresh token error");

        assert!(error.to_string().contains("refresh_token"));
    }
}
