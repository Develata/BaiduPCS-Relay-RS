//! 测试获取下载直链的独立工具
//!
//! 使用方法：
//! cargo run --bin test-direct-link -- <fs_id1> <fs_id2> ...
//! 或者自动从目录获取：
//! cargo run --bin test-direct-link -- --auto

use anyhow::Result;
use baidu_direct_link::{baidupcs, config::Config, AppState};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "baidu_direct_link=info".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    println!("\n🧪 百度网盘直链测试工具\n");

    let args: Vec<String> = std::env::args().collect();

    // 加载配置
    let config = Config::load("config.toml")?;
    println!("✅ 配置加载成功");
    println!("   BDUSS 长度: {}", config.baidu.cookie_bduss.len());
    println!("   STOKEN 长度: {}", config.baidu.cookie_stoken.len());
    println!();

    let state = Arc::new(AppState::new(config.clone())?);

    let fs_ids: Vec<u64> = if args.len() > 1 && args[1] == "--auto" {
        // 自动从目录获取
        println!("📁 自动从目录获取文件: {}", config.baidu.save_path);
        let all_ids =
            baidupcs::download::list_directory_fsids(state.as_ref(), &config.baidu.save_path)
                .await?;

        if all_ids.is_empty() {
            println!("⚠️  目录为空");
            return Ok(());
        }

        println!("✅ 找到 {} 个文件", all_ids.len());

        // 只测试前 3 个
        let test_count = 3.min(all_ids.len());
        println!("🎯 测试前 {} 个文件\n", test_count);

        all_ids.into_iter().take(test_count).collect()
    } else if args.len() > 1 {
        // 手动指定 fs_id
        println!("🎯 手动指定 fs_id:");
        args[1..]
            .iter()
            .filter_map(|s| s.parse::<u64>().ok())
            .inspect(|id| println!("   - {}", id))
            .collect()
    } else {
        println!("❌ 用法:");
        println!("   {} --auto                    # 自动从目录获取", args[0]);
        println!("   {} <fs_id1> <fs_id2> ...     # 手动指定\n", args[0]);
        println!("示例:");
        println!("   {} --auto", args[0]);
        println!("   {} 145167690140204 211466257985328", args[0]);
        return Ok(());
    };

    if fs_ids.is_empty() {
        println!("❌ 没有有效的 fs_id");
        return Ok(());
    }

    println!("\n🔗 开始获取下载直链...\n");

    // 调用获取直链函数
    match baidupcs::download::get_download_links(state.as_ref(), &fs_ids).await {
        Ok(links) => {
            println!("\n✅ 成功获取 {} 个下载链接:\n", links.len());
            println!("{}", "=".repeat(80));

            for (i, (filename, url)) in links.iter().enumerate() {
                println!("\n{}. 📄 {}", i + 1, filename);
                println!("   🔗 {}", url);
            }

            println!("\n{}", "=".repeat(80));
            println!("\n💡 可以用这些直链下载文件（需要带上 Cookie）\n");
        }
        Err(e) => {
            println!("\n❌ 获取直链失败: {}\n", e);
            println!("💡 可能原因:");
            println!("   1. fs_id 不正确");
            println!("   2. Cookie 权限不足");
            println!("   3. 需要会员权限");
            println!("   4. 百度 API 限制\n");
            return Err(e);
        }
    }

    Ok(())
}
