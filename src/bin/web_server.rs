//! Web 服务器 - OpenList 方案
use anyhow::Result;
use axum::{
    body::Body,
    extract::{Query, State},
    http::{header, StatusCode},
    response::{Html, IntoResponse, Json, Redirect, Response},
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

#[derive(Deserialize)]
struct ZipRequest {
    fsids: Vec<u64>,
    archive_name: String,
    #[serde(default)]
    token: String,
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
        .route("/api/zip", post(zip_handler)) // 服务器端打包 ZIP 并返回
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
                        3600 * 24,
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

/// 新的 ZIP 打包接口：接收一组 fsid，在服务器端打包为 ZIP 并返回附件
/// 支持大文件分卷：>1GB 自动分成多个 <1GB 的 part
async fn zip_handler(
    State(state): State<Arc<AppState>>,
    Json(req): Json<ZipRequest>,
) -> impl IntoResponse {
    if req.fsids.is_empty() {
        return (StatusCode::BAD_REQUEST, "fsids 不能为空").into_response();
    }

    // 验证访问密码
    if req.token.is_empty() || req.token != state.config.web.access_token {
        return (StatusCode::UNAUTHORIZED, "访问密码错误").into_response();
    }

    let access_token = match get_access_token(&state).await {
        Ok(token) => token,
        Err(e) => {
            warn!("❌ 获取 access_token 失败: {}", e);
            return StatusCode::INTERNAL_SERVER_ERROR.into_response();
        }
    };

    let jobs = match baidupcs::expand_fsids_to_file_jobs(&state, &req.fsids, &access_token).await {
        Ok(v) => v,
        Err(e) => {
            warn!("❌ 展开 fsids 失败: {}", e);
            return (StatusCode::BAD_REQUEST, format!("展开 fsids 失败: {}", e)).into_response();
        }
    };

    let archive_base_name = if req.archive_name.ends_with(".zip") {
        req.archive_name[..req.archive_name.len() - 4].to_string()
    } else {
        req.archive_name.clone()
    };

    match pack_files_to_zip_with_split(
        &state,
        &access_token,
        jobs,
        &archive_base_name,
        state.config.web.max_zip_size,
    )
    .await
    {
        Ok(parts) => {
            if parts.is_empty() {
                return (StatusCode::INTERNAL_SERVER_ERROR, "生成的 ZIP 文件为空").into_response();
            }

            if parts.len() == 1 {
                // 单个文件，直接返回
                let zip_bytes = parts.into_iter().next().unwrap();
                let mut resp = Response::new(Body::from(zip_bytes));
                *resp.status_mut() = StatusCode::OK;

                let headers = resp.headers_mut();
                headers.insert(
                    header::CONTENT_TYPE,
                    header::HeaderValue::from_static("application/zip"),
                );
                let filename = format!("{}.zip", archive_base_name);
                let cd_value = format!(
                    "attachment; filename=\"{}\"",
                    urlencoding::encode(&filename)
                );
                if let Ok(v) = header::HeaderValue::from_str(&cd_value) {
                    headers.insert(header::CONTENT_DISPOSITION, v);
                }
                resp.into_response()
            } else {
                // 多个分卷，返回 JSON 列表和分卷信息
                #[derive(Serialize)]
                struct ZipPart {
                    part_num: u32,
                    filename: String,
                    size_bytes: u64,
                }

                let part_list: Vec<ZipPart> = parts
                    .iter()
                    .enumerate()
                    .map(|(idx, data)| ZipPart {
                        part_num: (idx + 1) as u32,
                        filename: format!("{}.z{:02}", archive_base_name, idx + 1),
                        size_bytes: data.len() as u64,
                    })
                    .collect();

                let total_size: u64 = parts.iter().map(|p| p.len() as u64).sum();

                info!(
                    "✅ ZIP 分卷完成: {} 个 part, 总大小 {} MB",
                    parts.len(),
                    total_size / 1024 / 1024
                );

                (
                    StatusCode::OK,
                    Json(serde_json::json!({
                        "success": true,
                        "total_parts": parts.len(),
                        "total_size": total_size,
                        "parts": part_list,
                        "message": "文件超过大小限制，已分卷。请分别下载各个 part 文件。"
                    })),
                )
                    .into_response()
            }
        }
        Err(e) => {
            warn!("❌ ZIP 打包失败: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("ZIP 打包失败: {}", e),
            )
                .into_response()
        }
    }
}

/// 支持分卷的 ZIP 打包函数
/// 返回 Vec<Vec<u8>>，每个元素是一个 ZIP 分卷
/// 如果总大小 <= max_size，返回单个 part；否则分成多个 <1GB 的 part
async fn pack_files_to_zip_with_split(
    state: &Arc<AppState>,
    access_token: &str,
    jobs: Vec<(String, u64)>,
    archive_base_name: &str,
    max_zip_size: u64,
) -> Result<Vec<Vec<u8>>> {
    use std::io::Write;
    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;

    info!(
        "📦 开始下载并打包 {} 个文件到 ZIP (最大大小限制: {} MB)",
        jobs.len(),
        max_zip_size / 1024 / 1024
    );

    // 第一阶段：下载所有文件到内存，并估算大小
    let mut entries: Vec<(String, Vec<u8>)> = Vec::with_capacity(jobs.len());
    let mut total_uncompressed_size: u64 = 0;

    for (i, (zip_name, fsid)) in jobs.into_iter().enumerate() {
        info!(
            "📥 下载第 {}/{} 个文件 fsid={}",
            i + 1,
            entries.capacity().max(1),
            fsid
        );

        let (_filename, url) =
            baidupcs::get_download_link_by_fsid_internal(state, fsid, access_token)
                .await
                .map_err(|e| anyhow::anyhow!("获取直链失败 fsid={}: {}", fsid, e))?;

        let resp = state
            .client
            .get(&url)
            .header("User-Agent", "pan.baidu.com")
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("请求文件失败 fsid={}: {}", fsid, e))?;

        if !resp.status().is_success() {
            return Err(anyhow::anyhow!(
                "下载文件失败 fsid={}, status={}",
                fsid,
                resp.status()
            ));
        }

        let bytes = resp
            .bytes()
            .await
            .map_err(|e| anyhow::anyhow!("读取文件内容失败 fsid={}: {}", fsid, e))?
            .to_vec();

        total_uncompressed_size += bytes.len() as u64;
        info!("✅ 下载完成 {} bytes, filename={}", bytes.len(), zip_name);
        entries.push((zip_name, bytes));
    }

    info!(
        "📊 总未压缩大小: {} MB",
        total_uncompressed_size / 1024 / 1024
    );

    // 检查是否超过限制
    if total_uncompressed_size > max_zip_size {
        warn!(
            "⚠️  文件大小 {} MB 超过限制 {} MB，将分卷打包",
            total_uncompressed_size / 1024 / 1024,
            max_zip_size / 1024 / 1024
        );

        return pack_files_to_zip_parts(entries, archive_base_name, max_zip_size);
    }

    // 第二阶段：在 spawn_blocking 里打包成单个 ZIP
    let zip_bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>> {
        let mut buffer = Vec::new();
        let cursor = std::io::Cursor::new(&mut buffer);
        let mut zip = ZipWriter::new(cursor);

        let options = FileOptions::default().compression_method(CompressionMethod::Deflated);

        for (filename, data) in entries {
            let name = filename.replace("\\", "/");
            zip.start_file(&name, options)?;
            zip.write_all(&data[..])?;
        }

        let cursor = zip.finish()?;
        Ok(cursor.into_inner().to_vec())
    })
    .await
    .map_err(|e| anyhow::anyhow!("ZIP 打包任务失败: {}", e))??;

    info!("✅ ZIP 打包完成 {} bytes", zip_bytes.len());
    Ok(vec![zip_bytes])
}

/// 将文件分卷成多个 <1GB 的 ZIP 文件
fn pack_files_to_zip_parts(
    entries: Vec<(String, Vec<u8>)>,
    _archive_base_name: &str,
    max_part_size: u64,
) -> Result<Vec<Vec<u8>>> {
    use std::io::Write;
    use zip::write::{FileOptions, ZipWriter};
    use zip::CompressionMethod;

    const PART_SIZE_LIMIT: u64 = 1024 * 1024 * 1024; // 1GB per part

    let part_limit = PART_SIZE_LIMIT.min(max_part_size);
    let total_entries = entries.len();
    let mut parts = Vec::new();
    let mut current_part_data = Vec::new();
    let mut current_part_size: u64 = 0;
    let mut entries_in_part = 0;

    info!(
        "📊 开始分卷（每个 part 限制: {} MB）",
        part_limit / 1024 / 1024
    );

    for (idx, (filename, data)) in entries.into_iter().enumerate() {
        let data_size = data.len() as u64;

        // 如果加上这个文件会超过 part 限制，先保存当前 part
        if current_part_size > 0 && current_part_size + data_size > part_limit {
            info!(
                "💾 part {} 完成: {} 个文件, {} MB",
                parts.len() + 1,
                entries_in_part,
                current_part_size / 1024 / 1024
            );

            parts.push(current_part_data);
            current_part_data = Vec::new();
            current_part_size = 0;
            entries_in_part = 0;
        }

        current_part_data.push((filename, data));
        current_part_size += data_size;
        entries_in_part += 1;

        if (idx + 1) % 10 == 0 {
            info!("📦 已处理 {}/{} 个文件", idx + 1, total_entries);
        }
    }

    // 加入最后一个 part
    if !current_part_data.is_empty() {
        info!(
            "💾 part {} 完成: {} 个文件, {} MB",
            parts.len() + 1,
            entries_in_part,
            current_part_size / 1024 / 1024
        );
        parts.push(current_part_data);
    }

    // 第二阶段：在 spawn_blocking 中并行压缩每个 part
    let num_parts = parts.len();
    info!("⚙️  开始压缩 {} 个 part...", num_parts);

    let zips = std::thread::scope(|s| {
        let handles: Vec<_> = parts
            .into_iter()
            .enumerate()
            .map(|(part_idx, entries)| {
                s.spawn(move || -> Result<Vec<u8>> {
                    let mut buffer = Vec::new();
                    let cursor = std::io::Cursor::new(&mut buffer);
                    let mut zip = ZipWriter::new(cursor);
                    let options =
                        FileOptions::default().compression_method(CompressionMethod::Deflated);

                    for (filename, data) in entries {
                        let name = filename.replace("\\", "/");
                        zip.start_file(&name, options)?;
                        zip.write_all(&data[..])?;
                    }

                    let cursor = zip.finish()?;
                    let result = cursor.into_inner().to_vec();
                    info!("✅ part {} 压缩完成: {} bytes", part_idx + 1, result.len());
                    Ok(result)
                })
            })
            .collect();

        handles
            .into_iter()
            .map(|h| {
                h.join()
                    .unwrap_or_else(|_| Err(anyhow::anyhow!("线程 panic")))
            })
            .collect::<Result<Vec<_>>>()
    })?;

    info!("✅ 所有 part 压缩完成");
    Ok(zips)
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

    // 先判断 fsid 类型：文件 -> 302 跳转；文件夹 -> 递归打包 ZIP 返回
    let meta = match baidupcs::get_fsid_meta(&state, fsid, &access_token).await {
        Ok(m) => m,
        Err(e) => {
            warn!("❌ 查询 fsid 元信息失败: fsid={}, error={}", fsid, e);
            return StatusCode::BAD_GATEWAY.into_response();
        }
    };

    if meta.is_dir {
        info!(
            "📦 fsid={} 是文件夹，开始打包 ZIP: {} ({})",
            fsid, meta.filename, meta.path
        );

        let jobs = match baidupcs::expand_fsids_to_file_jobs(&state, &[fsid], &access_token).await {
            Ok(v) => v,
            Err(e) => {
                warn!("❌ 展开目录失败: fsid={}, error={}", fsid, e);
                return (StatusCode::BAD_GATEWAY, format!("展开目录失败: {}", e)).into_response();
            }
        };

        let filename = if meta.filename.ends_with(".zip") {
            meta.filename
        } else {
            format!("{}.zip", meta.filename)
        };

        let archive_base = if filename.ends_with(".zip") {
            filename[..filename.len() - 4].to_string()
        } else {
            filename.clone()
        };

        match pack_files_to_zip_with_split(
            &state,
            &access_token,
            jobs,
            &archive_base,
            state.config.web.max_zip_size,
        )
        .await
        {
            Ok(parts_list) => {
                if parts_list.is_empty() {
                    return (StatusCode::INTERNAL_SERVER_ERROR, "生成的 ZIP 文件为空")
                        .into_response();
                }

                // 对于 download_handler，只返回第一个 part（或单个 ZIP）
                // 如果有多个 part，用户需要使用 /api/zip 来获取完整信息
                let zip_bytes = parts_list.into_iter().next().unwrap();
                let mut resp = Response::new(Body::from(zip_bytes));
                *resp.status_mut() = StatusCode::OK;

                let headers = resp.headers_mut();
                headers.insert(
                    header::CONTENT_TYPE,
                    header::HeaderValue::from_static("application/zip"),
                );
                let cd_value = format!(
                    "attachment; filename=\"{}\"",
                    urlencoding::encode(&filename)
                );
                if let Ok(v) = header::HeaderValue::from_str(&cd_value) {
                    headers.insert(header::CONTENT_DISPOSITION, v);
                }
                return resp.into_response();
            }
            Err(e) => {
                warn!("❌ 文件夹 ZIP 打包失败: {}", e);
                return (StatusCode::BAD_GATEWAY, format!("ZIP 打包失败: {}", e)).into_response();
            }
        }
    }

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
