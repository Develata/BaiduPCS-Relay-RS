//! Web 服务器 - OpenList 方案

use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::{Html, IntoResponse, Json, Redirect},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use baidu_direct_link::{baidupcs, AppState};

use base64::engine::general_purpose::URL_SAFE_NO_PAD;
use base64::Engine;
use hmac::{Hmac, Mac};
use sha2::Sha256;

#[derive(Deserialize)]
struct ConvertRequest {
    link: String,
    #[serde(default)]
    pwd: String,
    #[serde(default)]
    token: String,
}

#[derive(Serialize)]
struct ConvertResponse {
    success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    links: Option<Vec<FileLink>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<String>,
}

#[derive(Serialize)]
struct FileLink {
    filename: String,
    download_url: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "baidu_direct_link=info,web_server=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    info!("🚀 百度网盘直链 Web 服务启动中（OpenList 方案）...");

    let config = baidu_direct_link::config::Config::load("config.toml")?;
    info!("✅ 配置加载完成");

    info!("🔑 访问密码: {}", config.web.access_token);
    if config.web.access_token == "change-me" {
        info!("⚠️  警告: 使用默认密码，请在 config.toml 中修改 [web] access_token");
    }

    let state = Arc::new(AppState::new(config)?);
    info!("✅ HTTP Client 初始化完成");

    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/convert", post(convert_handler))
        .route("/d/download", get(download_handler))
        .route("/health", get(health_handler))
        .with_state(state);

    let addr = "0.0.0.0:5200";
    info!("🌐 Web 服务器启动: http://localhost:5200");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../../templates/index.html"))
}

async fn health_handler() -> &'static str {
    "OK"
}

async fn convert_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConvertRequest>,
) -> impl IntoResponse {
    info!("📥 收到转换请求: {}", req.link);

    let correct_token = &state.config.web.access_token;
    
    if req.token.is_empty() {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ConvertResponse {
                success: false,
                links: None,
                error: Some("请输入访问密码".to_string()),
            }),
        );
    }

    if req.token != *correct_token {
        info!("❌ 访问密码错误");
        return (
            StatusCode::UNAUTHORIZED,
            Json(ConvertResponse {
                success: false,
                links: None,
                error: Some("访问密码错误".to_string()),
            }),
        );
    }

    info!("✅ 访问密码验证通过");

    match baidupcs::share_to_direct_link(state.as_ref(), &req.link, &req.pwd).await {
        Ok(files) => {
            let file_links: Vec<FileLink> = files
                .into_iter()
                .map(|(fsid, filename)| {
                    let signed_url = generate_signed_link(
                        &state.config.web.sign_secret,
                        fsid,
                        &filename,
                        3600 * 24
                    );

                    FileLink {
                        filename,
                        download_url: signed_url,
                    }
                })
                .collect();

            info!("✅ 成功生成 {} 个签名链接", file_links.len());

            (
                StatusCode::OK,
                Json(ConvertResponse {
                    success: true,
                    links: Some(file_links),
                    error: None,
                }),
            )
        }
        Err(e) => {
            warn!("❌ 转换失败: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ConvertResponse {
                    success: false,
                    links: None,
                    error: Some(e.to_string()),
                }),
            )
        }
    }
}

fn generate_signed_link(sign_secret: &str, fsid: u64, filename: &str, ttl_secs: u64) -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let expires = now + ttl_secs;

    let data = format!("{fsid}:{expires}");

    let mut mac = Hmac::<Sha256>::new_from_slice(sign_secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    let result = mac.finalize().into_bytes();
    let sign = URL_SAFE_NO_PAD.encode(result);

    format!(
        "/d/download?fsid={}&sign={}&expires={}&filename={}",
        fsid,
        sign,
        expires,
        urlencoding::encode(filename)
    )
}

async fn download_handler(
    State(state): State<Arc<AppState>>,
    Query(params): Query<HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(fsid_str) = params.get("fsid") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(sign) = params.get("sign") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(expires_str) = params.get("expires") else {
        return StatusCode::BAD_REQUEST.into_response();
    };

    let fsid: u64 = match fsid_str.parse() {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let expires: u64 = match expires_str.parse() {
        Ok(v) => v,
        Err(_) => return StatusCode::BAD_REQUEST.into_response(),
    };

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    
    if now > expires {
        info!("❌ 链接已过期: fsid={}", fsid);
        return (StatusCode::UNAUTHORIZED, "链接已过期").into_response();
    }

    let data = format!("{fsid}:{expires}");
    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.web.sign_secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    let expected = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

    if &expected != sign {
        info!("❌ 签名验证失败: fsid={}", fsid);
        return (StatusCode::UNAUTHORIZED, "签名无效").into_response();
    }

    info!("✅ 签名验证通过: fsid={}", fsid);

    let access_token = match get_access_token(&state).await {
        Ok(token) => token,
        Err(e) => {
            warn!("❌ 获取 access_token 失败: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    match baidupcs::get_download_link_by_fsid_internal(&state, fsid, &access_token).await {
        Ok((_filename, real_url)) => {
            info!("🔁 302 重定向: fsid={}", fsid);
            Redirect::temporary(&real_url).into_response()
        }
        Err(e) => {
            warn!("❌ 获取直链失败: fsid={}, error={}", fsid, e);
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}

async fn get_access_token(state: &AppState) -> Result<String> {
    let opencfg = &state.config.baidu_open;
    if !opencfg.access_token.is_empty() {
        return Ok(opencfg.access_token.clone());
    }
    if !opencfg.refresh_token.is_empty() {
        return baidupcs::refresh_token(state).await;
    }
    Err(anyhow::anyhow!("未配置 access_token"))
}
