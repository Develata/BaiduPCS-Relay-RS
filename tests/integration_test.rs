//! 集成测试

use baidu_direct_link::baidupcs;

#[test]
fn test_extract_surl_valid() {
    let test_cases = vec![
        ("https://pan.baidu.com/s/1abc123", Some(String::from("1abc123"))),
        ("https://pan.baidu.com/s/1xxxxx?pwd=1234", Some(String::from("1xxxxx"))),
        ("http://pan.baidu.com/s/1test", Some(String::from("1test"))),
    ];

    for (url, expected) in test_cases {
        let result = baidupcs::extract_surl(url);
        assert_eq!(result, expected, "Failed for URL: {}", url);
    }
}

#[test]
fn test_extract_surl_invalid() {
    let invalid_urls = vec![
        "https://example.com/s/1xxxxx",
        "not-a-url",
        "https://pan.baidu.com/other/path",
        "",
    ];

    for url in invalid_urls {
        let result = baidupcs::extract_surl(url);
        assert!(
            result.is_none(),
            "Should return None for invalid URL: {}",
            url
        );
    }
}

#[test]
fn test_basic() {
    assert_eq!(1 + 1, 2);
}
