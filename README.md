# 百度网盘分享链接转存 CLI

将百度网盘“分享链接”里的文件一键转存到你自己的网盘目录（依赖浏览器 Cookie：BDUSS/STOKEN）。

## 功能

- 分享链接解析：支持 `https://pan.baidu.com/s/xxxx`、`surl=` 等形式
- 可选提取码验证
- 获取分享文件列表并批量转存到指定目录
- 重复文件策略：`ondup=newcopy`（自动重命名生成副本）

## 配置

参考 [config.example.toml](config.example.toml)，创建你自己的 `config.toml`：


    [baidu]
    cookie_bduss = "YOUR_BDUSS"
    cookie_stoken = "YOUR_STOKEN"
    save_path = "/我的资源"
    http_timeout_secs = 30

注意：仓库默认在 [.gitignore](.gitignore) 里忽略了 `config.toml`，避免误提交敏感信息。

## 运行

本地运行：

    cargo run --release -- "https://pan.baidu.com/s/13beLs"

带提取码：

    cargo run --release -- "分享链接" "提取码"

指定配置文件路径：

    cargo run --release -- "分享链接" "提取码" "/path/to/config.toml"

## 免责声明

本项目仅用于学习与个人便捷使用，请遵守百度网盘及相关法律法规；请勿滥用、勿批量爬取、勿用于侵权用途。


## 免责声明
