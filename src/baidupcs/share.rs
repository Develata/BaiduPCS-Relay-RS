//! 百度网盘分享链接解析模块
//!
//! 参考 baidupcs-go 实现

use anyhow::{anyhow, Result};
use serde::{Deserialize, Deserializer};
use tracing::{debug, info, warn};

use super::types::ShareFileInfo;
use crate::config::Config;
use crate::AppState;

#[derive(Debug, Deserialize)]
struct ListResponse {
    errno: i32,
    #[serde(default)]
    list: Vec<FileItem>,
}

#[derive(Debug, Deserialize)]
struct FileItem {
    // 百度接口字段名可能是 fs_id 或 fsid，且值可能是字符串或数字
    #[serde(rename = "fs_id", alias = "fsid", deserialize_with = "string_or_u64")]
    fs_id: u64,
    #[serde(default)]
    server_filename: String,
}

/// 自定义反序列化：支持字符串或数字类型的 fsid
fn string_or_u64<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: Deserializer<'de>,
{
    use serde::de::Error;

    #[derive(Deserialize)]
    #[serde(untagged)]
    enum StringOrU64 {
        Str(String),
        Num(u64),
    }

    match StringOrU64::deserialize(deserializer)? {
        StringOrU64::Str(s) => s.parse().map_err(Error::custom),
        StringOrU64::Num(n) => Ok(n),
    }
}

impl FileItem {
    fn get_fsid(&self) -> u64 {
        self.fs_id
    }
}

/// 获取分享链接信息
pub async fn get_share_info(
    state: &AppState,
    _share_url: &str,
    surl: &str,
    pwd: &str,
) -> Result<ShareFileInfo> {
    info!("📥 获取分享信息: surl={}", surl);

    // Step 1: 访问分享页面
    let surl_param = surl.strip_prefix('1').unwrap_or(surl);
    let init_url = format!("https://pan.baidu.com/share/init?surl={}", surl_param);

    info!("🌐 访问分享页面: {}", init_url);

    let resp = state
        .client
        .get(&init_url)
        .header("User-Agent", Config::browser_ua())
        .send()
        .await?;

    let html = resp.text().await?;
    debug!("📄 页面长度: {} 字节", html.len());

    // Step 2: 提取 shareid 和 uk
    let (shareid, uk) = extract_share_ids(&html)?;
    debug!("✅ 提取到: shareid={}, uk={}", shareid, uk);

    // Step 3: 提取 bdstoken
    let bdstoken = extract_bdstoken(&html);
    debug!("🔑 bdstoken: {}", bdstoken);

    // Step 4: 验证提取码
    if !pwd.is_empty() {
        info!("🔐 验证提取码...");
        verify_password(state, surl_param, pwd, &bdstoken).await?;
        info!("✅ 提取码验证成功");
    }

    // Step 5: 获取文件列表
    info!("📋 获取文件列表...");
    let (fs_ids, filenames) = get_file_list(state, &shareid, &uk, surl_param, &bdstoken).await?;

    if fs_ids.is_empty() {
        return Err(anyhow!("未找到可转存的文件"));
    }

    info!("✅ 找到 {} 个文件", fs_ids.len());
    for (i, name) in filenames.iter().enumerate() {
        info!("  {}. {}", i + 1, name);
    }

    Ok(ShareFileInfo {
        shareid,
        uk,
        fs_ids,
        bdstoken,
        filenames,
    })
}

/// 验证提取码
async fn verify_password(state: &AppState, surl: &str, pwd: &str, bdstoken: &str) -> Result<()> {
    // 更贴近浏览器/baidupcs-go：verify 的大部分参数在 query string，表单仅提交 pwd/vcode。
    // 少带或带错参数/请求头时，百度有时也会返回 errno=-12（看起来像“提取码错误”）。
    let ts_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis();
    let url = format!(
        "https://pan.baidu.com/share/verify?surl={}&t={}&channel=chunlei&web=1&app_id=250528&clienttype=0&bdstoken={}",
        surl,
        ts_ms,
        bdstoken
    );

    let form = [("pwd", pwd), ("vcode", ""), ("vcode_str", "")];

    debug!("🔐 提取码验证: surl={}", surl);

    let resp = state
        .client
        .post(url)
        .header("User-Agent", Config::browser_ua())
        .header(
            "Referer",
            format!("https://pan.baidu.com/share/init?surl={}", surl),
        )
        .header("Origin", "https://pan.baidu.com")
        .header("X-Requested-With", "XMLHttpRequest")
        .form(&form)
        .send()
        .await?;

    let text = resp.text().await?;
    debug!("🔑 verify 响应: {}", text);

    #[derive(Deserialize)]
    struct VerifyResponse {
        errno: i32,
        #[serde(default)]
        err_msg: String,
        #[serde(default)]
        request_id: u64,
    }

    let result: VerifyResponse = serde_json::from_str(&text)
        .map_err(|e| anyhow!("解析 verify 响应失败: {} (body={})", e, text))?;

    if result.errno != 0 {
        let hint = match result.errno {
            -12 => "提取码错误，或验证请求被百度拒绝（常见于参数/请求头不符合预期、风控/频控）",
            -20 => "验证次数过多，请稍后再试",
            _ => "验证失败",
        };
        return Err(anyhow!(
            "{} (errno={}, request_id={}, err_msg={})",
            hint,
            result.errno,
            result.request_id,
            result.err_msg
        ));
    }

    Ok(())
}

/// 获取文件列表
///
/// 调用 share/list API 获取分享链接中的所有文件
async fn get_file_list(
    state: &AppState,
    shareid: &str,
    uk: &str,
    surl: &str,
    bdstoken: &str,
) -> Result<(Vec<u64>, Vec<String>)> {
    let url = format!(
        "https://pan.baidu.com/share/list?shareid={}&uk={}&shorturl={}&root=1&dir=%2F&page=1&num=1000&order=name&desc=1&showempty=0&web=1&channel=chunlei&clienttype=0&bdstoken={}",
        shareid, uk, surl, bdstoken
    );

    debug!("📡 调用 list API: {}", url);

    let resp = state
        .client
        .get(&url)
        .header("User-Agent", Config::browser_ua())
        .header(
            "Referer",
            format!("https://pan.baidu.com/share/init?surl={}", surl),
        )
        .send()
        .await?;

    let text = resp.text().await?;
    debug!("📨 list 响应: {}", &text[..200.min(text.len())]);

    let res: ListResponse =
        serde_json::from_str(&text).map_err(|e| anyhow!("解析响应失败: {}", e))?;

    if res.errno != 0 {
        warn!("⚠️ list API errno: {}", res.errno);
        let error_msg = match res.errno {
            -7 => "分享链接已过期或被删除",
            -9 => "提取码错误",
            105 => "分享链接不存在",
            110 => "分享链接已失效",
            _ => "未知错误",
        };
        return Err(anyhow!(
            "获取文件列表失败: errno={}, {}",
            res.errno,
            error_msg
        ));
    }

    let mut fs_ids = Vec::new();
    let mut filenames = Vec::new();

    for file in res.list {
        fs_ids.push(file.get_fsid());
        filenames.push(file.server_filename);
    }

    Ok((fs_ids, filenames))
}

/// 从 HTML 中提取 shareid 和 uk
fn extract_share_ids(html: &str) -> Result<(String, String)> {
    use regex::Regex;
    use std::sync::OnceLock;

    // share/init 页面里可能出现多个 shareid/uk，取“数字最长”的那个，避免误抓到很小的数字（如 5）
    static SHAREID_RE: OnceLock<Regex> = OnceLock::new();
    let shareid_re = SHAREID_RE.get_or_init(|| Regex::new(r"shareid\D*?(\d+)").unwrap());
    let shareid = shareid_re
        .captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .max_by_key(|s| s.len())
        .ok_or_else(|| anyhow!("无法提取 shareid，页面格式可能已变化"))?;

    static UK_RE: OnceLock<Regex> = OnceLock::new();
    let uk_re = UK_RE.get_or_init(|| Regex::new(r"uk\D*?(\d+)").unwrap());
    let uk = uk_re
        .captures_iter(html)
        .filter_map(|c| c.get(1).map(|m| m.as_str().to_string()))
        .max_by_key(|s| s.len())
        .ok_or_else(|| anyhow!("无法提取 uk"))?;

    Ok((shareid, uk))
}

/// 从 HTML 中提取 bdstoken
fn extract_bdstoken(html: &str) -> String {
    use regex::Regex;
    use std::sync::OnceLock;

    // 匹配 bdstoken 后面的 32 位十六进制字符
    static BDSTOKEN_RE: OnceLock<Regex> = OnceLock::new();
    let re = BDSTOKEN_RE.get_or_init(|| Regex::new(r"bdstoken\D*?([a-f0-9]{32})").unwrap());
    re.captures(html)
        .and_then(|c| c.get(1))
        .map(|m| m.as_str().to_string())
        .unwrap_or_else(|| "null".to_string())
}
