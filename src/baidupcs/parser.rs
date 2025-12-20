//! 链接解析（仅保留转存所需）

/// 从分享链接中提取 surl
///
/// 支持：
/// - https://pan.baidu.com/s/1xxxx
/// - https://pan.baidu.com/share/init?surl=xxxx
/// - ...?surl=xxxx
pub fn extract_surl(share_url: &str) -> Option<String> {
    let url = share_url.trim();

    // 如果是完整 URL，优先解析并确保主机是 baidu.com，否则视为无效
    if let Ok(parsed) = url::Url::parse(url) {
        if !parsed
            .host_str()
            .is_some_and(|h| h.contains("baidu.com"))
        {
            return None;
        }
    }

    if let Some(pos) = url.find("/s/") {
        let start = pos + 3;
        if start >= url.len() {
            return None;
        }

        let surl = &url[start..];
        let end = surl
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .unwrap_or(surl.len());

        if end > 0 {
            return Some(surl[..end].to_string());
        }
    }

    if let Some(pos) = url.find("surl=") {
        let start = pos + 5;
        if start >= url.len() {
            return None;
        }

        let surl = &url[start..];
        let end = surl
            .find(|c: char| !c.is_alphanumeric() && c != '_' && c != '-')
            .unwrap_or(surl.len());

        if end > 0 {
            return Some(surl[..end].to_string());
        }
    }

    None
}
