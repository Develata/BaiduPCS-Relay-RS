//! v1 暂不支持服务器端 ZIP 打包。

fn main() {
    eprintln!("v1 暂不支持服务器端 ZIP 打包；/api/zip 会返回 501 zip_unsupported。");
    eprintln!("请使用 /api/convert 获取单文件签名下载链接。");
}
