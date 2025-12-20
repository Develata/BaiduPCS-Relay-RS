//! 链接解析（仅保留转存所需）

/// 从分享链接中提取 surl
///
/// 支持：
/// - https://pan.baidu.com/s/1xxxx
/// - https://pan.baidu.com/share/init?surl=xxxx
/// - ...?surl=xxxx
pub fn extract_surl(share_url: &str) -> Option<String> {
    let url = share_url.trim();

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
