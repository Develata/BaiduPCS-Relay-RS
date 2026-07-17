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

#[derive(Clone, Copy)]
pub struct TransferRequest<'a> {
    pub shareid: &'a str,
    pub uk: &'a str,
    pub fs_ids: &'a [u64],
    pub bdstoken: &'a str,
    pub sekey: &'a str,
    pub surl: &'a str,
}

pub async fn create_remote_dir(state: &AppState, path: &str) -> Result<()> {
    let access_token = crate::baidupcs::download::get_or_refresh_access_token(state).await?;
    let url = create_dir_url(&access_token);
    let form = create_dir_form(path);

    let resp = state
        .oauth_client
        .post(&url)
        .header("User-Agent", Config::browser_ua())
        .form(&form)
        .send()
        .await?;
    let text = resp.text().await?;
    debug!("create dir 响应: {}", text);

    #[derive(Deserialize)]
    struct CreateResponse {
        errno: i32,
        #[serde(default)]
        errmsg: String,
    }

    let result: CreateResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 create 响应失败: {}, body={}", e, text))?;

    match result.errno {
        0 | -8 => Ok(()),
        errno => {
            debug!(errno, message = %result.errmsg, "百度官方创建目录失败");
            Err(map_baidu_errno(errno, "create_remote_dir").into())
        }
    }
}

fn create_dir_url(access_token: &str) -> String {
    format!(
        "https://pan.baidu.com/rest/2.0/xpan/file?method=create&access_token={}",
        urlencoding::encode(access_token)
    )
}

fn create_dir_form(path: &str) -> [(&'static str, &str); 3] {
    [("path", path), ("isdir", "1"), ("rtype", "0")]
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

pub async fn transfer_files(state: &AppState, request: TransferRequest<'_>) -> Result<()> {
    transfer_files_to_path(state, request, &state.config.baidu.save_path).await
}

pub async fn transfer_files_to_path(
    state: &AppState,
    request: TransferRequest<'_>,
    savepath: &str,
) -> Result<()> {
    if request.fs_ids.is_empty() {
        return Err(anyhow!("转存 fs_id 列表不能为空"));
    }
    if !verify_save_path(state, savepath).await? {
        return Err(anyhow!("保存路径不存在: {}", savepath));
    }

    let url = transfer_url(request.shareid, request.uk, request.bdstoken, request.sekey);
    let fsidlist = serde_json::to_string(request.fs_ids)?;
    let params = transfer_form(&fsidlist, savepath);
    let referer = format!("https://pan.baidu.com/s/{}", request.surl);

    debug!("预访问 referer 页面: {}", referer);
    let _ = state
        .client
        .get(&referer)
        .header("User-Agent", Config::browser_ua())
        .send()
        .await;

    info!(
        "发送转存请求: {} 个文件 -> {}",
        request.fs_ids.len(),
        savepath
    );
    let resp = state
        .client
        .post(&url)
        .header("User-Agent", Config::browser_ua())
        .header("Referer", &referer)
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

fn transfer_url(shareid: &str, uk: &str, bdstoken: &str, sekey: &str) -> String {
    let mut url = reqwest::Url::parse("https://pan.baidu.com/share/transfer")
        .expect("static transfer URL must be valid");
    url.query_pairs_mut()
        .append_pair("shareid", shareid)
        .append_pair("from", uk)
        .append_pair("ondup", "newcopy")
        .append_pair("async", "1")
        .append_pair("channel", "chunlei")
        .append_pair("web", "1")
        .append_pair("app_id", "250528")
        .append_pair("clienttype", "0")
        .append_pair("bdstoken", bdstoken);
    if !sekey.is_empty() {
        url.query_pairs_mut().append_pair("sekey", sekey);
    }
    url.into()
}

fn transfer_form<'a>(fsidlist: &'a str, savepath: &'a str) -> [(&'static str, &'a str); 2] {
    [("fsidlist", fsidlist), ("path", savepath)]
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
    request: TransferRequest<'_>,
    savepath: &str,
) -> Result<Vec<u64>> {
    transfer_files_to_path(state.as_ref(), request, savepath).await?;
    Ok(request.fs_ids.to_vec())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn create_directory_uses_official_oauth_request_shape() {
        let url = reqwest::Url::parse(&create_dir_url("token value")).unwrap();
        let query = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            query.get("method").map(|value| value.as_ref()),
            Some("create")
        );
        assert_eq!(
            query.get("access_token").map(|value| value.as_ref()),
            Some("token value")
        );
        assert!(!query.contains_key("bdstoken"));
        assert!(!query.contains_key("path"));

        let form = create_dir_form("/relay/job")
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(form.get("path"), Some(&"/relay/job"));
        assert_eq!(form.get("isdir"), Some(&"1"));
        assert_eq!(form.get("rtype"), Some(&"0"));
    }

    #[test]
    fn transfer_uses_current_web_request_shape() {
        let url = reqwest::Url::parse(&transfer_url(
            "share-id",
            "owner-uk",
            "bd-token",
            "share-key=",
        ))
        .unwrap();
        let query = url
            .query_pairs()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(
            query.get("shareid").map(|value| value.as_ref()),
            Some("share-id")
        );
        assert_eq!(
            query.get("from").map(|value| value.as_ref()),
            Some("owner-uk")
        );
        assert_eq!(
            query.get("sekey").map(|value| value.as_ref()),
            Some("share-key=")
        );
        assert_eq!(query.get("async").map(|value| value.as_ref()), Some("1"));
        assert_eq!(
            query.get("app_id").map(|value| value.as_ref()),
            Some("250528")
        );

        let fsids = serde_json::to_string(&[1_u64, 2]).unwrap();
        let form = transfer_form(&fsids, "/relay/job")
            .into_iter()
            .collect::<std::collections::HashMap<_, _>>();
        assert_eq!(form.get("fsidlist"), Some(&"[1,2]"));
        assert_eq!(form.get("path"), Some(&"/relay/job"));
    }
}
