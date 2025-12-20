//! Web 服务器模块

use anyhow::{anyhow, Result};
use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, Json, Redirect},
    routing::{get, post},
    Router,
};
use axum_extra::extract::CookieJar;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tracing::{error, info, warn};
use url::Url;

use crate::{baidupcs, AppState};

// 缓存 HTML 模板（避免每次都读取）
static HTML_TEMPLATE: &str = include_str!("../templates/index.html");
static LOGIN_TEMPLATE: &str = include_str!("../templates/login.html");

// 认证 token（简单实现，生产环境应使用更安全的方式）
const AUTH_TOKEN: &str = "baidupcs_auth_token";
const AUTH_COOKIE_NAME: &str = "baidupcs_auth";

#[derive(Debug, Deserialize)]
pub struct TransferRequest {
    pub share_url: String,
    #[serde(default)]
    pub pwd: String,
}

#[derive(Debug, Serialize)]
pub struct TransferResponse {
    pub success: bool,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub file_count: Option<usize>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub save_path: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct LoginRequest {
    pub password: String,
}

#[derive(Debug, Serialize)]
pub struct LoginResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct HealthResponse {
    pub status: String,
    pub version: &'static str,
}

/// 检查是否已认证
fn is_authenticated(cookies: &CookieJar) -> bool {
    cookies
        .get(AUTH_COOKIE_NAME)
        .map(|c| c.value() == AUTH_TOKEN)
        .unwrap_or(false)
}

/// 验证密码
/// 注意：这里使用简单的字符串比较，适用于单用户场景
/// 如需多用户或更高安全性，建议使用密码哈希（如 bcrypt）
fn verify_password(state: &AppState, password: &str) -> bool {
    !state.config.web.password.is_empty() && state.config.web.password == password
}

/// 登录页面
pub async fn login_page_handler() -> Html<&'static str> {
    Html(LOGIN_TEMPLATE)
}

/// 登录 API
pub async fn login_handler(
    State(state): State<Arc<AppState>>,
    mut cookies: CookieJar,
    Json(req): Json<LoginRequest>,
) -> (CookieJar, Json<LoginResponse>) {
    // 如果未设置密码，允许直接访问
    if state.config.web.password.is_empty() {
        return (
            cookies,
            Json(LoginResponse {
                success: true,
                message: "密码未设置，无需登录".to_string(),
            }),
        );
    }

    if verify_password(&state, &req.password) {
        info!("✅ 登录成功");
        // 设置认证 cookie（30天过期，HttpOnly 防止 XSS）
        let mut cookie = axum_extra::extract::cookie::Cookie::new(AUTH_COOKIE_NAME, AUTH_TOKEN);
        cookie.set_path("/");
        cookie.set_max_age(time::Duration::days(30));
        cookie.set_http_only(true); // 防止 JavaScript 访问，提高安全性
                                    // 注意：Secure 标志仅在 HTTPS 环境下启用，HTTP 环境下不设置
        cookies = cookies.add(cookie);
        (
            cookies,
            Json(LoginResponse {
                success: true,
                message: "登录成功".to_string(),
            }),
        )
    } else {
        warn!("❌ 登录失败：密码错误");
        (
            cookies,
            Json(LoginResponse {
                success: false,
                message: "密码错误".to_string(),
            }),
        )
    }
}

/// 登出 API：清除认证 cookie
pub async fn logout_handler(mut cookies: CookieJar) -> (CookieJar, Json<LoginResponse>) {
    // 通过设置 Max-Age = 0 来删除 cookie（并保持 Path 与 HttpOnly 设置以确保正确移除）
    let mut cookie = axum_extra::extract::cookie::Cookie::new(AUTH_COOKIE_NAME, "");
    cookie.set_path("/");
    cookie.set_max_age(time::Duration::seconds(0));
    cookie.set_http_only(true);
    cookies = cookies.remove(cookie);

    (
        cookies,
        Json(LoginResponse {
            success: true,
            message: "已退出登录".to_string(),
        }),
    )
}

/// 首页 - 返回 HTML 页面（需要认证）
pub async fn index_handler(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
) -> Result<Html<&'static str>, Redirect> {
    // 如果未设置密码，直接返回页面
    if state.config.web.password.is_empty() {
        return Ok(Html(HTML_TEMPLATE));
    }

    // 检查认证
    if !is_authenticated(&cookies) {
        return Err(Redirect::to("/login"));
    }

    Ok(Html(HTML_TEMPLATE))
}

/// 健康检查端点（不需要认证）
pub async fn health_handler() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "ok".to_string(),
        version: crate::VERSION,
    })
}

/// 转存 API 端点（需要认证）
pub async fn transfer_handler(
    State(state): State<Arc<AppState>>,
    cookies: CookieJar,
    Json(req): Json<TransferRequest>,
) -> Result<Json<TransferResponse>, StatusCode> {
    // 检查认证（如果设置了密码）
    if !state.config.web.password.is_empty() && !is_authenticated(&cookies) {
        return Ok(Json(TransferResponse {
            success: false,
            message: "未登录，请先登录".to_string(),
            file_count: None,
            save_path: None,
        }));
    }

    info!("📥 收到转存请求: {}", req.share_url);

    // 验证输入
    if let Err(e) = validate_share_url(&req.share_url) {
        warn!("❌ 分享链接验证失败: {}", e);
        return Ok(Json(TransferResponse {
            success: false,
            message: format!("分享链接验证失败: {}", e),
            file_count: None,
            save_path: None,
        }));
    }

    // 验证提取码
    if let Err(e) = validate_password(&req.pwd) {
        warn!("❌ 提取码验证失败: {}", e);
        return Ok(Json(TransferResponse {
            success: false,
            message: format!("提取码验证失败: {}", e),
            file_count: None,
            save_path: None,
        }));
    }

    // 提取 surl
    let surl = match baidupcs::extract_surl(&req.share_url) {
        Some(s) => s,
        None => {
            error!("❌ 无法从链接中提取 surl: {}", req.share_url);
            return Ok(Json(TransferResponse {
                success: false,
                message: format!("无效的分享链接格式，无法提取分享码: {}", req.share_url),
                file_count: None,
                save_path: None,
            }));
        }
    };

    // 获取分享信息
    let info = match baidupcs::get_share_info(state.as_ref(), &req.share_url, &surl, &req.pwd).await
    {
        Ok(info) => info,
        Err(e) => {
            error!("❌ 获取分享信息失败: {}", e);
            // 提供更友好的错误消息
            let error_msg = e.to_string();
            let user_friendly_msg = if error_msg.contains("提取码") || error_msg.contains("密码")
            {
                "提取码错误，请检查后重试".to_string()
            } else if error_msg.contains("失效") || error_msg.contains("过期") {
                "分享链接已失效或过期".to_string()
            } else if error_msg.contains("Cookie") || error_msg.contains("登录") {
                "Cookie 失效，请检查配置文件中的 BDUSS 和 STOKEN".to_string()
            } else {
                error_msg
            };
            return Ok(Json(TransferResponse {
                success: false,
                message: format!("获取分享信息失败: {}", user_friendly_msg),
                file_count: None,
                save_path: None,
            }));
        }
    };

    info!("📦 获取到 {} 个文件，开始转存...", info.fs_ids.len());

    // 执行转存
    match baidupcs::transfer_files(
        state.as_ref(),
        &info.shareid,
        &info.uk,
        &info.fs_ids,
        &info.bdstoken,
        &surl,
    )
    .await
    {
        Ok(_) => {
            info!("✅ 转存成功");
            Ok(Json(TransferResponse {
                success: true,
                message: format!(
                    "转存成功！{} 个文件已保存至: {}",
                    info.fs_ids.len(),
                    state.config.baidu.save_path
                ),
                file_count: Some(info.fs_ids.len()),
                save_path: Some(state.config.baidu.save_path.clone()),
            }))
        }
        Err(e) => {
            error!("❌ 转存失败: {}", e);
            // 提供更友好的错误消息
            let error_msg = e.to_string();
            let user_friendly_msg =
                if error_msg.contains("路径不存在") || error_msg.contains("路径错误") {
                    format!(
                        "保存路径不存在: {}，请在百度网盘中先创建该文件夹",
                        state.config.baidu.save_path
                    )
                } else if error_msg.contains("Cookie") || error_msg.contains("登录") {
                    "Cookie 失效，请检查配置文件中的 BDUSS 和 STOKEN".to_string()
                } else if error_msg.contains("权限") {
                    "权限不足，可能是分享链接已失效或设置了权限限制".to_string()
                } else {
                    error_msg
                };
            Ok(Json(TransferResponse {
                success: false,
                message: format!("转存失败: {}", user_friendly_msg),
                file_count: Some(info.fs_ids.len()),
                save_path: Some(state.config.baidu.save_path.clone()),
            }))
        }
    }
}

/// 验证分享链接格式
pub fn validate_share_url(url: &str) -> Result<()> {
    if url.is_empty() {
        return Err(anyhow!("分享链接不能为空"));
    }

    // 检查是否是有效的 URL
    let parsed = Url::parse(url).map_err(|_| anyhow!("无效的 URL 格式"))?;

    // 检查是否是百度网盘链接
    if !parsed.host_str().map_or(false, |h| h.contains("baidu.com")) {
        return Err(anyhow!("必须是百度网盘分享链接"));
    }

    // 检查路径是否包含 /s/
    if !parsed.path().contains("/s/") {
        return Err(anyhow!("无效的分享链接格式，应包含 /s/"));
    }

    Ok(())
}

/// 验证提取码格式
/// 百度网盘提取码必须是4位字符（可以为空）
pub fn validate_password(pwd: &str) -> Result<()> {
    if !pwd.is_empty() && pwd.len() != 4 {
        return Err(anyhow!("提取码必须是4位字符"));
    }
    Ok(())
}

/// 创建 Web 路由
pub fn create_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/login", get(login_page_handler))
        .route("/api/login", post(login_handler))
        .route("/api/logout", post(logout_handler))
        .route("/", get(index_handler))
        .route("/health", get(health_handler))
        .route("/api/transfer", post(transfer_handler))
        .with_state(state)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_share_url_valid() {
        let valid_urls = vec![
            "https://pan.baidu.com/s/1xxxxx",
            "https://pan.baidu.com/s/1abc123",
            "http://pan.baidu.com/s/1test",
        ];

        for url in valid_urls {
            assert!(
                validate_share_url(url).is_ok(),
                "URL should be valid: {}",
                url
            );
        }
    }

    #[test]
    fn test_validate_share_url_invalid() {
        let invalid_urls = vec![
            "",
            "not-a-url",
            "https://example.com/s/1xxxxx",
            "https://pan.baidu.com/other/path",
            "https://google.com/s/1xxxxx",
        ];

        for url in invalid_urls {
            assert!(
                validate_share_url(url).is_err(),
                "URL should be invalid: {}",
                url
            );
        }
    }

    #[test]
    fn test_validate_password_valid() {
        let valid_passwords = vec!["", "1234", "abcd", "A1B2", "test"];

        for pwd in valid_passwords {
            assert!(
                validate_password(pwd).is_ok(),
                "Password should be valid: {}",
                pwd
            );
        }
    }

    #[test]
    fn test_validate_password_invalid() {
        let invalid_passwords = vec!["123", "12345", "abc", "12"];

        for pwd in invalid_passwords {
            assert!(
                validate_password(pwd).is_err(),
                "Password should be invalid: {}",
                pwd
            );
        }
    }

    #[test]
    fn test_health_response() {
        let response = HealthResponse {
            status: "ok".to_string(),
            version: "1.0.0",
        };
        assert_eq!(response.status, "ok");
        assert_eq!(response.version, "1.0.0");
    }
}
