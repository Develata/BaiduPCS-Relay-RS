//! 百度网盘下载模块 - OpenList 方案
//! 通过 OpenAPI 获取直链，支持文件夹自动打包

use anyhow::{anyhow, Result};
use serde::Deserialize;
use std::io::Write;
use tracing::{debug, info, warn};
use zip::{
    write::{FileOptions, ZipWriter},
    CompressionMethod,
};

use crate::config::Config;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct FsidMeta {
    pub fsid: u64,
    pub filename: String,
    pub path: String,
    pub is_dir: bool,
}

/// 查询 fsid 的元信息（用于区分文件/文件夹，并拿到 path）
pub async fn get_fsid_meta(state: &AppState, fsid: u64, access_token: &str) -> Result<FsidMeta> {
    let url = format!(
        "https://pan.baidu.com/rest/2.0/xpan/multimedia?method=filemetas&fsids=[{}]&dlink=1&access_token={}",
        fsid,
        urlencoding::encode(access_token)
    );

    debug!("🔍 查询文件元信息 fsid={}", fsid);

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", "pan.baidu.com")
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    debug!(
        "filemetas 响应 status={}, body={}",
        status,
        &text[..text.len().min(300)]
    );

    #[derive(Deserialize)]
    struct FileMetasResponse {
        errno: i32,
        #[serde(default)]
        list: Vec<FileMetaItem>,
    }

    #[derive(Deserialize)]
    struct FileMetaItem {
        #[serde(default)]
        filename: String,
        #[serde(default)]
        path: String,
        #[serde(default)]
        isdir: i32,
    }

    let result: FileMetasResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 filemetas 失败: {}, body={}", e, text))?;

    if result.errno != 0 {
        return Err(anyhow!("filemetas 返回错误 errno={}", result.errno));
    }

    let item = result
        .list
        .first()
        .ok_or_else(|| anyhow!("filemetas 返回空列表"))?;

    Ok(FsidMeta {
        fsid,
        filename: item.filename.clone(),
        path: item.path.clone(),
        is_dir: item.isdir == 1,
    })
}

/// 批量获取下载链接 - OpenList 方案
pub async fn get_download_links(state: &AppState, fsids: &[u64]) -> Result<Vec<(String, String)>> {
    if fsids.is_empty() {
        return Err(anyhow!("文件 fsids 列表不能为空"));
    }

    info!(
        "📥 使用 OpenAPI 方式获取下载链接..., 共 {} 个文件",
        fsids.len()
    );

    let access_token = get_or_refresh_access_token(state).await?;

    let mut all_links = Vec::new();

    for (i, &fsid) in fsids.iter().enumerate() {
        info!("🔍 处理第 {}/{} 个 fsid: {}", i + 1, fsids.len(), fsid);

        match get_download_link_by_fsid_internal(state, fsid, &access_token).await {
            Ok((filename, url)) => {
                info!("✅ 获取成功: {}", filename);
                all_links.push((filename, url));
            }
            Err(e) => {
                warn!("⚠️  fsid {} 获取失败: {}", fsid, e);
            }
        }

        if i < fsids.len() - 1 {
            tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        }
    }

    if all_links.is_empty() {
        return Err(anyhow!("所有文件都获取失败"));
    }

    info!("✅ 成功获取 {} 个下载链接", all_links.len());
    Ok(all_links)
}

/// 获取单个文件的下载链接（内部使用）
pub async fn get_download_link_by_fsid_internal(
    state: &AppState,
    fsid: u64,
    access_token: &str,
) -> Result<(String, String)> {
    let url = format!(
        "https://pan.baidu.com/rest/2.0/xpan/multimedia?method=filemetas&fsids=[{}]&dlink=1&access_token={}",
        fsid,
        urlencoding::encode(access_token)
    );

    debug!("🔍 查询文件元信息 fsid={}", fsid);

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", "pan.baidu.com")
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;

    debug!(
        "filemetas 响应 status={}, body={}",
        status,
        &text[..text.len().min(300)]
    );

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
        #[serde(default)]
        path: String,
        #[serde(default)]
        isdir: i32,
    }

    let result: FileMetasResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 filemetas 失败: {}, body={}", e, text))?;

    if result.errno != 0 {
        return Err(anyhow!("filemetas 返回错误 errno={}", result.errno));
    }

    let item = result
        .list
        .first()
        .ok_or_else(|| anyhow!("filemetas 返回空列表"))?;

    if item.isdir == 1 {
        warn!("⚠️  fsid={} 是文件夹: {}", fsid, item.filename);
        return Err(anyhow!("FOLDER:{}:{}:{}", fsid, item.path, item.filename));
    }

    if item.dlink.is_empty() {
        return Err(anyhow!("文件 dlink 为空: {}", item.filename));
    }

    let full_url = format!(
        "{}?access_token={}",
        item.dlink,
        urlencoding::encode(access_token)
    );
    debug!("📥 302 跳转获取最终下载链接...");

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
            .ok_or_else(|| anyhow!("302 重定向缺少 Location 头"))?
            .to_string()
    } else {
        full_url
    };

    Ok((item.filename.clone(), final_url))
}

#[derive(Debug, Clone)]
struct DirEntry {
    fsid: u64,
    name: String,
    path: String,
    is_dir: bool,
}

async fn list_dir_entries(state: &AppState, dir_path: &str) -> Result<Vec<DirEntry>> {
    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&num=1000&order=time&desc=0",
        urlencoding::encode(dir_path)
    );

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
        #[serde(rename = "fs_id")]
        fsid: u64,
        #[serde(default)]
        server_filename: String,
        #[serde(default)]
        path: String,
        #[serde(default)]
        isdir: i32,
    }

    let result: ListResult = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析目录列表失败, body={}, error={}", text, e))?;

    if result.errno != 0 {
        return Err(anyhow!("获取目录列表失败 errno={}", result.errno));
    }

    Ok(result
        .list
        .into_iter()
        .map(|f| {
            let name = f.server_filename;
            let path = if f.path.is_empty() {
                // 兜底：部分字段缺失时，用 dir_path + name 拼一个
                format!("{}/{}", dir_path.trim_end_matches('/'), &name)
            } else {
                f.path
            };

            DirEntry {
                fsid: f.fsid,
                name,
                path,
                is_dir: f.isdir == 1,
            }
        })
        .collect())
}

async fn collect_files_recursive(state: &AppState, base_dir: &str) -> Result<Vec<(String, u64)>> {
    let base_dir = base_dir.trim_end_matches('/').to_string();
    let mut stack = vec![base_dir.clone()];
    let mut out: Vec<(String, u64)> = Vec::new();

    while let Some(dir) = stack.pop() {
        let entries = list_dir_entries(state, &dir).await?;
        for e in entries {
            if e.is_dir {
                stack.push(e.path);
                continue;
            }

            let rel = e
                .path
                .strip_prefix(&base_dir)
                .unwrap_or(&e.path)
                .trim_start_matches('/')
                .to_string();

            let name = if rel.is_empty() { e.name } else { rel };
            out.push((name, e.fsid));
        }
    }

    if out.is_empty() {
        return Err(anyhow!("目录为空或没有可下载文件"));
    }

    Ok(out)
}

/// 将输入的 fsid（文件/文件夹）展开为具体文件列表：返回 (zip 内路径, 文件 fsid)
///
/// - 文件：返回 (filename, fsid)
/// - 文件夹：递归展开目录，并返回 (folder_name/relative/path, file_fsid)
pub async fn expand_fsids_to_file_jobs(
    state: &AppState,
    fsids: &[u64],
    access_token: &str,
) -> Result<Vec<(String, u64)>> {
    if fsids.is_empty() {
        return Err(anyhow!("fsids 不能为空"));
    }

    let mut file_jobs: Vec<(String, u64)> = Vec::new();

    for &fsid in fsids {
        let meta = get_fsid_meta(state, fsid, access_token).await?;
        if meta.is_dir {
            info!("📂 展开目录: {} ({})", meta.filename, meta.path);
            let files = collect_files_recursive(state, &meta.path).await?;
            for (rel, child_fsid) in files {
                let zip_name = if rel.is_empty() {
                    meta.filename.clone()
                } else {
                    format!("{}/{}", meta.filename, rel)
                };
                file_jobs.push((zip_name, child_fsid));
            }
        } else {
            file_jobs.push((meta.filename, fsid));
        }
    }

    if file_jobs.is_empty() {
        return Err(anyhow!("没有可打包的文件"));
    }

    Ok(file_jobs)
}

/// 将一个目录（通过目录 path）递归打包为 ZIP
pub async fn zip_directory_by_path_to_bytes(
    state: &AppState,
    dir_path: &str,
    access_token: &str,
) -> Result<Vec<u8>> {
    info!("🗜️  开始递归打包目录为 ZIP: {}", dir_path);

    let files = collect_files_recursive(state, dir_path).await?;
    let total = files.len();
    info!("📄 目录内共 {} 个文件需要打包", total);

    // 拉取每个文件内容
    let mut entries: Vec<(String, Vec<u8>)> = Vec::with_capacity(total);

    for (i, (zip_name, fsid)) in files.into_iter().enumerate() {
        info!("📥 下载第 {}/{} 个文件 fsid={}", i + 1, total, fsid);

        let (_filename, url) =
            get_download_link_by_fsid_internal(state, fsid, access_token).await?;

        let resp = state
            .client
            .get(&url)
            .header("User-Agent", "pan.baidu.com")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "下载文件失败 fsid={}, status={}",
                fsid,
                resp.status()
            ));
        }

        let bytes = resp.bytes().await?.to_vec();
        entries.push((zip_name, bytes));
    }

    let zip_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let cursor = std::io::Cursor::new(Vec::<u8>::new());
        let mut zip = ZipWriter::new(cursor);

        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);
        for (name, data) in entries {
            let name = name.replace('\\', "/");
            zip.start_file(&name, options)?;
            zip.write_all(&data[..])?;
        }
        let cursor = zip.finish()?;
        Ok(cursor.into_inner())
    })
    .await??;

    info!("✅ 目录 ZIP 打包完成 bytes={}", zip_bytes.len());
    Ok(zip_bytes)
}

/// 将多个 fsid 打包成 ZIP（用于文件夹）
pub async fn zip_fsids_to_bytes(
    state: &AppState,
    fsids: &[u64],
    access_token: &str,
) -> Result<Vec<u8>> {
    if fsids.is_empty() {
        return Err(anyhow!("文件 fsids 列表不能为空"));
    }

    info!(
        "📦 开始 ZIP 打包，共 {} 个输入项（文件/文件夹）",
        fsids.len()
    );

    let file_jobs = expand_fsids_to_file_jobs(state, fsids, access_token).await?;
    info!("📄 需要打包的文件总数: {}", file_jobs.len());

    // 下载所有文件内容
    let total = file_jobs.len();
    let mut entries: Vec<(String, Vec<u8>)> = Vec::with_capacity(total);

    for (i, (zip_name, fsid)) in file_jobs.into_iter().enumerate() {
        info!("📥 下载第 {}/{} 个文件 fsid={}", i + 1, total, fsid);

        let (_filename, url) =
            get_download_link_by_fsid_internal(state, fsid, access_token).await?;

        let resp = state
            .client
            .get(&url)
            .header("User-Agent", "pan.baidu.com")
            .send()
            .await?;

        if !resp.status().is_success() {
            return Err(anyhow!(
                "下载文件失败 fsid={}, status={}",
                fsid,
                resp.status()
            ));
        }

        let bytes = resp.bytes().await?.to_vec();
        entries.push((zip_name, bytes));
    }

    // 打包成 ZIP（在 Tokio runtime 中执行阻塞操作）
    let zip_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let cursor = std::io::Cursor::new(Vec::<u8>::new());
        let mut zip = ZipWriter::new(cursor);

        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

        for (filename, data) in entries {
            let name = filename.replace("\\", "/");
            zip.start_file(&name, options)?;
            zip.write_all(&data[..])?;
        }

        let cursor = zip.finish()?;
        Ok(cursor.into_inner())
    })
    .await??;

    info!("✅ ZIP 打包完成 bytes={}", zip_bytes.len());
    Ok(zip_bytes)
}

async fn get_or_refresh_access_token(state: &AppState) -> Result<String> {
    let open_cfg = &state.config.baidu_open;

    if !open_cfg.access_token.is_empty() {
        return Ok(open_cfg.access_token.clone());
    }

    if !open_cfg.refresh_token.is_empty() {
        info!("🔄 使用 accesstoken refreshtoken ...");
        let token = crate::baidupcs::openapi::refresh_token(state).await?;
        info!("✅ 获取 accesstoken 成功，长度={}", token.len());
        return Ok(token);
    }

    Err(anyhow!("未配置 accesstoken 或 refreshtoken"))
}

pub async fn list_directory_fsids(state: &AppState, path: &str) -> Result<Vec<u64>> {
    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&num=100&order=time&desc=1",
        urlencoding::encode(path)
    );

    debug!("📂 列出目录: {}", path);

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
        #[serde(rename = "fs_id")]
        fsid: u64,
        #[serde(default)]
        server_filename: String,
    }

    let result: ListResult = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析目录列表失败, body={}, error={}", text, e))?;

    if result.errno != 0 {
        return Err(anyhow!("获取目录列表失败 errno={}", result.errno));
    }

    info!("📂 目录文件数: {}", result.list.len());

    for (i, file) in result.list.iter().take(5).enumerate() {
        info!("  {}. {} (fsid={})", i + 1, file.server_filename, file.fsid);
    }

    Ok(result.list.into_iter().map(|f| f.fsid).collect())
}

/// 分享链接转直链（返回 fsid 和文件名列表）
/// 分享链接转直链（返回 fsid 和文件名列表）
pub async fn share_to_direct_link(
    state: &AppState,
    share_url: &str,
    pwd: &str,
) -> Result<Vec<(u64, String)>> {
    use crate::baidupcs;

    info!("🔗 处理分享链接: {}", share_url);

    let surl = baidupcs::extract_surl(share_url).ok_or_else(|| anyhow!("无法提取 surl"))?;

    let info = baidupcs::get_share_info(state, share_url, &surl, pwd).await?;
    info!("📦 分享文件数: {}", info.fs_ids.len()); // ✅ 修复：fsids -> fs_ids

    baidupcs::transfer_files(
        state,
        &info.shareid,
        &info.uk,
        &info.fs_ids, // ✅ 修复
        &info.bdstoken,
        &surl,
    )
    .await?;

    info!("⏳ 等待转存完成...");
    tokio::time::sleep(tokio::time::Duration::from_secs(8)).await;

    info!("📂 查询转存目录...");
    let files = list_directory_files(state, &state.config.baidu.save_path).await?;

    if files.is_empty() {
        return Err(anyhow!("转存目录为空"));
    }

    info!("✅ 找到 {} 个文件", files.len());

    let target_count = info.fs_ids.len(); // ✅ 修复
    let target_files: Vec<(u64, String)> = files.into_iter().take(target_count).collect();

    info!("🎯 返回 {} 个 fsid", target_files.len());

    Ok(target_files)
}

pub async fn list_directory_files(state: &AppState, path: &str) -> Result<Vec<(u64, String)>> {
    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&num=100&order=time&desc=1",
        urlencoding::encode(path)
    );

    debug!("📂 列出目录: {}", path);

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
        #[serde(rename = "fs_id")]
        fsid: u64,
        #[serde(default)]
        server_filename: String,
    }

    let result: ListResult = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析目录列表失败, body={}, error={}", text, e))?;

    if result.errno != 0 {
        return Err(anyhow!("获取目录列表失败 errno={}", result.errno));
    }

    info!("📂 目录文件数: {}", result.list.len());

    Ok(result
        .list
        .into_iter()
        .map(|f| (f.fsid, f.server_filename))
        .collect())
}
