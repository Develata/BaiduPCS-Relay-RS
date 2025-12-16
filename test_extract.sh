#!/bin/bash

echo "=== 测试 shorturl 提取逻辑 ==="

cat > /tmp/test_surl.rs << 'EOF'
fn extract_surl(share_url: &str) -> Option<String> {
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

fn main() {
    let test_cases = vec![
        "https://pan.baidu.com/s/158pDc",
        "https://pan.baidu.com/s/1abcdefg",
        "pan.baidu.com/s/1xyz",
        "https://pan.baidu.com/share/init?surl=test123",
    ];

    for url in test_cases {
        match extract_surl(url) {
            Some(surl) => println!("✅ {} -> {}", url, surl),
            None => println!("❌ {} -> None", url),
        }
    }
}
EOF

cd /tmp
rustc test_surl.rs && ./test_surl
rm -f test_surl.rs test_surl
