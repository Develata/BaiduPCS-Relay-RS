//! 数据类型

#[derive(Clone)]
pub struct ShareFileInfo {
    pub shareid: String,
    pub uk: String,
    pub fs_ids: Vec<u64>,
    pub bdstoken: String,
    pub sekey: String,
    pub filenames: Vec<String>,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ShareInput {
    pub original_url: String,
    pub surl: String,
    pub password: String,
}

#[derive(Clone, PartialEq, Eq)]
pub struct ShareListing {
    pub shareid: String,
    pub uk: String,
    pub bdstoken: String,
    pub sekey: String,
    pub files: Vec<PanEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanEntry {
    pub fsid: u64,
    pub filename: String,
    pub path: String,
    pub relative_path: String,
    pub is_dir: bool,
    pub size: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TransferJob {
    pub id: String,
    pub target_dir: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DownloadTarget {
    pub fsid: u64,
    pub filename: String,
    pub dlink: String,
}

/// 百度 OAuth 授权结果。令牌只保存在内存中，不写回配置文件。
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthTokenSet {
    pub access_token: String,
    pub refresh_token: String,
    pub expires_at: u64,
    pub scope: String,
}
