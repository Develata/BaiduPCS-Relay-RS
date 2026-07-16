use crate::baidupcs;
use crate::baidupcs::types::{PanEntry, TransferJob};
use crate::error::AppError;
use crate::signing::{generate_signed_download, SignedDownload};
use crate::AppState;
use std::time::{SystemTime, UNIX_EPOCH};
use tracing::{debug, info};

#[derive(Debug, Clone)]
pub struct ConvertShareCommand {
    pub link: String,
    pub pwd: String,
    pub ttl_secs: u64,
}

#[derive(Debug, Clone)]
pub struct ConvertShareResult {
    pub items: Vec<SignedDownload>,
    pub transfer_job: TransferJob,
}

pub async fn convert_share_to_signed_downloads(
    state: &AppState,
    command: ConvertShareCommand,
) -> Result<ConvertShareResult, AppError> {
    if command.link.trim().is_empty() {
        return Err(AppError::bad_request(
            "empty_share_link",
            "分享链接不能为空",
        ));
    }

    let surl = baidupcs::extract_surl(&command.link)
        .ok_or_else(|| AppError::bad_request("invalid_share_link", "无法从分享链接中提取 surl"))?;
    let share = baidupcs::get_share_info(state, &command.link, &surl, &command.pwd)
        .await
        .map_err(classify_share_error)?;

    let transfer_job = build_transfer_job(&state.config.baidu.save_path);
    let relay_root = relay_root(&state.config.baidu.save_path);
    baidupcs::transfer::create_remote_dir(state, &relay_root, &share.bdstoken)
        .await
        .map_err(classify_transfer_error)?;
    baidupcs::transfer::create_remote_dir(state, &transfer_job.target_dir, &share.bdstoken)
        .await
        .map_err(classify_transfer_error)?;

    baidupcs::transfer::transfer_files_to_path(
        state,
        &share.shareid,
        &share.uk,
        &share.fs_ids,
        &share.bdstoken,
        &surl,
        &transfer_job.target_dir,
    )
    .await
    .map_err(classify_transfer_error)?;

    let files = wait_for_transferred_files(state, &transfer_job.target_dir).await?;
    let items = sign_entries(&state.config.web.sign_secret, files, command.ttl_secs)?;

    info!(
        "分享转直链完成: job={}, files={}",
        transfer_job.id,
        items.len()
    );
    Ok(ConvertShareResult {
        items,
        transfer_job,
    })
}

fn sign_entries(
    sign_secret: &str,
    files: Vec<PanEntry>,
    ttl_secs: u64,
) -> Result<Vec<SignedDownload>, AppError> {
    if files.is_empty() {
        return Err(AppError::upstream(
            "transfer_result_empty",
            "转存完成后目标 relay 目录为空",
        ));
    }

    files
        .into_iter()
        .map(|file| generate_signed_download(sign_secret, file.fsid, &file.relative_path, ttl_secs))
        .collect()
}

async fn wait_for_transferred_files(
    state: &AppState,
    target_dir: &str,
) -> Result<Vec<PanEntry>, AppError> {
    let mut last_error = None;
    for attempt in 0..8 {
        match baidupcs::download::list_directory_entries_recursive(state, target_dir).await {
            Ok(files) if !files.is_empty() => return Ok(files),
            Ok(_) => {
                debug!("relay 目录暂为空: {}, attempt={}", target_dir, attempt + 1);
            }
            Err(e) => {
                last_error = Some(e.to_string());
            }
        }
        tokio::time::sleep(std::time::Duration::from_millis(800)).await;
    }

    Err(AppError::upstream(
        "transfer_result_empty",
        last_error.unwrap_or_else(|| "转存后未在 relay 目录发现文件".to_string()),
    ))
}

fn build_transfer_job(save_path: &str) -> TransferJob {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let id = format!("{}-{}", now, std::process::id());
    let target_dir = format!("{}/{}", relay_root(save_path), id);
    TransferJob { id, target_dir }
}

fn relay_root(save_path: &str) -> String {
    format!("{}/.baidupcs-relay", save_path.trim_end_matches('/'))
}

fn classify_share_error(error: anyhow::Error) -> AppError {
    if let Some(app_error) = error.downcast_ref::<AppError>() {
        return app_error.clone();
    }

    let message = error.to_string();
    if message.contains("提取码") || message.contains("密码") {
        AppError::bad_request("share_password_invalid", message)
    } else if message.contains("失效") || message.contains("过期") || message.contains("不存在")
    {
        AppError::not_found("share_unavailable", message)
    } else if message.contains("Cookie") || message.contains("登录") {
        AppError::unauthorized("baidu_auth_failed", message)
    } else {
        AppError::upstream("share_info_failed", message)
    }
}

fn classify_transfer_error(error: anyhow::Error) -> AppError {
    if let Some(app_error) = error.downcast_ref::<AppError>() {
        return app_error.clone();
    }

    let message = error.to_string();
    if message.contains("路径") {
        AppError::bad_request("remote_path_missing", message)
    } else if message.contains("Cookie") || message.contains("登录") || message.contains("权限")
    {
        AppError::unauthorized("baidu_auth_failed", message)
    } else if message.contains("提取码") {
        AppError::bad_request("share_password_invalid", message)
    } else {
        AppError::upstream("transfer_failed", message)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_relay_job_under_save_path() {
        let job = build_transfer_job("/我的资源/");
        assert!(job.target_dir.starts_with("/我的资源/.baidupcs-relay/"));
        assert!(!job.id.is_empty());
    }

    #[test]
    fn maps_transfer_path_error() {
        let err = classify_transfer_error(anyhow::anyhow!("保存路径不存在"));
        assert_eq!(err.code(), "remote_path_missing");
    }

    #[test]
    fn preserves_structured_transfer_error() {
        let err = classify_transfer_error(crate::error::map_baidu_errno(-20, "create").into());
        assert_eq!(err.code(), "remote_path_missing");
    }

    #[test]
    fn signs_files_from_mocked_workflow_entries() {
        let entries = vec![
            PanEntry {
                fsid: 1,
                filename: "a.txt".to_string(),
                path: "/relay/job/a.txt".to_string(),
                relative_path: "a.txt".to_string(),
                is_dir: false,
                size: Some(1),
            },
            PanEntry {
                fsid: 2,
                filename: "b.txt".to_string(),
                path: "/relay/job/folder/b.txt".to_string(),
                relative_path: "folder/b.txt".to_string(),
                is_dir: false,
                size: Some(2),
            },
        ];

        let items = sign_entries("secret", entries, 60).unwrap();
        assert_eq!(items.len(), 2);
        assert_eq!(items[0].filename, "a.txt");
        assert_eq!(items[1].filename, "folder/b.txt");
        assert!(items[1].url.contains("filename=folder%2Fb.txt"));
    }
}
