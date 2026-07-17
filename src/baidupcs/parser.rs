//! 链接解析（仅保留转存所需）

use reqwest::Url;

use super::types::ShareInput;

/// 解析完整分享输入。URL 内非空 `pwd` 的优先级高于单独传入的提取码。
pub fn parse_share_input(share_url: &str, fallback_password: &str) -> Option<ShareInput> {
    let original_url = share_url.trim().to_string();
    let surl = extract_surl(&original_url)?;
    let password =
        extract_url_password(&original_url).unwrap_or_else(|| fallback_password.trim().to_string());

    Some(ShareInput {
        original_url,
        surl,
        password,
    })
}

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

    let parsed = Url::parse(url).ok()?;
    if !parsed.host_str().is_some_and(is_baidu_host) {
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

fn extract_url_password(value: &str) -> Option<String> {
    let parsed = Url::parse(value).ok()?;
    if !parsed.host_str().is_some_and(is_baidu_host) {
        return None;
    }

    parsed
        .query_pairs()
        .find(|(key, value)| key == "pwd" && !value.trim().is_empty())
        .map(|(_, value)| value.trim().to_string())
}

fn is_baidu_host(host: &str) -> bool {
    host == "baidu.com" || host.ends_with(".baidu.com")
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
        assert_eq!(extract_surl("https://notbaidu.com/s/1abc123"), None);
        assert_eq!(extract_surl("not a url"), None);
        assert_eq!(extract_surl("not-a-url"), None);
    }

    #[test]
    fn url_password_overrides_form_password() {
        let input = parse_share_input(
            "https://pan.baidu.com/s/1abc123?pwd=9un1",
            "wrong-form-value",
        )
        .unwrap();

        assert_eq!(input.surl, "1abc123");
        assert_eq!(input.password, "9un1");
    }

    #[test]
    fn decodes_url_password_and_falls_back_when_missing_or_empty() {
        let encoded =
            parse_share_input("https://pan.baidu.com/s/1abc123?pwd=%39un1", "fallback").unwrap();
        assert_eq!(encoded.password, "9un1");

        let missing = parse_share_input("https://pan.baidu.com/s/1abc123", " fallback ").unwrap();
        assert_eq!(missing.password, "fallback");

        let empty = parse_share_input("https://pan.baidu.com/s/1abc123?pwd=", "fallback").unwrap();
        assert_eq!(empty.password, "fallback");
    }
}
