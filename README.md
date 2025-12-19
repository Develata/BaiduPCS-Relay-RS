<div align="center">

# BaiduPCS-Relay-RS

[![CI](https://github.com/Develata/BaiduPCS-Relay-RS/actions/workflows/ci.yml/badge.svg)](https://github.com/Develata/BaiduPCS-Relay-RS/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)

百度网盘分享链接转存 CLI：把分享里的文件/文件夹转存到你自己的网盘目录。

</div>

---

## 项目说明

这是一个学习/研究性质的 Rust 命令行工具，用于将**百度网盘分享链接**中的文件/文件夹**转存**到你自己的网盘目录。

工作方式：你在本地运行程序，它会携带你自己的 BDUSS/STOKEN 去请求百度网盘相关接口。

请自行评估并遵守百度网盘相关服务条款。

## 功能特性

- 支持带/不带提取码的分享链接
- 自动拉取分享内文件列表并发起转存
- 可配置转存保存路径与 HTTP 超时
- 支持从源码运行 / Docker（从源码运行）

## 快速开始

### 方式一：从 Release 下载（二进制）

1) 下载 Release 中的二进制文件：

https://github.com/Develata/BaiduPCS-Relay-RS/releases

2) 准备配置文件（推荐从示例复制）：

```bash
cp config.example.toml config.toml
# 编辑 config.toml，填入 BDUSS / STOKEN
```

3) 运行：

```bash
./baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "提取码(可选)"
```

### 方式二：从源码编译运行

```bash
git clone https://github.com/Develata/BaiduPCS-Relay-RS.git
cd BaiduPCS-Relay-RS

cp config.example.toml config.toml
# 编辑 config.toml，填入 BDUSS / STOKEN

cargo run -- "https://pan.baidu.com/s/1xxxxx" "提取码(可选)"
```

（可选）编译 release：

```bash
cargo build --release
./target/release/baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "提取码(可选)"
```

## 配置说明

默认读取当前目录的 `config.toml`，也可以在命令行第 3 个参数指定路径。

配置示例见 config.example.toml；当前分支只需要以下字段：

```toml
[baidu]
cookie_bduss = "YOUR_BDUSS"
cookie_stoken = "YOUR_STOKEN"

# 转存保存路径（网盘目录，需要你提前创建）
save_path = "/我的资源"

# HTTP 请求超时（秒）
http_timeout_secs = 30
```

## 使用方法

```bash
baidu-direct-link <share_url> [pwd] [config_path]

# 无提取码
baidu-direct-link "https://pan.baidu.com/s/1xxxxx"

# 有提取码
baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "1234"

# 指定配置文件路径
baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "1234" "/path/to/config.toml"
```

## Docker 运行（从源码）

仓库提供 docker-compose.yml，用于在容器中直接 `cargo run`（更适合本地开发/快速试跑）。

```bash
cp config.example.toml config.toml
# 编辑 config.toml，填入 BDUSS / STOKEN

docker compose run --rm app bash -lc "cargo run -- 'https://pan.baidu.com/s/1xxxxx' '1234' 'config.toml'"
```

## 安全提示

- 请勿分享或提交 config.toml（包含敏感 Cookie）
- BDUSS/STOKEN 等同于账号凭证，请妥善保管
- 建议将配置权限设置为仅自己可读写：

```bash
chmod 600 config.toml
```

## 日志说明

### 正常运行示例

```
🚀 百度网盘转存工具启动中...
✅ 配置加载完成: config.toml
✅ HTTP Client 初始化完成
📥 获取分享信息: surl=158pDc
🌐 访问分享页面: https://pan.baidu.com/share/init?surl=58pDc
✅ 提取到: shareid=123456, uk=789012
🔑 bdstoken: abc123def456
📋 获取文件列表...
✅ 找到 1 个文件
  1. 示例文件.pdf
📦 开始转存 1 个文件...
🔍 验证保存路径: /我的资源
✅ 保存路径存在
🚀 发送转存请求...
✅ 转存成功! (errno=0)
📂 文件已保存至: /我的资源
```

### 常见错误

#### Cookie 失效/未登录
```
❌ errno=2 - Cookie 失效或未登录
📝 请检查 config.toml 中的:
   1. cookie_bduss (长度应为192字符)
   2. cookie_stoken (长度应为32字符)
```

**解决方法：** 重新获取 Cookie

#### 保存路径不存在
```
❌ 保存路径不存在 (errno=-20)
📝 当前路径: /我的资源
💡 请在百度网盘中创建该文件夹
```

**解决方法：** 在网盘中创建对应目录

#### 分享链接失效/被删除
```
❌ errno=-7 - 分享链接已过期或被删除
```

**解决方法：** 确认分享链接有效

## 致谢

### 核心参考

本项目参考了以下优秀开源项目：

- **[BaiduPCS-Go](https://github.com/qjfoidnh/BaiduPCS-Go)** (Apache-2.0) by [@qjfoidnh](https://github.com/qjfoidnh)
  - 百度网盘命令行客户端
  - 本项目的转存逻辑和 API 调用方式参考了该项目的实现
  - 包括：API 参数配置、User-Agent 设置、错误处理机制
  - 特别感谢开源贡献 🙏

### 技术栈

- [Rust](https://www.rust-lang.org/) - 系统编程语言
- [Tokio](https://tokio.rs/) - 异步运行时
- [Reqwest](https://github.com/seanmonstar/reqwest) - HTTP 客户端
- [Serde](https://serde.rs/) - 序列化框架

## 系统要求

### 最低配置
- CPU: 单核
- 内存: 32 MB
- 存储: 10 MB

### 推荐配置
- CPU: 双核
- 内存: 64 MB
- 存储: 50 MB

### 支持平台
- 运行二进制：取决于 Release 提供的构建产物
- 从源码编译：Rust 支持的平台（取决于本地工具链与依赖）
- Docker / Podman：使用 docker compose 在容器内从源码运行

## 常见问题

### Q: Cookie 在哪里获取？
A: 浏览器登录 pan.baidu.com → F12 → Application → Cookies → 复制 BDUSS 和 STOKEN

### Q: Cookie 多久会过期？
A: 通常 30-90 天，过期后重新获取即可。

### Q: 为什么提示"保存路径不存在"？
A: 需要在百度网盘中**先创建**对应文件夹，工具不会自动创建。

### Q: 支持批量转存吗？
A: 支持，可以写 Shell 脚本循环调用（见上面的批量转存脚本示例）。

### Q: 转存后文件在哪里？
A: 在 `config.toml` 中 `save_path` 指定的网盘目录下。

### Q: 为什么是 AI 写的代码？
A: 作者在学习 Rust，使用 AI 辅助快速实现想法。代码可能不完美，欢迎 PR 改进！

### Q: 可以商用吗？
A: MIT 许可证允许商用，但请遵守百度网盘服务条款。

### 常见错误码

| errno | 含义 | 解决方法 |
|-------|------|----------|
| 0 | 成功 | - |
| 2 | Cookie失效/路径错误 | 检查 Cookie 和路径 |
| 12 | 文件已存在 | 正常，表示转存成功 |
| -7 | 分享链接失效 | 检查链接是否有效 |
| -9 | 提取码错误 | 检查提取码 |
| -20 | 路径不存在 | 在网盘中创建目录 |
| 110 | 分享已过期 | 链接已失效 |

## 贡献指南

虽然代码主要由 AI 生成，但仍然欢迎贡献！

```
# 1. Fork 项目
# 2. 创建分支
git checkout -b feature/your-feature

# 3. 提交代码
git commit -m 'Add some feature'

# 4. 推送分支
git push origin feature/your-feature

# 5. 提交 Pull Request
```

### 代码规范

```
# 格式化代码
cargo fmt

# 代码检查
cargo clippy -- -D warnings

# 运行测试
cargo test
```

## 免责声明

- 本项目仅供学习交流，请勿用于违法用途
- 使用本工具需遵守百度网盘服务条款
- 请勿分享违法、侵权内容
- 建议合理使用，避免频繁请求
- 使用产生的任何后果由使用者自行承担
- 代码由 AI 辅助生成，可能存在未知问题

## 开源协议

[MIT License](LICENSE)