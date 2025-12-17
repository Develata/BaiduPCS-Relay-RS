//! 百度网盘开放平台（OpenAPI）相关逻辑
//!
//! 参考 OpenList/AList 的做法，通过 access_token 调用百度开放平台接口获取下载链接。

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tracing::{debug, info, warn};
use std::fs;
use reqwest::header;

use crate::{AppState, config::Config};

/// 使用百度开放平台 access_token + 网盘路径获取下载直链
///
/// 优先使用 config.toml 中的 [baidu_open].access_token；
/// 如果为空但提供了 refresh_token，则会自动尝试通过 refresh_token 刷新出新的 access_token。
pub async fn get_open_download_link(state: &AppState, path: &str) -> Result<String> {
    let open_cfg = &state.config.baidu_open;

    let access_token = if !open_cfg.access_token.is_empty() {
        open_cfg.access_token.clone()
    } else if !open_cfg.refresh_token.is_empty() {
        info!("baidu_open.access_token 为空，尝试使用 refresh_token 刷新 access_token...");
        let token = refresh_access_token_with_refresh_token(state).await?;
        info!(
            "✅ 成功刷新 access_token（长度={}），建议将其写回 config.toml 的 [baidu_open].access_token 中",
            token.len()
        );
        token
    } else {
        return Err(anyhow!(
            "未配置百度开放平台凭据，请在 config.toml 的 [baidu_open] 中填写 access_token 或 refresh_token"
        ));
    };

    // 尝试调用百度开放平台的下载接口
    // 参考：pan.baidu.com 开放平台 xpan/file 接口
    let url = format!(
        "https://pan.baidu.com/rest/2.0/xpan/file?method=download&path={}&access_token={}",
        urlencoding::encode(path),
        access_token
    );

    debug!("📡 Baidu OpenAPI download: {}", url);

    let resp = state
        .client
        .get(&url)
        // 按 OpenList/官方要求，大文件下载需要带上 pan.baidu.com UA
        .header(header::USER_AGENT, "pan.baidu.com")
        .send()
        .await?;
    let status = resp.status();

    // 情况 1：OpenAPI 直接返回重定向（302/301 等），Location 即为真实下载地址
    if status.is_redirection() {
        if let Some(loc) = resp.headers().get(header::LOCATION) {
            let url = loc.to_str().unwrap_or_default().to_string();
            if !url.is_empty() {
                info!("✅ 从 OpenAPI 重定向 Location 中获取到下载链接");
                return Ok(url);
            }
        }
        let text = resp.text().await.unwrap_or_default();
        warn!("OpenAPI 返回重定向但未包含有效的 Location 头，body={}", text);
        return Err(anyhow!(
            "OpenAPI 重定向响应缺少 Location 头: status={}, body={}",
            status,
            text
        ));
    }

    // 情况 2：返回 2xx 或 4xx/5xx，需要读取 body 内容（JSON/错误信息）
    let text = resp.text().await?;
    debug!(
        "📨 OpenAPI 响应 (status={}): {}",
        status,
        &text[..text.len().min(300)]
    );

    // 非 2xx 直接报错
    if !status.is_success() {
        return Err(anyhow!(
            "OpenAPI HTTP 失败: status={}, body={}",
            status,
            text
        ));
    }

    #[derive(Deserialize)]
    struct UrlItem {
        #[serde(default)]
        dlink: String,
        #[serde(default)]
        url: String,
    }

    #[derive(Deserialize)]
    struct OpenDownloadResponse {
        #[serde(default)]
        errno: i32,
        #[serde(default)]
        error_code: i32,
        #[serde(default)]
        error_msg: String,
        #[serde(default)]
        dlink: String,
        #[serde(default)]
        list: Vec<UrlItem>,
        #[serde(default)]
        urls: Vec<UrlItem>,
    }

    let parsed: OpenDownloadResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 OpenAPI 响应失败: {}, body={}", e, text))?;

    let code = if parsed.errno != 0 {
        parsed.errno
    } else {
        parsed.error_code
    };

    if code != 0 {
        return Err(anyhow!(
            "OpenAPI 下载接口返回错误: errno={}, error_msg={}",
            code,
            parsed.error_msg
        ));
    }

    // 尝试从多个字段中提取实际下载 URL
    if !parsed.dlink.is_empty() {
        info!("✅ 从 OpenAPI 响应中获取到 dlink");
        return Ok(parsed.dlink);
    }

    if let Some(item) = parsed.list.first().or_else(|| parsed.urls.first()) {
        if !item.dlink.is_empty() {
            info!("✅ 从 OpenAPI list/urls.dlink 中获取到下载链接");
            return Ok(item.dlink.clone());
        }
        if !item.url.is_empty() {
            info!("✅ 从 OpenAPI list/urls.url 中获取到下载链接");
            return Ok(item.url.clone());
        }
    }

    Err(anyhow!("OpenAPI 未返回可用的下载链接"))
}

/// 使用 refresh_token 调用百度 OAuth2 接口刷新 access_token
async fn refresh_access_token_with_refresh_token(state: &AppState) -> Result<String> {
    let open_cfg = &state.config.baidu_open;

    if open_cfg.client_id.is_empty() || open_cfg.client_secret.is_empty() {
        return Err(anyhow!(
            "baidu_open.client_id 或 client_secret 为空，无法使用 refresh_token 刷新 access_token"
        ));
    }
    if open_cfg.refresh_token.is_empty() {
        return Err(anyhow!(
            "baidu_open.refresh_token 为空，无法刷新 access_token"
        ));
    }

    let url = format!(
        "https://openapi.baidu.com/oauth/2.0/token?grant_type=refresh_token&refresh_token={}&client_id={}&client_secret={}",
        urlencoding::encode(&open_cfg.refresh_token),
        urlencoding::encode(&open_cfg.client_id),
        urlencoding::encode(&open_cfg.client_secret),
    );

    debug!("🔐 刷新 access_token: {}", url);

    let resp = state.client.get(&url).send().await?;
    let status = resp.status();
    let text = resp.text().await?;

    debug!(
        "🔐 refresh_token 响应 (status={}): {}",
        status,
        &text[..text.len().min(300)]
    );

    #[derive(Deserialize)]
    struct TokenResponse {
        #[serde(default)]
        access_token: String,
        #[serde(default)]
        refresh_token: String,
        #[serde(default)]
        expires_in: Option<i64>,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        error_description: Option<String>,
    }

    if !status.is_success() {
        return Err(anyhow!(
            "刷新 access_token 失败: HTTP status={}, body={}",
            status,
            text
        ));
    }

    let token: TokenResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 refresh_token 响应失败: {} (body={})", e, text))?;

    if let Some(err) = token.error {
        return Err(anyhow!(
            "refresh_token 接口返回错误: {} ({})",
            err,
            token.error_description.unwrap_or_default()
        ));
    }

    if token.access_token.is_empty() {
        return Err(anyhow!(
            "refresh_token 响应中未包含 access_token，body={}",
            text
        ));
    }

    // 尝试自动写回到 config.toml，方便下次启动直接复用
    if let Err(e) = save_access_token_to_config_file(&token.access_token) {
        warn!("写入新的 access_token 到 config.toml 失败: {}", e);
    }

    Ok(token.access_token)
}

/// 将新的 access_token 写回当前工作目录下的 config.toml
fn save_access_token_to_config_file(new_token: &str) -> Result<()> {
    let path = "config.toml";
    let content = fs::read_to_string(path)?;
    let mut cfg: Config = toml::from_str(&content)?;

    cfg.baidu_open.access_token = new_token.to_string();

    let new_content = toml::to_string_pretty(&cfg)?;
    fs::write(path, new_content)?;

    info!("✅ 已将新的 access_token 自动写入 {}", path);
    Ok(())
}



