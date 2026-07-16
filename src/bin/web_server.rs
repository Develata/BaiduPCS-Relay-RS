//! Web 服务器 - v1 直链服务

use anyhow::Result;
use axum::{
    extract::{Query, State},
    http::{HeaderMap, StatusCode},
    response::{Html, IntoResponse, Json, Redirect},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use baidu_direct_link::{
    baidupcs, config::Config, direct_link, signing::verify_signed_download, AppError, AppState,
};

#[derive(Debug, Deserialize)]
struct ConvertRequest {
    link: String,
    #[serde(default)]
    pwd: String,
    #[serde(default)]
    token: String,
}

#[derive(Debug, Serialize)]
struct ConvertResponse {
    success: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    items: Vec<FileLink>,
    #[serde(skip_serializing_if = "Option::is_none")]
    transfer_job: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<ApiError>,
}

#[derive(Debug, Serialize)]
struct FileLink {
    fsid: u64,
    filename: String,
    download_url: String,
    expires: u64,
}

#[derive(Debug, Serialize)]
struct ApiError {
    code: &'static str,
    message: String,
}

#[derive(Debug, Serialize)]
struct HealthResponse {
    status: &'static str,
    version: &'static str,
}

#[derive(Debug, Deserialize)]
struct ZipRequest {
    #[serde(default)]
    token: String,
}

#[derive(Debug, Deserialize)]
struct DownloadQuery {
    fsid: u64,
    expires: u64,
    filename: String,
    sign: String,
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

    let config_path = Config::config_path_from_env_or_arg(std::env::args().nth(1));
    let config = Config::load(&config_path)?;
    config.validate_web()?;
    info!("配置加载完成: {}", config_path);

    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(5200);
    let state = Arc::new(AppState::new(config)?);
    let app = create_router(state);

    let addr = format!("0.0.0.0:{port}");
    info!("Web 服务器启动: http://localhost:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/api/convert", post(convert_handler))
        .route("/api/zip", post(zip_handler))
        .route("/d/download", get(download_handler))
        .route("/health", get(health_handler))
        .with_state(state)
}

async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../../templates/index.html"))
}

async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok",
        version: baidu_direct_link::VERSION,
    })
}

async fn convert_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ConvertRequest>,
) -> impl IntoResponse {
    if let Err(err) = authorize(&state, &headers, &req.token) {
        return error_response(err);
    }

    info!("收到分享转直链请求: {}", req.link);
    let command = direct_link::ConvertShareCommand {
        link: req.link,
        pwd: req.pwd,
        ttl_secs: 24 * 3600,
    };

    match direct_link::convert_share_to_signed_downloads(&state, command).await {
        Ok(result) => {
            let items = result
                .items
                .into_iter()
                .map(|item| FileLink {
                    fsid: item.fsid,
                    filename: item.filename,
                    download_url: item.url,
                    expires: item.expires,
                })
                .collect();
            (
                StatusCode::OK,
                Json(ConvertResponse {
                    success: true,
                    items,
                    transfer_job: Some(result.transfer_job.id),
                    error: None,
                }),
            )
        }
        Err(err) => {
            warn!("分享转直链失败: {}", err);
            error_response(err)
        }
    }
}

async fn zip_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(req): Json<ZipRequest>,
) -> impl IntoResponse {
    if let Err(err) = authorize(&state, &headers, &req.token) {
        return error_response(err);
    }

    error_response(AppError::unsupported(
        "zip_unsupported",
        "v1 暂不支持服务器端 ZIP 打包；请使用 /api/convert 获取单文件签名下载链接",
    ))
}

async fn download_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<DownloadQuery>,
) -> impl IntoResponse {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();

    if let Err(err) = verify_signed_download(
        &state.config.web.sign_secret,
        query.fsid,
        &query.filename,
        query.expires,
        &query.sign,
        now,
    ) {
        return error_response(err).into_response();
    }

    let access_token = match baidupcs::download::get_or_refresh_access_token(&state).await {
        Ok(token) => token,
        Err(e) => {
            return error_response(AppError::config(
                "baidu_access_token_missing",
                e.to_string(),
            ))
            .into_response()
        }
    };
    let (target, access_token) =
        match get_download_target_with_retry(&state, query.fsid, access_token).await {
            Ok(result) => result,
            Err(err) => return error_response(err).into_response(),
        };
    let real_url =
        match resolve_dlink_redirect_with_retry(&state, &target.dlink, access_token).await {
            Ok(url) => url,
            Err(err) => return error_response(err).into_response(),
        };

    Redirect::temporary(&real_url).into_response()
}

async fn get_download_target_with_retry(
    state: &AppState,
    fsid: u64,
    access_token: String,
) -> Result<(baidupcs::DownloadTarget, String), AppError> {
    match baidupcs::download::get_download_target(state, fsid, &access_token).await {
        Ok(target) => Ok((target, access_token)),
        Err(error) => {
            let app_error = app_error_from_anyhow(error, "filemetas_failed");
            if app_error.code() != "baidu_access_token_invalid" {
                return Err(app_error);
            }

            let refreshed = baidupcs::download::refresh_access_token(state)
                .await
                .map_err(|e| {
                    AppError::config("baidu_access_token_refresh_failed", e.to_string())
                })?;
            let target = baidupcs::download::get_download_target(state, fsid, &refreshed)
                .await
                .map_err(|e| app_error_from_anyhow(e, "filemetas_failed"))?;
            Ok((target, refreshed))
        }
    }
}

async fn resolve_dlink_redirect_with_retry(
    state: &AppState,
    dlink: &str,
    access_token: String,
) -> Result<String, AppError> {
    match baidupcs::download::resolve_dlink_redirect(state, dlink, &access_token).await {
        Ok(url) => Ok(url),
        Err(error) => {
            let app_error = app_error_from_anyhow(error, "dlink_redirect_failed");
            if app_error.code() != "baidu_access_token_invalid" {
                return Err(app_error);
            }

            let refreshed = baidupcs::download::refresh_access_token(state)
                .await
                .map_err(|e| {
                    AppError::config("baidu_access_token_refresh_failed", e.to_string())
                })?;
            baidupcs::download::resolve_dlink_redirect(state, dlink, &refreshed)
                .await
                .map_err(|e| app_error_from_anyhow(e, "dlink_redirect_failed"))
        }
    }
}

fn app_error_from_anyhow(error: anyhow::Error, fallback_code: &'static str) -> AppError {
    match error.downcast::<AppError>() {
        Ok(app_error) => app_error,
        Err(error) => AppError::upstream(fallback_code, error.to_string()),
    }
}

fn authorize(state: &AppState, headers: &HeaderMap, body_token: &str) -> Result<(), AppError> {
    let token = bearer_token(headers)
        .or_else(|| (!body_token.is_empty()).then_some(body_token))
        .ok_or_else(|| AppError::unauthorized("missing_token", "缺少访问 token"))?;

    if constant_time_eq(token.as_bytes(), state.config.web.access_token.as_bytes()) {
        Ok(())
    } else {
        Err(AppError::unauthorized("invalid_token", "访问 token 错误"))
    }
}

fn bearer_token(headers: &HeaderMap) -> Option<&str> {
    let value = headers
        .get(axum::http::header::AUTHORIZATION)?
        .to_str()
        .ok()?;
    value.strip_prefix("Bearer ").filter(|s| !s.is_empty())
}

fn constant_time_eq(left: &[u8], right: &[u8]) -> bool {
    if left.len() != right.len() {
        return false;
    }
    let mut diff = 0u8;
    for (a, b) in left.iter().zip(right.iter()) {
        diff |= a ^ b;
    }
    diff == 0
}

fn error_response(error: AppError) -> (StatusCode, Json<ConvertResponse>) {
    let status = match error {
        AppError::BadRequest { .. } => StatusCode::BAD_REQUEST,
        AppError::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
        AppError::NotFound { .. } => StatusCode::NOT_FOUND,
        AppError::Unsupported { .. } => StatusCode::NOT_IMPLEMENTED,
        AppError::Config { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        AppError::Upstream { .. } => StatusCode::BAD_GATEWAY,
        AppError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    };
    (
        status,
        Json(ConvertResponse {
            success: false,
            items: Vec::new(),
            transfer_job: None,
            error: Some(ApiError {
                code: error.code(),
                message: error.message().to_string(),
            }),
        }),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::HeaderValue;

    #[test]
    fn extracts_bearer_token() {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer abc"),
        );
        assert_eq!(bearer_token(&headers), Some("abc"));
    }

    #[test]
    fn zip_error_is_501() {
        let (status, body) = error_response(AppError::unsupported("zip_unsupported", "no zip"));
        assert_eq!(status, StatusCode::NOT_IMPLEMENTED);
        assert_eq!(body.0.error.unwrap().code, "zip_unsupported");
    }

    #[test]
    fn preserves_app_error_from_anyhow() {
        let error: anyhow::Error =
            AppError::unauthorized("baidu_access_token_invalid", "token invalid").into();
        let app_error = app_error_from_anyhow(error, "fallback");
        assert_eq!(app_error.code(), "baidu_access_token_invalid");
    }
}
