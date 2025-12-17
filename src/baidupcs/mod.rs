//! 百度网盘 PCS 模块

pub mod download;  // ← 新增
pub mod openapi;
pub mod parser;
pub mod share;
pub mod transfer;
pub mod types;

pub use download::{get_download_links, get_download_link_by_path, share_to_direct_link};  // ← 新增
pub use openapi::get_open_download_link;
pub use parser::extract_surl;
pub use share::get_share_info;
pub use transfer::transfer_files;
pub use types::ShareFileInfo;
