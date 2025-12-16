use anyhow::{anyhow, Result};
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use baidu_direct_link::{
    config::Config,
    baidupcs,
    AppState,
};

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "baidu_direct_link=debug".into()),
        )
        .with(tracing_subscriber::fmt::layer())
        .init();

    tracing::info!("🚀 百度网盘转存工具启动中...");

    // 用法：baidu-direct-link <share_url> [pwd] [config_path]
    let args: Vec<String> = std::env::args().collect();
    if args.len() < 2 {
        return Err(anyhow!(
            "用法: {} <share_url> [pwd] [config_path]",
            args.get(0).map(|s| s.as_str()).unwrap_or("baidu-direct-link")
        ));
    }

    let share_url = args[1].clone();
    let pwd = args.get(2).cloned().unwrap_or_default();
    let config_path = args.get(3).map(|s| s.as_str()).unwrap_or("config.toml");

    // 加载配置
    let config = Config::load(config_path)?;
    tracing::info!("✅ 配置加载完成: {}", config_path);

    // 初始化应用状态（仅 Cookie + HTTP client）
    let state = Arc::new(AppState::new(config)?);
    tracing::info!("✅ HTTP Client 初始化完成");

    // 提取 surl
    let surl = baidupcs::extract_surl(&share_url)
        .ok_or_else(|| anyhow!("无法从链接中提取 surl: {}", share_url))?;

    // 1) 获取分享信息
    let info = baidupcs::get_share_info(&state, &share_url, &surl, &pwd).await?;
    tracing::info!("📦 获取到 {} 个文件，开始转存...", info.fs_ids.len());

    // 2) 转存
    baidupcs::transfer_files(
        &state,
        &info.shareid,
        &info.uk,
        &info.fs_ids,
        &info.bdstoken,
        &surl,
    )
    .await?;

    tracing::info!("✅ 转存请求已完成，保存路径: {}", state.config.baidu.save_path);
    Ok(())
}

