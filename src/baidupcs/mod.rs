//! 百度网盘 PCS API 模块

pub mod download;
pub mod openapi;
pub mod parser;
pub mod share;
pub mod transfer;
pub mod types;

// 导出常用函数
pub use download::{
    expand_fsids_to_file_jobs, get_download_link_by_fsid, get_download_links, get_fsid_meta,
    list_directory_files, list_directory_fsids, share_to_direct_link,
    zip_directory_by_path_to_bytes, zip_fsids_to_bytes,
};

pub use openapi::{authorization_url, exchange_authorization_code, is_oob_redirect, refresh_token};
pub use parser::{extract_surl, parse_share_input};
pub use share::get_share_info;
pub use transfer::{transfer_files, TransferRequest};
pub use types::{
    DownloadTarget, OAuthTokenSet, PanEntry, ShareFileInfo, ShareInput, ShareListing, TransferJob,
};
