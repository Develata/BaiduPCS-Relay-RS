//! 百度网盘转存（精简版）

pub mod baidupcs;
pub mod config;
pub mod direct_link;
pub mod error;
pub mod signing;
pub mod state;

pub use config::Config;
pub use error::AppError;
pub use signing::SignedDownload;
pub use state::AppState;

/// 库版本
pub const VERSION: &str = env!("CARGO_PKG_VERSION"); // ✅ 修复：移除了反斜杠
