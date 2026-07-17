//! 百度网盘下载与目录枚举适配层
//!
//! 官方下载流程约束：
//! 1. 使用 `xpan/multimedia?method=filemetas&dlink=1` 获取 dlink；
//! 2. 使用 dlink 时必须追加 `access_token`；
//! 3. 请求 dlink 时必须设置 `User-Agent: pan.baidu.com`；
//! 4. dlink 有 8 小时有效期且存在 302 跳转。

use anyhow::{anyhow, Result};
use reqwest::header::CONTENT_TYPE;
use serde::Deserialize;
use tracing::debug;

use crate::baidupcs::types::{DownloadTarget, PanEntry};
use crate::config::Config;
use crate::error::map_baidu_errno;
use crate::AppState;

#[derive(Debug, Clone)]
pub struct FsidMeta {
    pub fsid: u64,
    pub filename: String,
    pub path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
    pub dlink: Option<String>,
}

pub async fn get_or_refresh_access_token(state: &AppState) -> Result<String> {
    if let Some(token) = state.cached_access_token() {
        return Ok(token);
    }

    let open_cfg = &state.config.baidu_open;
    if state.current_refresh_token().is_some() {
        match refresh_access_token(state).await {
            Ok(token) => return Ok(token),
            Err(err) if !open_cfg.access_token.is_empty() => {
                debug!("刷新 access_token 失败，回退到静态 access_token: {}", err);
                return Ok(open_cfg.access_token.clone());
            }
            Err(err) => return Err(err),
        }
    }
    if !open_cfg.access_token.is_empty() {
        return Ok(open_cfg.access_token.clone());
    }
    Err(anyhow!("未配置 BAIDU_ACCESS_TOKEN 或 BAIDU_REFRESH_TOKEN"))
}

pub async fn refresh_access_token(state: &AppState) -> Result<String> {
    crate::baidupcs::openapi::refresh_token(state).await
}

pub async fn get_fsid_meta(state: &AppState, fsid: u64, access_token: &str) -> Result<FsidMeta> {
    let mut metas = get_fsid_metas(state, &[fsid], access_token, true).await?;
    metas
        .pop()
        .ok_or_else(|| anyhow!("filemetas 返回空列表: fsid={}", fsid))
}

pub async fn get_fsid_metas(
    state: &AppState,
    fsids: &[u64],
    access_token: &str,
    need_dlink: bool,
) -> Result<Vec<FsidMeta>> {
    if fsids.is_empty() {
        return Err(anyhow!("fsids 不能为空"));
    }
    if fsids.len() > 100 {
        return Err(anyhow!("filemetas 单次最多查询 100 个 fsid"));
    }

    let fsids_json = serde_json::to_string(fsids)?;
    let url = format!(
        "https://pan.baidu.com/rest/2.0/xpan/multimedia?method=filemetas&access_token={}&fsids={}&dlink={}",
        urlencoding::encode(access_token),
        urlencoding::encode(&fsids_json),
        if need_dlink { 1 } else { 0 }
    );

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", Config::dlink_ua())
        .send()
        .await?;
    let status = resp.status();
    let text = resp.text().await?;
    debug!(
        "filemetas 响应 status={}, body={}",
        status,
        &text[..text.len().min(500)]
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
        fs_id: Option<u64>,
        #[serde(default)]
        fsid: Option<u64>,
        #[serde(default)]
        filename: String,
        #[serde(default)]
        path: String,
        #[serde(default)]
        isdir: i32,
        #[serde(default)]
        size: Option<u64>,
        #[serde(default)]
        dlink: String,
    }

    let result: FileMetasResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 filemetas 失败: {}, body={}", e, text))?;
    if result.errno != 0 {
        return Err(map_baidu_errno(result.errno, "filemetas").into());
    }

    Ok(result
        .list
        .into_iter()
        .map(|item| FsidMeta {
            fsid: item.fs_id.or(item.fsid).unwrap_or_default(),
            filename: item.filename,
            path: item.path,
            is_dir: item.isdir == 1,
            size: item.size,
            dlink: if item.dlink.is_empty() {
                None
            } else {
                Some(item.dlink)
            },
        })
        .collect())
}

pub async fn get_download_links(state: &AppState, fsids: &[u64]) -> Result<Vec<(String, String)>> {
    let mut out = Vec::new();
    for &fsid in fsids {
        out.push(get_download_link_by_fsid(state, fsid).await?);
    }
    Ok(out)
}

pub async fn get_download_link_by_fsid(state: &AppState, fsid: u64) -> Result<(String, String)> {
    let access_token = get_or_refresh_access_token(state).await?;
    match get_download_link_by_fsid_internal(state, fsid, &access_token).await {
        Ok(link) => Ok(link),
        Err(error) if is_access_token_invalid(&error) => {
            let refreshed = refresh_access_token(state)
                .await
                .map_err(|e| anyhow!("access_token 无效且刷新失败: {}", e))?;
            get_download_link_by_fsid_internal(state, fsid, &refreshed).await
        }
        Err(error) => Err(error),
    }
}

pub(crate) async fn get_download_link_by_fsid_internal(
    state: &AppState,
    fsid: u64,
    access_token: &str,
) -> Result<(String, String)> {
    let target = get_download_target(state, fsid, access_token).await?;
    let final_url = resolve_dlink_redirect(state, &target.dlink, access_token).await?;
    Ok((target.filename, final_url))
}

pub async fn get_download_target(
    state: &AppState,
    fsid: u64,
    access_token: &str,
) -> Result<DownloadTarget> {
    let meta = get_fsid_meta(state, fsid, access_token).await?;
    if meta.is_dir {
        return Err(anyhow!("fsid={} 是目录，不能直接获取文件 dlink", fsid));
    }
    let dlink = meta
        .dlink
        .ok_or_else(|| anyhow!("filemetas 未返回 dlink: fsid={}", fsid))?;
    Ok(DownloadTarget {
        fsid,
        filename: meta.filename,
        dlink,
    })
}

pub async fn resolve_dlink_redirect(
    state: &AppState,
    dlink: &str,
    access_token: &str,
) -> Result<String> {
    let url = append_access_token(dlink, access_token);
    let resp = state
        .no_redirect_client
        .get(&url)
        .header("User-Agent", Config::dlink_ua())
        .send()
        .await?;

    let status = resp.status();
    if status.is_redirection() {
        return resp
            .headers()
            .get(reqwest::header::LOCATION)
            .and_then(|v| v.to_str().ok())
            .map(|s| s.to_string())
            .ok_or_else(|| anyhow!("百度 dlink 302 响应缺少 Location"));
    }

    let content_type_is_json = resp
        .headers()
        .get(CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.to_ascii_lowercase().contains("json"));
    let small_response = resp.content_length().is_some_and(|len| len <= 4096);
    if status.is_success() && !content_type_is_json && !small_response {
        return Ok(url);
    }

    let text = resp.text().await.unwrap_or_default();
    if let Some(error) = baidu_error_from_body(&text, "dlink_redirect") {
        return Err(error.into());
    }
    if status.is_success() {
        return Ok(url);
    }

    Err(anyhow!(
        "解析 dlink 跳转失败: HTTP {}, body={}",
        status,
        text
    ))
}

fn is_access_token_invalid(error: &anyhow::Error) -> bool {
    error
        .downcast_ref::<crate::AppError>()
        .is_some_and(|err| err.code() == "baidu_access_token_invalid")
}

fn baidu_error_from_body(body: &str, context: &'static str) -> Option<crate::AppError> {
    #[derive(Deserialize)]
    struct ErrnoResponse {
        #[serde(default)]
        errno: Option<i32>,
        #[serde(default)]
        error_code: Option<i32>,
    }

    let result = serde_json::from_str::<ErrnoResponse>(body).ok()?;
    let errno = result.errno.or(result.error_code)?;
    (errno != 0).then(|| map_baidu_errno(errno, context))
}

fn append_access_token(dlink: &str, access_token: &str) -> String {
    let sep = if dlink.contains('?') { '&' } else { '?' };
    format!(
        "{}{}access_token={}",
        dlink,
        sep,
        urlencoding::encode(access_token)
    )
}

pub async fn list_directory_entries(state: &AppState, path: &str) -> Result<Vec<PanEntry>> {
    const PAGE_SIZE: usize = 1000;
    let mut page = 1;
    let mut out = Vec::new();

    loop {
        let entries = list_directory_entries_page(state, path, page, PAGE_SIZE).await?;
        let count = entries.len();
        out.extend(entries);
        if count < PAGE_SIZE {
            break;
        }
        page += 1;
    }

    Ok(out)
}

async fn list_directory_entries_page(
    state: &AppState,
    path: &str,
    page: usize,
    page_size: usize,
) -> Result<Vec<PanEntry>> {
    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&page={}&num={}&order=time&desc=1",
        urlencoding::encode(path),
        page,
        page_size
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
        #[serde(default)]
        size: Option<u64>,
    }

    let result: ListResult = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析目录列表失败: {}, body={}", e, text))?;
    if result.errno != 0 {
        return Err(map_baidu_errno(result.errno, "list_directory").into());
    }

    Ok(result
        .list
        .into_iter()
        .map(|f| {
            let full_path = if f.path.is_empty() {
                format!("{}/{}", path.trim_end_matches('/'), f.server_filename)
            } else {
                f.path
            };
            PanEntry {
                fsid: f.fsid,
                filename: f.server_filename.clone(),
                relative_path: f.server_filename,
                path: full_path,
                is_dir: f.isdir == 1,
                size: f.size,
            }
        })
        .collect())
}

pub async fn list_directory_entries_recursive(
    state: &AppState,
    root_path: &str,
) -> Result<Vec<PanEntry>> {
    let root_path = root_path.trim_end_matches('/').to_string();
    let mut stack = vec![root_path.clone()];
    let mut out = Vec::new();

    while let Some(dir) = stack.pop() {
        for mut entry in list_directory_entries(state, &dir).await? {
            let rel = entry
                .path
                .strip_prefix(&root_path)
                .unwrap_or(&entry.path)
                .trim_start_matches('/')
                .to_string();
            entry.relative_path = if rel.is_empty() {
                entry.filename.clone()
            } else {
                rel
            };

            if entry.is_dir {
                stack.push(entry.path.clone());
            } else {
                out.push(entry);
            }
        }
    }

    Ok(out)
}

pub async fn list_directory_fsids(state: &AppState, path: &str) -> Result<Vec<u64>> {
    Ok(list_directory_entries(state, path)
        .await?
        .into_iter()
        .map(|entry| entry.fsid)
        .collect())
}

pub async fn list_directory_files(state: &AppState, path: &str) -> Result<Vec<(u64, String)>> {
    Ok(list_directory_entries(state, path)
        .await?
        .into_iter()
        .map(|entry| (entry.fsid, entry.filename))
        .collect())
}

pub async fn expand_fsids_to_file_jobs(
    state: &AppState,
    fsids: &[u64],
    access_token: &str,
) -> Result<Vec<(String, u64)>> {
    let mut jobs = Vec::new();
    for &fsid in fsids {
        let meta = get_fsid_meta(state, fsid, access_token).await?;
        if meta.is_dir {
            for entry in list_directory_entries_recursive(state, &meta.path).await? {
                jobs.push((
                    format!("{}/{}", meta.filename, entry.relative_path),
                    entry.fsid,
                ));
            }
        } else {
            jobs.push((meta.filename, fsid));
        }
    }
    Ok(jobs)
}

pub async fn zip_directory_by_path_to_bytes(
    _state: &AppState,
    _dir_path: &str,
    _access_token: &str,
) -> Result<Vec<u8>> {
    Err(anyhow!("v1 暂不支持服务器端 ZIP 打包"))
}

pub async fn zip_fsids_to_bytes(
    _state: &AppState,
    _fsids: &[u64],
    _access_token: &str,
) -> Result<Vec<u8>> {
    Err(anyhow!("v1 暂不支持服务器端 ZIP 打包"))
}

pub async fn share_to_direct_link(
    state: &AppState,
    share_url: &str,
    pwd: &str,
) -> Result<Vec<(u64, String)>> {
    let result = crate::direct_link::convert_share_to_signed_downloads(
        state,
        crate::direct_link::ConvertShareCommand {
            link: share_url.to_string(),
            pwd: pwd.to_string(),
            ttl_secs: 24 * 3600,
        },
    )
    .await
    .map_err(|e| anyhow!(e.to_string()))?;
    Ok(result
        .items
        .into_iter()
        .map(|item| (item.fsid, item.filename))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn appends_access_token_with_existing_query() {
        assert_eq!(
            append_access_token("https://d.pcs.baidu.com/file/a?x=1", "tok en"),
            "https://d.pcs.baidu.com/file/a?x=1&access_token=tok%20en"
        );
    }

    #[test]
    fn appends_access_token_without_query() {
        assert_eq!(
            append_access_token("https://d.pcs.baidu.com/file/a", "token"),
            "https://d.pcs.baidu.com/file/a?access_token=token"
        );
    }

    #[test]
    fn parses_baidu_errno_from_json_body() {
        let err = baidu_error_from_body(r#"{"errno":31045}"#, "download").unwrap();
        assert_eq!(err.code(), "baidu_access_token_invalid");

        let err = baidu_error_from_body(r#"{"error_code":31326}"#, "download").unwrap();
        assert_eq!(err.code(), "baidu_hotlink_protection");

        assert!(baidu_error_from_body(r#"{"errno":0}"#, "download").is_none());
    }
}
