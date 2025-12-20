//! Web 服务器入口点

use anyhow::Result;
use std::sync::Arc;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

use baidu_direct_link::{config::Config, web, AppState};

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

    tracing::info!("🚀 百度网盘转存 Web 服务器启动中...");

    // 加载配置（支持环境变量 CONFIG_PATH 或命令行参数）
    let config_path = std::env::var("CONFIG_PATH")
        .ok()
        .or_else(|| std::env::args().nth(1))
        .unwrap_or_else(|| "config.toml".to_string());
    let config = Config::load(&config_path)?;
    if std::path::Path::new(&config_path).exists() {
        tracing::info!("✅ 配置加载完成: {}", config_path);
    } else {
        tracing::info!("✅ 配置从环境变量加载");
    }

    // 初始化应用状态
    let state = Arc::new(AppState::new(config)?);
    tracing::info!("✅ HTTP Client 初始化完成");

    // 创建路由
    let app = web::create_router(state);

    // 获取端口（默认 5200）
    let port = std::env::var("PORT")
        .ok()
        .and_then(|s| s.parse::<u16>().ok())
        .unwrap_or(5200);

    let addr = format!("0.0.0.0:{}", port);
    tracing::info!("🌐 Web 服务器启动在: http://{}", addr);
    tracing::info!("📝 请在浏览器中访问: http://localhost:{}", port);
    tracing::info!("💚 健康检查: http://localhost:{}/health", port);

    // 启动服务器
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    axum::serve(listener, app).await?;

    Ok(())
}

