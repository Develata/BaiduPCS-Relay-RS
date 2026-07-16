//! 百度网盘分享转存适配层

use anyhow::{anyhow, Result};
use serde::Deserialize;
use tracing::{debug, error, info, warn};

use crate::config::Config;
use crate::error::map_baidu_errno;
use crate::AppState;

#[derive(Debug, Deserialize)]
struct TransferResult {
    #[serde(default)]
    errno: i32,
    #[serde(default)]
    show_msg: String,
}

pub async fn create_remote_dir(state: &AppState, path: &str, bdstoken: &str) -> Result<()> {
    let url = format!(
        "https://pan.baidu.com/rest/2.0/xpan/file?method=create&path={}&isdir=1&bdstoken={}",
        urlencoding::encode(path),
        bdstoken
    );

    let resp = state
        .client
        .post(&url)
        .header("User-Agent", Config::browser_ua())
        .header("Referer", "https://pan.baidu.com/")
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .send()
        .await?;
    let text = resp.text().await?;
    debug!("create dir 响应: {}", text);

    #[derive(Deserialize)]
    struct CreateResponse {
        errno: i32,
        #[serde(default)]
        err_msg: Option<String>,
    }

    let result: CreateResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 create 响应失败: {}, body={}", e, text))?;

    match result.errno {
        0 | -8 => Ok(()),
        errno => {
            let _ = result.err_msg;
            Err(map_baidu_errno(errno, "create_remote_dir").into())
        }
    }
}

/// 验证保存路径是否存在。
pub async fn verify_save_path(state: &AppState, path: &str) -> Result<bool> {
    let url = format!(
        "https://pan.baidu.com/api/list?dir={}&num=1&order=name&desc=0",
        urlencoding::encode(path)
    );

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", Config::browser_ua())
        .send()
        .await?;
    let text = resp.text().await?;
    debug!("路径验证响应: {}", text);

    #[derive(Deserialize)]
    struct ApiListResponse {
        errno: i32,
    }

    let result: ApiListResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("路径验证响应解析失败: {}, body={}", e, text))?;
    Ok(result.errno == 0)
}

pub async fn transfer_files(
    state: &AppState,
    shareid: &str,
    uk: &str,
    fs_ids: &[u64],
    bdstoken: &str,
    surl: &str,
) -> Result<()> {
    transfer_files_to_path(
        state,
        shareid,
        uk,
        fs_ids,
        bdstoken,
        surl,
        &state.config.baidu.save_path,
    )
    .await
}

pub async fn transfer_files_to_path(
    state: &AppState,
    shareid: &str,
    uk: &str,
    fs_ids: &[u64],
    bdstoken: &str,
    surl: &str,
    savepath: &str,
) -> Result<()> {
    if fs_ids.is_empty() {
        return Err(anyhow!("转存 fs_id 列表不能为空"));
    }
    if !verify_save_path(state, savepath).await? {
        return Err(anyhow!("保存路径不存在: {}", savepath));
    }

    let url = format!(
        "https://pan.baidu.com/share/transfer?shareid={}&from={}&ondup=newcopy&channel=chunlei&clienttype=0&web=1&bdstoken={}",
        shareid, uk, bdstoken
    );
    let fsidlist = serde_json::to_string(fs_ids)?;
    let params = [("fsidlist", fsidlist.as_str()), ("path", savepath)];
    let surlparam = surl.strip_prefix('1').unwrap_or(surl);
    let referer = format!("https://pan.baidu.com/share/init?surl={}", surlparam);

    debug!("预访问 referer 页面: {}", referer);
    let _ = state
        .client
        .get(&referer)
        .header("User-Agent", Config::browser_ua())
        .send()
        .await;

    info!("发送转存请求: {} 个文件 -> {}", fs_ids.len(), savepath);
    let resp = state
        .client
        .post(&url)
        .header("User-Agent", Config::browser_ua())
        .header("Referer", &referer)
        .header("Host", "pan.baidu.com")
        .header("Origin", "https://pan.baidu.com")
        .header(
            "Content-Type",
            "application/x-www-form-urlencoded; charset=UTF-8",
        )
        .header("Accept", "application/json, text/javascript, */*; q=0.01")
        .header("Accept-Language", "zh-CN,zh;q=0.9,en;q=0.8")
        .header("X-Requested-With", "XMLHttpRequest")
        .form(&params)
        .send()
        .await?;

    let status = resp.status();
    let text = resp.text().await?;
    debug!("转存响应 status={}, body={}", status, text);
    let result: TransferResult = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析转存响应失败: {}, body={}", e, text))?;

    match result.errno {
        0 | 12 => Ok(()),
        2 if is_duplicate_message(&result.show_msg) => {
            warn!("百度返回重复转存: {}", result.show_msg);
            Ok(())
        }
        2 => {
            error!("百度登录态或权限异常: {}", result.show_msg);
            Err(anyhow!("Cookie失效或权限不足: {}", result.show_msg))
        }
        -7 | 110 => Err(map_baidu_errno(result.errno, "transfer").into()),
        -9 | -12 => Err(map_baidu_errno(result.errno, "transfer").into()),
        -20 => Err(map_baidu_errno(result.errno, "transfer").into()),
        errno => Err(anyhow!("转存失败: errno={}, {}", errno, result.show_msg)),
    }
}

fn is_duplicate_message(message: &str) -> bool {
    let msg = message.to_lowercase();
    msg.contains("已经保存过")
        || msg.contains("已存在")
        || msg.contains("重复转存")
        || msg.contains("duplicate")
}

pub async fn do_transfer(
    state: std::sync::Arc<AppState>,
    shareid: String,
    uk: String,
    fsids: Vec<u64>,
    bdstoken: String,
    surl: &str,
    savepath: &str,
) -> Result<Vec<u64>> {
    transfer_files_to_path(
        state.as_ref(),
        &shareid,
        &uk,
        &fsids,
        &bdstoken,
        surl,
        savepath,
    )
    .await?;
    Ok(fsids)
}
