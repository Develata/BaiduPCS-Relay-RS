//! Web 服务器 - 通过浏览器获取下载直链

use anyhow::Result;
use axum::{
    body::Body,
    extract::{Path, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Json, Response, Redirect},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use baidu_direct_link::{baidupcs, config::Config, AppState};

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

// ✅ 新增：代理下载请求结构
#[derive(Deserialize)]
struct DownloadRequest {
    url: String,
    filename: String,
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

    info!("🚀 百度网盘直链 Web 服务启动中...");

    // 加载配置
    let config = Config::load("config.toml")?;
    info!("✅ 配置加载完成");

    // 显示访问密码
    info!("🔑 访问密码: {}", config.web.access_token);
    if config.web.access_token == "change-me" {
        info!("⚠️  警告: 使用默认密码，请在 config.toml 中修改 [web] access_token");
    }

    let state = Arc::new(AppState::new(config)?);
    info!("✅ HTTP Client 初始化完成");

    // ✅ 构建路由（添加代理下载端点）
    let app = Router::new()
        .route("/", get(index_handler))
        .route("/api/convert", post(convert_handler))
        .route("/api/download", post(proxy_download_handler)) // ✅ 代理下载（旧逻辑，保留）
        // ✅ 新增：本地签名直链重定向（参考 OpenList /AList 的 /d/...?...sign=）
        .route("/d/*path", get(signed_redirect_handler))
        .route("/health", get(health_handler))
        .with_state(state);

    // 启动服务器
    let addr = "0.0.0.0:5200";
    info!("🌐 Web 服务器启动: http://localhost:5200");
    info!("📖 使用方法:");
    info!("   1. 浏览器访问 http://localhost:5200");
    info!("   2. 输入访问密码");
    info!("   3. 输入分享链接和提取码");
    info!("   4. 点击开始转换");
    info!("   5. 可以复制链接或直接下载");

    let listener = tokio::net::TcpListener::bind(addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

/// 首页
async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../../templates/index.html"))
}

/// 健康检查
async fn health_handler() -> &'static str {
    "OK"
}

/// 转换处理
async fn convert_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ConvertRequest>,
) -> impl IntoResponse {
    info!("📥 收到转换请求: {}", req.link);

    // 验证访问密码
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
        info!("❌ 访问密码错误: 输入={}, 正确={}", req.token, correct_token);
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
        Ok(links) => {
            // 现在我们只使用文件名，根据保存路径生成本地签名直链 /d/...?...sign=
            let save_root = state.config.baidu.save_path.trim_end_matches('/').to_string();

            let file_links: Vec<FileLink> = links
                .into_iter()
                .map(|(filename, _url)| {
                    // 约定：转存后的路径为 save_root/filename
                    let mut path = save_root.clone();
                    path.push('/');
                    path.push_str(&filename);

                    let signed_url =
                        generate_signed_link(&state.config.web.sign_secret, &path, 3600 * 24);

                    FileLink {
                        filename,
                        download_url: signed_url,
                    }
                })
                .collect();

            info!("✅ 成功生成 {} 个本地签名直链", file_links.len());

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
            info!("❌ 转换失败: {}", e);
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

// ✅ 新增：代理下载处理器
async fn proxy_download_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<DownloadRequest>,
) -> Result<Response<Body>, StatusCode> {
    info!("📥 代理下载请求: {}", req.filename);

    // 使用正确的 User-Agent 和 Referer 请求
    let resp = match state
        .client
        .get(&req.url)
        .header("User-Agent", "pan.baidu.com")
        .header("Referer", "https://pan.baidu.com/disk/main")
        .header("Accept", "*/*")
        .send()
        .await
    {
        Ok(r) => r,
        Err(e) => {
            warn!("❌ 下载失败: {}", e);
            return Err(StatusCode::INTERNAL_SERVER_ERROR);
        }
    };

    if !resp.status().is_success() {
        warn!("❌ 下载失败: HTTP {}", resp.status());
        return Err(StatusCode::BAD_GATEWAY);
    }

    info!("✅ 开始传输文件: {}", req.filename);

    // 获取文件大小
    let content_length = resp.content_length();
    
    // 构建响应
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(
            header::CONTENT_DISPOSITION,
            format!(
                "attachment; filename=\"{}\"",
                urlencoding::encode(&req.filename)
            ),
        )
        .header(header::CONTENT_TYPE, "application/octet-stream")
        .header(header::CACHE_CONTROL, "no-cache");

    if let Some(len) = content_length {
        response = response.header(header::CONTENT_LENGTH, len);
        info!("📦 文件大小: {} bytes", len);
    }

    // 将响应体转换为流
    let body = Body::from_stream(resp.bytes_stream());
    
    Ok(response.body(body).unwrap())
}

/// 生成本地签名直链 `/d/<path>?sign=...&expires=...`
fn generate_signed_link(sign_secret: &str, pan_path: &str, ttl_secs: u64) -> String {
    // 过期时间（秒）
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let expires = now + ttl_secs;

    let data = format!("{pan_path}:{expires}");

    let mut mac = Hmac::<Sha256>::new_from_slice(sign_secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    let result = mac.finalize().into_bytes();
    let sign = URL_SAFE_NO_PAD.encode(result);

    // 注意：这里返回的是相对路径，前端在当前域名下访问即可
    format!("/d{}?sign={sign}&expires={expires}", pan_path)
}

/// 校验签名并根据网盘路径获取真实直链，然后 302 重定向
async fn signed_redirect_handler(
    State(state): State<Arc<AppState>>,
    Path(path): Path<String>,
    axum::extract::Query(params): axum::extract::Query<std::collections::HashMap<String, String>>,
) -> impl IntoResponse {
    let Some(sign) = params.get("sign") else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Some(expires_str) = params.get("expires") else {
        return StatusCode::BAD_REQUEST.into_response();
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
        // 链接已过期
        return (StatusCode::UNAUTHORIZED, "link expired").into_response();
    }

    // 还原出完整的网盘路径：我们在生成时已经包含了 save_path，所以这里直接使用
    let pan_path = format!("/{path}"); // Path extractor 已经去掉了前导 `/`

    // 重新计算签名
    let data = format!("{pan_path}:{expires}");
    let mut mac = Hmac::<Sha256>::new_from_slice(state.config.web.sign_secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    let expected = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

    if &expected != sign {
        // 签名不匹配
        return (StatusCode::UNAUTHORIZED, "invalid sign").into_response();
    }

    // 调用百度开放平台 API 获取真实直链，再 302 重定向
    match baidupcs::get_open_download_link(state.as_ref(), &pan_path).await {
        Ok(real_url) => {
            info!("🔁 OpenAPI 重定向到真实下载地址: {}", real_url);
            Redirect::temporary(&real_url).into_response()
        }
        Err(e) => {
            warn!("❌ OpenAPI 获取直链失败: {}", e);
            StatusCode::BAD_GATEWAY.into_response()
        }
    }
}
