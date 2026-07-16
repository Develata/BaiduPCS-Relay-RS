//! 链接解析（仅保留转存所需）

use reqwest::Url;

/// 从分享链接中提取 surl
///
/// 支持：
/// - https://pan.baidu.com/s/1xxxx
/// - https://pan.baidu.com/share/init?surl=xxxx
/// - ...?surl=xxxx
pub fn extract_surl(share_url: &str) -> Option<String> {
    let url = share_url.trim();
    if looks_like_surl(url) {
        return Some(url.to_string());
    }

    if let Ok(parsed) = Url::parse(url) {
        if !parsed.host_str().is_some_and(|h| h.ends_with("baidu.com")) {
            return None;
        }
    } else {
        return None;
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

fn looks_like_surl(value: &str) -> bool {
    if !(6..=128).contains(&value.len()) {
        return false;
    }

    let valid_chars = value
        .bytes()
        .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-');
    valid_chars && (value.starts_with('1') || value.bytes().all(|b| b.is_ascii_alphanumeric()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_surl_from_supported_links() {
        let cases = [
            ("https://pan.baidu.com/s/1abc123?pwd=9xyz", "1abc123"),
            ("https://pan.baidu.com/share/init?surl=abc123", "abc123"),
            (
                "https://pan.baidu.com/share/init?foo=bar&surl=abc-123_x",
                "abc-123_x",
            ),
            ("1abc123", "1abc123"),
            ("abc123", "abc123"),
        ];

        for (input, expected) in cases {
            assert_eq!(extract_surl(input).as_deref(), Some(expected));
        }
    }

    #[test]
    fn rejects_non_baidu_hosts() {
        assert_eq!(extract_surl("https://example.com/s/1abc123"), None);
        assert_eq!(extract_surl("not a url"), None);
        assert_eq!(extract_surl("not-a-url"), None);
    }
}
