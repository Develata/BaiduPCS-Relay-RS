//! 数据类型

#[derive(Debug, Clone)]
pub struct ShareFileInfo {
    pub shareid: String,
    pub uk: String,
    pub fs_ids: Vec<u64>,
    pub bdstoken: String,
    pub filenames: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareInput {
    pub original_url: String,
    pub surl: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShareListing {
    pub shareid: String,
    pub uk: String,
    pub bdstoken: String,
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
