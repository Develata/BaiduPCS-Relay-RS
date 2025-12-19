//! 测试获取下载直链功能（含 Web 本地签名直链逻辑）
//!
//! 使用方法：
//! cargo test --test test_download -- --nocapture

use anyhow::Result;
use baidu_direct_link::{baidupcs, config::Config, AppState};
use std::sync::Arc;

#[tokio::test]
#[ignore] // 默认忽略，需要手动运行
async fn test_get_download_links() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter("baidu_direct_link=debug")
        .init();

    println!("\n🧪 测试获取下载直链功能（PCS 原始直链）\n");

    // 1. 加载配置
    println!("📋 加载配置文件...");
    let config = Config::load("config.toml")?;
    println!("✅ BDUSS 长度: {}", config.baidu.cookie_bduss.len());
    println!("✅ STOKEN 长度: {}", config.baidu.cookie_stoken.len());
    println!();

    let state = Arc::new(AppState::new(config)?);

    // 2. 列举你的网盘目录
    println!("📁 列举网盘目录: /我的资源");
    let fs_ids = baidupcs::download::list_directory_fsids(state.as_ref(), "/我的资源").await?;

    println!("✅ 找到 {} 个文件\n", fs_ids.len());

    if fs_ids.is_empty() {
        println!("⚠️  目录为空，请先转存一些文件");
        return Ok(());
    }

    // 3. 测试获取前 3 个文件的直链
    let test_count = 3.min(fs_ids.len());
    let test_fs_ids: Vec<u64> = fs_ids.iter().take(test_count).copied().collect();

    println!("🎯 测试获取 {} 个文件的直链", test_count);
    println!("fs_ids: {:?}\n", test_fs_ids);

    // 4. 调用获取直链函数
    match baidupcs::download::get_download_links(state.as_ref(), &test_fs_ids).await {
        Ok(links) => {
            println!("\n✅ 成功获取 {} 个下载链接:\n", links.len());
            for (i, (filename, url)) in links.iter().enumerate() {
                println!("{}. {}", i + 1, filename);
                println!("   {}\n", &url[..100.min(url.len())]);
            }
        }
        Err(e) => {
            println!("\n❌ 获取直链失败: {}\n", e);
            return Err(e);
        }
    }

    Ok(())
}

#[tokio::test]
#[ignore]
async fn test_single_file_download_link() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("baidu_direct_link=debug")
        .init();

    println!("\n🧪 测试单个文件直链（PCS 原始直链）\n");

    // 手动指定一个 fs_id 测试（从你的网盘中选一个）
    let test_fs_id: u64 = 145167690140204; // ⚠️ 改成你实际的 fs_id

    let config = Config::load("config.toml")?;
    let state = Arc::new(AppState::new(config)?);

    println!("🎯 测试 fs_id: {}\n", test_fs_id);

    match baidupcs::download::get_download_links(state.as_ref(), &[test_fs_id]).await {
        Ok(links) => {
            println!("✅ 成功:");
            for (filename, url) in links {
                println!("文件名: {}", filename);
                println!("直链: {}\n", url);
            }
        }
        Err(e) => {
            println!("❌ 失败: {}\n", e);
            return Err(e);
        }
    }

    Ok(())
}

/// 新增：只测试“本地签名直链”是否能成功生成（不真正访问 /d 路由）
#[tokio::test]
#[ignore]
async fn test_generate_signed_links_only() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter("baidu_direct_link=info")
        .init();

    println!("\n🧪 测试生成本地签名直链\n");

    // 1. 加载配置
    let config = Config::load("config.toml")?;
    let save_path = config.baidu.save_path.clone();
    let sign_secret = config.web.sign_secret.clone();

    println!("📁 save_path = {}", save_path);
    println!("🔑 sign_secret 长度 = {}", sign_secret.len());

    let state = Arc::new(AppState::new(config)?);

    // 2. 列举目录，取前 3 个文件，拿到它们的路径
    let fs_ids = baidupcs::download::list_directory_fsids(state.as_ref(), &save_path).await?;

    if fs_ids.is_empty() {
        println!("⚠️  目录为空，请先转存一些文件");
        return Ok(());
    }

    let test_count = 3.min(fs_ids.len());
    let test_fs_ids: Vec<u64> = fs_ids.iter().take(test_count).copied().collect();

    println!("🎯 准备为 {} 个文件生成本地签名直链", test_count);

    // 3. 通过已有逻辑拿到 (filename, 原始 PCS 直链)，这里只用 filename
    let links = baidupcs::download::get_download_links(state.as_ref(), &test_fs_ids).await?;

    for (filename, _url) in links.into_iter() {
        let full_path = format!("{}/{}", save_path.trim_end_matches('/'), filename);
        let local_link = crate_like_generate_signed_link_for_test(&sign_secret, &full_path, 3600);
        println!("📄 {}", filename);
        println!("   本地直链: {}\n", local_link);
    }

    Ok(())
}

/// 与 web_server 中的 generate_signed_link 保持一致，用于测试
fn crate_like_generate_signed_link_for_test(
    sign_secret: &str,
    pan_path: &str,
    ttl_secs: u64,
) -> String {
    use base64::engine::general_purpose::URL_SAFE_NO_PAD;
    use base64::Engine;
    use hmac::{Hmac, Mac};
    use sha2::Sha256;
    use std::time::{SystemTime, UNIX_EPOCH};

    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs();
    let expires = now + ttl_secs;

    let data = format!("{pan_path}:{expires}");

    let mut mac = Hmac::<Sha256>::new_from_slice(sign_secret.as_bytes())
        .expect("HMAC can take key of any size");
    mac.update(data.as_bytes());
    let result = mac.finalize().into_bytes();
    let sign = URL_SAFE_NO_PAD.encode(result);

    format!("/d{}?sign={sign}&expires={expires}", pan_path)
}
