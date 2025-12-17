//! 获取百度网盘下载直链（基于 OpenList 方案）

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::AppState;

/// 获取文件下载直链（主入口）- OpenList 方案
pub async fn get_download_links(
    state: &AppState,
    fs_ids: &[u64],
) -> Result<Vec<(String, String)>> {
    if fs_ids.is_empty() {
        return Err(anyhow!("fs_ids 不能为空"));
    }

    info!("🔗 获取 {} 个文件的下载直链（OpenAPI 方式）...", fs_ids.len());

    let access_token = get_or_refresh_access_token(state).await?;
    let mut all_links = Vec::new();

    for (i, fs_id) in fs_ids.iter().enumerate() {
        info!("📥 [{}/{}] 获取 fs_id={} 的直链...", i + 1, fs_ids.len(), fs_id);

        // ✅ 改这里：加上 _internal
        match get_download_link_by_fsid_internal(state, *fs_id, &access_token).await {
            Ok((filename, url)) => {
                info!("✅ {}", filename);
                all_links.push((filename, url));
            }
            Err(e) => {
                warn!("⚠️ 获取 fs_id={} 的直链失败: {}", fs_id, e);
            }
        }

        if i < fs_ids.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    if all_links.is_empty() {
        return Err(anyhow!("未获取到任何下载链接"));
    }

    info!("✅ 成功获取 {} 个下载链接", all_links.len());
    Ok(all_links)
}

pub async fn get_download_link_by_fsid_internal(
    state: &AppState,
    fs_id: u64,
    access_token: &str,
) -> Result<(String, String)> {
    let url = format!(
        "https://pan.baidu.com/rest/2.0/xpan/multimedia?method=filemetas&fsids=[{}]&dlink=1&access_token={}",
        fs_id,
        urlencoding::encode(access_token)
    );

    debug!("📡 filemetas: fsid={}", fs_id);

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", "pan.baidu.com")
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    debug!("📨 filemetas 响应 (status={}): {}", status, &text[..text.len().min(300)]);

    #[derive(Deserialize)]
    struct FileMetasResponse {
        errno: i32,
        #[serde(default)]
        list: Vec<FileMetaItem>,
    }

    #[derive(Deserialize)]
    struct FileMetaItem {
        #[serde(default)]
        dlink: String,
        #[serde(default)]
        filename: String,
    }

    let result: FileMetasResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 filemetas 失败: {}, body: {}", e, text))?;

    if result.errno != 0 {
        return Err(anyhow!("filemetas errno={}", result.errno));
    }

    let item = result.list.first()
        .ok_or_else(|| anyhow!("filemetas 未返回数据"))?;

    if item.dlink.is_empty() {
        return Err(anyhow!("dlink 为空"));
    }

    let full_url = format!("{}&access_token={}", item.dlink, urlencoding::encode(access_token));

    debug!("🔗 请求 302 跳转...");

    let res = state
        .client
        .head(&full_url)
        .header("User-Agent", "pan.baidu.com")
        .send()
        .await?;

    let final_url = if res.status() == 302 {
        res.headers()
            .get("location")
            .and_then(|v| v.to_str().ok())
            .ok_or_else(|| anyhow!("302 但未返回 Location"))?
            .to_string()
    } else {
        full_url
    };

    Ok((item.filename.clone(), final_url))
}

async fn get_or_refresh_access_token(state: &AppState) -> Result<String> {
    let opencfg = &state.config.baidu_open;

    if !opencfg.access_token.is_empty() {
        return Ok(opencfg.access_token.clone());
    }

    if !opencfg.refresh_token.is_empty() {
        info!("⚠️ access_token 为空，使用 refresh_token 刷新...");
        let token = crate::baidupcs::openapi::refresh_token(state).await?;
        info!("✅ 已刷新 access_token (长度: {})", token.len());
        return Ok(token);
    }

    Err(anyhow!("未配置 access_token 或 refresh_token"))
}

pub async fn list_directory_fsids(state: &AppState, path: &str) -> Result<Vec<u64>> {
    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&num=100&order=time&desc=1",
        urlencoding::encode(path)
    );

    debug!("📡 列举目录: {}", path);

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", Config::browser_ua())
        .send()
        .await?;

    let text = resp.text().await?;

    #[derive(Deserialize)]
    struct ListResult {
        errno: i32,
        #[serde(default)]
        list: Vec<FileInfo>,
    }

    #[derive(Deserialize)]
    struct FileInfo {
        fs_id: u64,
        #[serde(default)]
        server_filename: String,
    }

    let result: ListResult = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析列举响应失败: {}, body: {}", e, text))?;

    if result.errno != 0 {
        return Err(anyhow!("列举目录失败: errno={}", result.errno));
    }

    info!("📁 目录中共有 {} 个文件", result.list.len());
    for (i, file) in result.list.iter().take(5).enumerate() {
        info!("  {}. {} (fs_id: {})", i + 1, file.server_filename, file.fs_id);
    }

    Ok(result.list.into_iter().map(|f| f.fs_id).collect())
}

/// 完整流程：分享链接 → 转存 → 获取 fsid（不获取直链）
pub async fn share_to_direct_link(
    state: &AppState,
    share_url: &str,
    pwd: &str,
) -> Result<Vec<(u64, String)>> {
    use crate::baidupcs;

    info!("🚀 处理分享链接: {}", share_url);

    let surl = baidupcs::extract_surl(share_url)
        .ok_or_else(|| anyhow!("无法提取 surl"))?;

    let info = baidupcs::get_share_info(state, share_url, &surl, pwd).await?;
    info!("📦 找到 {} 个文件", info.fs_ids.len());

    baidupcs::transfer_files(state, &info.shareid, &info.uk, &info.fs_ids, &info.bdstoken, &surl).await?;

    info!("⏳ 等待文件转存完成...");
    tokio::time::sleep(tokio::time::Duration::from_secs(8)).await;

    info!("📋 列举转存目录...");
    let files = list_directory_files(state, &state.config.baidu.save_path).await?;

    if files.is_empty() {
        return Err(anyhow!("转存后未找到文件"));
    }

    info!("✅ 找到 {} 个转存后的文件", files.len());

    let target_count = info.fs_ids.len();
    let target_files: Vec<(u64, String)> = files.into_iter().take(target_count).collect();

    info!("🎯 准备返回 {} 个文件的 fsid", target_files.len());

    Ok(target_files)
}

/// 列举目录获取 (fsid, filename) 列表
pub async fn list_directory_files(state: &AppState, path: &str) -> Result<Vec<(u64, String)>> {
    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&num=100&order=time&desc=1",
        urlencoding::encode(path)
    );

    debug!("📡 列举目录: {}", path);

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", Config::browser_ua())
        .send()
        .await?;

    let text = resp.text().await?;

    #[derive(Deserialize)]
    struct ListResult {
        errno: i32,
        #[serde(default)]
        list: Vec<FileInfo>,
    }

    #[derive(Deserialize)]
    struct FileInfo {
        fs_id: u64,
        #[serde(default)]
        server_filename: String,
    }

    let result: ListResult = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析列举响应失败: {}, body: {}", e, text))?;

    if result.errno != 0 {
        return Err(anyhow!("列举目录失败: errno={}", result.errno));
    }

    info!("📁 目录中共有 {} 个文件", result.list.len());

    Ok(result.list.into_iter().map(|f| (f.fs_id, f.server_filename)).collect())
}
