//! Web 服务器 - v1 直链服务

use anyhow::{anyhow, Result};
use axum::{
    extract::{Query, State},
    http::{header, HeaderMap, HeaderValue, StatusCode},
    response::{Html, IntoResponse, Json, Response},
    routing::{get, post},
    Router,
};
use serde::{Deserialize, Serialize};
use std::io::{Read, Write};
use std::net::{Ipv4Addr, SocketAddr, SocketAddrV4, TcpStream};
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use tracing::{info, warn};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use baidu_direct_link::{
    baidupcs,
    config::{BaiduOpenConfig, Config},
    direct_link,
    signing::verify_signed_download,
    AppError, AppState,
};

const OAUTH_STATE_TTL_SECS: u64 = 10 * 60;

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

#[derive(Deserialize)]
struct OAuthCallbackQuery {
    code: Option<String>,
    state: Option<String>,
    error: Option<String>,
}

#[derive(Serialize)]
struct OAuthStartResponse {
    success: bool,
    authorize_url: String,
    redirect_uri: String,
    mode: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    flow_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct OAuthStatusResponse {
    success: bool,
    configured: bool,
    authorized: bool,
    token_available: bool,
    has_refresh_token: bool,
    client_id: String,
    client_secret_configured: bool,
    redirect_uri: String,
    mode: &'static str,
    expires_at: Option<u64>,
    scope: String,
}

#[derive(Deserialize)]
struct OAuthExchangeRequest {
    flow_id: String,
    code: String,
}

#[derive(Serialize)]
struct OAuthExchangeResponse {
    success: bool,
    expires_at: u64,
    has_refresh_token: bool,
}

#[derive(Serialize)]
struct OAuthCredentialsResponse {
    success: bool,
    access_token: String,
    refresh_token: String,
    client_id: String,
    client_secret_configured: bool,
    expires_at: u64,
    scope: String,
}

#[derive(Debug, Serialize)]
struct ApiFailure {
    success: bool,
    error: ApiError,
}

#[tokio::main]
async fn main() -> Result<()> {
    if std::env::args().nth(1).as_deref() == Some("--healthcheck") {
        return run_healthcheck();
    }

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

    let port = configured_port();
    let state = Arc::new(AppState::new(config)?);
    let app = create_router(state);

    let addr = format!("0.0.0.0:{port}");
    info!("Web 服务器启动: http://localhost:{port}");
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;
    Ok(())
}

fn configured_port() -> u16 {
    std::env::var("PORT")
        .ok()
        .and_then(|value| value.parse::<u16>().ok())
        .unwrap_or(5200)
}

fn run_healthcheck() -> Result<()> {
    let address = SocketAddr::V4(SocketAddrV4::new(Ipv4Addr::LOCALHOST, configured_port()));
    let timeout = Duration::from_secs(2);
    let mut stream = TcpStream::connect_timeout(&address, timeout)?;
    stream.set_read_timeout(Some(timeout))?;
    stream.set_write_timeout(Some(timeout))?;
    stream.write_all(b"GET /health HTTP/1.1\r\nHost: 127.0.0.1\r\nConnection: close\r\n\r\n")?;

    let mut response = String::new();
    (&mut stream).take(4096).read_to_string(&mut response)?;
    if is_healthy_response(&response) {
        Ok(())
    } else {
        Err(anyhow!("health endpoint did not return status=ok"))
    }
}

fn is_healthy_response(response: &str) -> bool {
    response.starts_with("HTTP/1.1 200") && response.contains("\"status\":\"ok\"")
}

fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/", get(index_handler))
        .route("/api/convert", post(convert_handler))
        .route("/api/zip", post(zip_handler))
        .route("/api/oauth/start", post(oauth_start_handler))
        .route("/api/oauth/exchange", post(oauth_exchange_handler))
        .route("/api/oauth/status", get(oauth_status_handler))
        .route("/api/oauth/credentials", get(oauth_credentials_handler))
        .route("/oauth/callback", get(oauth_callback_handler))
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

async fn oauth_start_handler(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Err(error) = authorize(&state, &headers, "") {
        return api_error_response(error);
    }

    let oauth_state = match state.issue_oauth_state(unix_timestamp(), OAUTH_STATE_TTL_SECS) {
        Ok(value) => value,
        Err(error) => {
            return api_error_response(AppError::internal("oauth_state_failed", error.to_string()))
        }
    };
    let authorize_url =
        match baidupcs::openapi::authorization_url(&state.config.baidu_open, &oauth_state) {
            Ok(url) => url,
            Err(error) => {
                return api_error_response(AppError::config(
                    "oauth_config_invalid",
                    error.to_string(),
                ))
            }
        };

    no_store_json(OAuthStartResponse {
        success: true,
        authorize_url,
        redirect_uri: state.config.baidu_open.redirect_uri.clone(),
        mode: oauth_mode(&state.config.baidu_open),
        flow_id: baidupcs::openapi::is_oob_redirect(&state.config.baidu_open)
            .then_some(oauth_state),
    })
}

async fn oauth_status_handler(State(state): State<Arc<AppState>>, headers: HeaderMap) -> Response {
    if let Err(error) = authorize(&state, &headers, "") {
        return api_error_response(error);
    }

    let config = &state.config.baidu_open;
    let tokens = state.oauth_tokens();
    let response = OAuthStatusResponse {
        success: true,
        configured: !config.client_id.is_empty()
            && !config.client_secret.is_empty()
            && !config.redirect_uri.is_empty(),
        authorized: tokens.is_some(),
        token_available: state.cached_access_token().is_some()
            || !config.access_token.is_empty()
            || state.current_refresh_token().is_some(),
        has_refresh_token: state.current_refresh_token().is_some(),
        client_id: config.client_id.clone(),
        client_secret_configured: !config.client_secret.is_empty(),
        redirect_uri: config.redirect_uri.clone(),
        mode: oauth_mode(config),
        expires_at: tokens.as_ref().map(|value| value.expires_at),
        scope: tokens
            .as_ref()
            .map(|value| value.scope.clone())
            .unwrap_or_default(),
    };
    no_store_json(response)
}

async fn oauth_exchange_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
    Json(request): Json<OAuthExchangeRequest>,
) -> Response {
    if let Err(error) = authorize(&state, &headers, "") {
        return api_error_response(error);
    }
    if !baidupcs::openapi::is_oob_redirect(&state.config.baidu_open) {
        return api_error_response(AppError::bad_request(
            "oauth_mode_mismatch",
            "当前服务使用 HTTP callback 模式，无需手工提交授权码",
        ));
    }

    let flow_id = request.flow_id.trim();
    let code = request.code.trim();
    if flow_id.is_empty() || code.is_empty() {
        return api_error_response(AppError::bad_request(
            "oauth_exchange_invalid",
            "flow_id 和 authorization code 不能为空",
        ));
    }
    if !state.consume_oauth_state(flow_id, unix_timestamp()) {
        return api_error_response(AppError::unauthorized(
            "oauth_state_invalid",
            "OAuth flow_id 无效、已过期或已被使用，请重新发起授权",
        ));
    }

    match exchange_oauth_code_and_cache(&state, code).await {
        Ok(tokens) => no_store_json(OAuthExchangeResponse {
            success: true,
            expires_at: tokens.expires_at,
            has_refresh_token: !tokens.refresh_token.is_empty(),
        }),
        Err(error) => api_error_response(error),
    }
}

async fn oauth_credentials_handler(
    State(state): State<Arc<AppState>>,
    headers: HeaderMap,
) -> Response {
    if let Err(error) = authorize(&state, &headers, "") {
        return api_error_response(error);
    }
    let Some(tokens) = state.oauth_tokens() else {
        return api_error_response(AppError::not_found(
            "oauth_tokens_missing",
            "当前进程尚未通过前端完成百度 OAuth 授权",
        ));
    };

    no_store_json(OAuthCredentialsResponse {
        success: true,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        client_id: state.config.baidu_open.client_id.clone(),
        client_secret_configured: !state.config.baidu_open.client_secret.is_empty(),
        expires_at: tokens.expires_at,
        scope: tokens.scope,
    })
}

async fn oauth_callback_handler(
    State(state): State<Arc<AppState>>,
    Query(query): Query<OAuthCallbackQuery>,
) -> Response {
    if baidupcs::openapi::is_oob_redirect(&state.config.baidu_open) {
        return oauth_callback_page(StatusCode::BAD_REQUEST, false);
    }
    let Some(returned_state) = query.state.as_deref() else {
        return oauth_callback_page(StatusCode::BAD_REQUEST, false);
    };
    if !state.consume_oauth_state(returned_state, unix_timestamp()) {
        return oauth_callback_page(StatusCode::BAD_REQUEST, false);
    }
    if query.error.is_some() {
        warn!("百度 OAuth 授权被取消或拒绝");
        return oauth_callback_page(StatusCode::BAD_REQUEST, false);
    }
    let Some(code) = query.code.as_deref().filter(|value| !value.is_empty()) else {
        return oauth_callback_page(StatusCode::BAD_REQUEST, false);
    };

    match exchange_oauth_code_and_cache(&state, code).await {
        Ok(tokens) => {
            info!(
                expires_at = tokens.expires_at,
                "百度 OAuth 授权完成，token 已写入内存 provider"
            );
            oauth_callback_page(StatusCode::OK, true)
        }
        Err(error) => {
            warn!(code = error.code(), "百度 OAuth callback 处理失败");
            oauth_callback_page(status_for_error(&error), false)
        }
    }
}

async fn exchange_oauth_code_and_cache(
    state: &AppState,
    code: &str,
) -> Result<baidupcs::OAuthTokenSet, AppError> {
    let tokens = baidupcs::openapi::exchange_authorization_code(state, code)
        .await
        .map_err(|_| {
            AppError::upstream(
                "oauth_exchange_failed",
                "百度 OAuth 授权码交换失败，请重新发起授权",
            )
        })?;
    state.cache_oauth_tokens(tokens.clone()).map_err(|_| {
        AppError::internal(
            "oauth_token_cache_failed",
            "OAuth token 无法写入当前服务进程",
        )
    })?;
    Ok(tokens)
}

fn oauth_mode(config: &BaiduOpenConfig) -> &'static str {
    if baidupcs::openapi::is_oob_redirect(config) {
        "oob"
    } else {
        "callback"
    }
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
    let now = unix_timestamp();

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

    match found_redirect(&real_url) {
        Ok(response) => response,
        Err(err) => error_response(err).into_response(),
    }
}

fn found_redirect(location: &str) -> Result<Response, AppError> {
    let location = HeaderValue::from_str(location).map_err(|_| {
        AppError::upstream("baidu_dlink_invalid", "百度下载接口返回了无效的重定向地址")
    })?;
    let mut response = StatusCode::FOUND.into_response();
    response.headers_mut().insert(header::LOCATION, location);
    Ok(response)
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

fn unix_timestamp() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn no_store_json<T: Serialize>(value: T) -> Response {
    ([(header::CACHE_CONTROL, "no-store")], Json(value)).into_response()
}

fn oauth_callback_page(status: StatusCode, success: bool) -> Response {
    let body = if success {
        r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>授权完成</title><style>body{font-family:system-ui,sans-serif;margin:0;display:grid;place-items:center;min-height:100vh;background:#f6f7f8;color:#17202a}.box{border:1px solid #d9dee5;background:#fff;padding:28px;max-width:360px}h1{font-size:20px;margin:0 0 8px}p{margin:0;color:#56616f}</style></head><body><main class="box"><h1>授权完成</h1><p>令牌已安全写入本地服务内存。</p></main><script>if(window.opener){window.opener.postMessage({type:'baidu-oauth-complete',success:true},'*')}setTimeout(()=>window.close(),1200)</script></body></html>"#
    } else {
        r#"<!doctype html><html lang="zh-CN"><head><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1"><title>授权失败</title><style>body{font-family:system-ui,sans-serif;margin:0;display:grid;place-items:center;min-height:100vh;background:#f6f7f8;color:#17202a}.box{border:1px solid #d9dee5;background:#fff;padding:28px;max-width:360px}h1{font-size:20px;margin:0 0 8px}p{margin:0;color:#8f2f2f}</style></head><body><main class="box"><h1>授权未完成</h1><p>请关闭窗口后重新发起授权。</p></main><script>if(window.opener){window.opener.postMessage({type:'baidu-oauth-complete',success:false},'*')}</script></body></html>"#
    };
    (
        status,
        [
            (header::CACHE_CONTROL, "no-store"),
            (header::PRAGMA, "no-cache"),
            (header::X_FRAME_OPTIONS, "DENY"),
            (
                header::CONTENT_SECURITY_POLICY,
                "default-src 'none'; style-src 'unsafe-inline'; script-src 'unsafe-inline'",
            ),
        ],
        Html(body),
    )
        .into_response()
}

fn api_error_response(error: AppError) -> Response {
    let status = status_for_error(&error);
    (
        status,
        [(header::CACHE_CONTROL, "no-store")],
        Json(ApiFailure {
            success: false,
            error: ApiError {
                code: error.code(),
                message: error.message().to_string(),
            },
        }),
    )
        .into_response()
}

fn status_for_error(error: &AppError) -> StatusCode {
    match error {
        AppError::BadRequest { .. } => StatusCode::BAD_REQUEST,
        AppError::Unauthorized { .. } => StatusCode::UNAUTHORIZED,
        AppError::NotFound { .. } => StatusCode::NOT_FOUND,
        AppError::Unsupported { .. } => StatusCode::NOT_IMPLEMENTED,
        AppError::Config { .. } => StatusCode::INTERNAL_SERVER_ERROR,
        AppError::Upstream { .. } => StatusCode::BAD_GATEWAY,
        AppError::Internal { .. } => StatusCode::INTERNAL_SERVER_ERROR,
    }
}

fn error_response(error: AppError) -> (StatusCode, Json<ConvertResponse>) {
    let status = status_for_error(&error);
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
    use axum::body::to_bytes;
    use axum::http::HeaderValue;

    fn test_app_state(redirect_uri: &str) -> Arc<AppState> {
        let mut config = Config::default();
        config.baidu.cookie_bduss = "b".repeat(80);
        config.baidu.cookie_stoken = "s".repeat(40);
        config.web.access_token = "test-service-token".to_string();
        config.web.sign_secret = "test-sign-secret".to_string();
        config.baidu_open.client_id = "public-client-id".to_string();
        config.baidu_open.client_secret = "private-client-secret".to_string();
        config.baidu_open.redirect_uri = redirect_uri.to_string();
        Arc::new(AppState::new(config).unwrap())
    }

    fn test_auth_headers() -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert(
            axum::http::header::AUTHORIZATION,
            HeaderValue::from_static("Bearer test-service-token"),
        );
        headers
    }

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

    #[test]
    fn oauth_status_never_serializes_client_secret() {
        let response = OAuthStatusResponse {
            success: true,
            configured: true,
            authorized: false,
            token_available: false,
            has_refresh_token: false,
            client_id: "public-client-id".to_string(),
            client_secret_configured: true,
            redirect_uri: "http://127.0.0.1:5200/oauth/callback".to_string(),
            mode: "callback",
            expires_at: None,
            scope: String::new(),
        };

        let json = serde_json::to_string(&response).unwrap();
        assert!(json.contains("public-client-id"));
        assert!(!json.contains("client-secret"));
    }

    #[test]
    fn reports_oob_mode_only_for_exact_oob_redirect() {
        let mut config = BaiduOpenConfig {
            redirect_uri: "oob".to_string(),
            ..BaiduOpenConfig::default()
        };
        assert_eq!(oauth_mode(&config), "oob");

        config.redirect_uri = "http://127.0.0.1:5200/oauth/callback".to_string();
        assert_eq!(oauth_mode(&config), "callback");
    }

    #[tokio::test]
    async fn oob_start_returns_one_time_flow_id_without_secret() {
        let response = oauth_start_handler(State(test_app_state("oob")), test_auth_headers()).await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["mode"], "oob");
        assert_eq!(json["redirect_uri"], "oob");
        assert!(json["flow_id"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(json["authorize_url"]
            .as_str()
            .is_some_and(|value| value.contains("redirect_uri=oob")));
        assert!(!String::from_utf8_lossy(&body).contains("private-client-secret"));
    }

    #[tokio::test]
    async fn oob_exchange_rejects_unknown_flow_before_contacting_baidu() {
        let response = oauth_exchange_handler(
            State(test_app_state("oob")),
            test_auth_headers(),
            Json(OAuthExchangeRequest {
                flow_id: "unknown-flow".to_string(),
                code: "unused-code".to_string(),
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);

        let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
        let json: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(json["error"]["code"], "oauth_state_invalid");
        assert!(!String::from_utf8_lossy(&body).contains("unused-code"));
    }

    #[tokio::test]
    async fn callback_endpoint_is_disabled_in_oob_mode() {
        let response = oauth_callback_handler(
            State(test_app_state("oob")),
            Query(OAuthCallbackQuery {
                code: Some("unused-code".to_string()),
                state: Some("unused-state".to_string()),
                error: None,
            }),
        )
        .await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    }

    #[test]
    fn callback_page_does_not_contain_token_fields() {
        let response = oauth_callback_page(StatusCode::OK, true);
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers().get(header::CACHE_CONTROL).unwrap(),
            "no-store"
        );
    }

    #[test]
    fn download_redirect_uses_http_302() {
        let response = found_redirect("https://example.com/download").unwrap();
        assert_eq!(response.status(), StatusCode::FOUND);
        assert_eq!(
            response.headers().get(header::LOCATION).unwrap(),
            "https://example.com/download"
        );
    }

    #[test]
    fn recognizes_healthcheck_response() {
        assert!(is_healthy_response(
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\n\r\n{\"status\":\"ok\"}"
        ));
        assert!(!is_healthy_response(
            "HTTP/1.1 503 Service Unavailable\r\n\r\n{\"status\":\"ok\"}"
        ));
    }
}
