//! 数据类型

#[derive(Debug, Clone)]
pub struct ShareFileInfo {
    pub shareid: String,
    pub uk: String,
    pub fs_ids: Vec<u64>,
    pub bdstoken: String,
    pub filenames: Vec<String>, // 文件名列表
}
