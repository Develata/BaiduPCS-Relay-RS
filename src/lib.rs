//! 百度网盘转存（精简版）

pub mod baidupcs;
pub mod config;
pub mod state;

pub use config::Config;
pub use state::AppState;

/// 库版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION");  // ✅ 修复：移除了反斜杠
