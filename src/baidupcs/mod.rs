//! 百度网盘 PCS API 模块

pub mod download;
pub mod openapi;
pub mod parser;
pub mod share;
pub mod transfer;
pub mod types;

// 导出常用函数
pub use download::{
    get_download_links,
    share_to_direct_link,
    list_directory_fsids,
    list_directory_files,
    get_download_link_by_fsid_internal,
    zip_fsids_to_bytes,
    get_fsid_meta,
    zip_directory_by_path_to_bytes,
    expand_fsids_to_file_jobs,
};

pub use openapi::refresh_token;
pub use parser::extract_surl;
pub use share::get_share_info;
pub use transfer::transfer_files;
pub use types::ShareFileInfo;
