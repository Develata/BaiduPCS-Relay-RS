//! 获取百度网盘下载直链

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tracing::{debug, info, warn};

use crate::config::Config;
use crate::AppState;

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DownloadResponse {
    errno: i32,
    #[serde(default)]
    list: Vec<DownloadItem>,
}

#[derive(Debug, Deserialize)]
#[allow(dead_code)]
struct DownloadItem {
    #[serde(rename = "fs_id")]
    fs_id: u64,
    #[serde(default)]
    dlink: String,
    #[serde(default)]
    filename: String,
}

/// 获取文件下载直链（主入口）
pub async fn get_download_links(
    state: &AppState,
    fs_ids: &[u64],
) -> Result<Vec<(String, String)>> {
    if fs_ids.is_empty() {
        return Err(anyhow!("fs_ids 不能为空"));
    }

    info!("🔗 获取 {} 个文件的下载直链...", fs_ids.len());

    // ✅ 直接使用逐个获取（PCS API）
    let mut all_links = Vec::new();
    for (i, fs_id) in fs_ids.iter().enumerate() {
        info!("📥 [{}/{}] 获取 fs_id={} 的直链...", i + 1, fs_ids.len(), fs_id);
        
        // 先通过 list API 获取文件路径
        match get_file_path_by_fsid(state, *fs_id).await {
            Ok((path, filename)) => {
                info!("   文件路径: {}", path);
                
                // 再通过路径获取直链
                match get_download_link_by_path(state, &path).await {
                    Ok(dlink) => {
                        info!("✅ {}", filename);
                        all_links.push((filename, dlink));
                    }
                    Err(e) => {
                        warn!("⚠️ 获取 {} 的直链失败: {}", filename, e);
                    }
                }
            }
            Err(e) => {
                warn!("⚠️ 获取 fs_id={} 的路径失败: {}", fs_id, e);
            }
        }

        // 避免请求过快
        if i < fs_ids.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(800)).await;
        }
    }

    if all_links.is_empty() {
        return Err(anyhow!("未获取到任何下载链接"));
    }

    info!("✅ 成功获取 {} 个下载链接", all_links.len());
    Ok(all_links)
}

/// 通过 fs_id 获取文件路径
async fn get_file_path_by_fsid(
    state: &AppState,
    fs_id: u64,
) -> Result<(String, String)> {
    // 遍历目录寻找对应的 fs_id
    let path = &state.config.baidu.save_path;
    
    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&num=1000&order=time&desc=1",
        urlencoding::encode(path)
    );

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", Config::browser_ua())
        .send()
        .await?;

    #[derive(Deserialize)]
    struct ListResult {
        errno: i32,
        #[serde(default)]
        list: Vec<FileInfo>,
    }

    #[derive(Deserialize)]
    struct FileInfo {
        fs_id: u64,
        path: String,
        server_filename: String,
    }

    let result: ListResult = resp.json().await?;

    if result.errno != 0 {
        return Err(anyhow!("列举失败: errno={}", result.errno));
    }

    for file in result.list {
        if file.fs_id == fs_id {
            return Ok((file.path, file.server_filename));
        }
    }

    Err(anyhow!("未找到 fs_id={}", fs_id))
}

/// 通过文件路径获取下载直链（使用 PCS API）
pub async fn get_download_link_by_path(
    state: &AppState,
    path: &str,
) -> Result<String> {
    // 使用 PCS API
    let url = format!(
        "https://pcs.baidu.com/rest/2.0/pcs/file?method=locatedownload&app_id=250528&path={}",
        urlencoding::encode(path)
    );

    debug!("📡 PCS API: {}", url);

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", Config::app_ua())  // ✅ 使用 App UA
        .send()
        .await?;

    let text = resp.text().await?;
    debug!("📨 PCS 响应: {}", &text[..200.min(text.len())]);

    #[derive(Deserialize)]
    struct PcsResponse {
        #[serde(default)]
        error_code: i32,
        #[serde(default)]
        urls: Vec<UrlInfo>,
    }

    #[derive(Deserialize)]
    struct UrlInfo {
        url: String,
    }

    let result: PcsResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析失败: {}, body: {}", e, text))?;

    if result.error_code != 0 {
        return Err(anyhow!("PCS API 失败: error_code={}", result.error_code));
    }

    if let Some(url_info) = result.urls.first() {
        return Ok(url_info.url.clone());
    }

    Err(anyhow!("未返回下载链接"))
}

/// 列举目录获取 fs_id
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
    debug!("📨 list 响应: {}", &text[..500.min(text.len())]);

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

/// 完整流程：分享链接 → 转存 → 获取直链
pub async fn share_to_direct_link(
    state: &AppState,
    share_url: &str,
    pwd: &str,
) -> Result<Vec<(String, String)>> {
    use crate::baidupcs;

    info!("🚀 处理分享链接: {}", share_url);

    // 1. 提取 surl
    let surl = baidupcs::extract_surl(share_url)
        .ok_or_else(|| anyhow!("无法提取 surl"))?;

    // 2. 获取分享信息
    let info = baidupcs::get_share_info(state, share_url, &surl, pwd).await?;
    info!("📦 找到 {} 个文件", info.fs_ids.len());

    // 3. 转存到网盘
    baidupcs::transfer_files(state, &info.shareid, &info.uk, &info.fs_ids, &info.bdstoken, &surl).await?;

    // 4. 等待转存完成
    info!("⏳ 等待文件转存完成...");
    tokio::time::sleep(tokio::time::Duration::from_secs(8)).await;

    // 5. 列举目录获取转存后的文件
    info!("📋 列举转存目录...");
    let saved_fs_ids = list_directory_fsids(state, &state.config.baidu.save_path).await?;

    if saved_fs_ids.is_empty() {
        return Err(anyhow!("转存后未找到文件"));
    }

    info!("✅ 找到 {} 个转存后的文件", saved_fs_ids.len());

    // 6. 只获取最新的 N 个文件的直链
    let target_count = info.fs_ids.len();
    let target_fs_ids: Vec<u64> = saved_fs_ids.into_iter().take(target_count).collect();

    info!("🎯 准备获取 {} 个文件的直链", target_fs_ids.len());

    // 7. 获取下载直链
    get_download_links(state, &target_fs_ids).await
}
