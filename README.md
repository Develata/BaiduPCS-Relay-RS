<div align="center">

# BaiduPCS-Relay-RS

[![CI](https://github.com/Develata/BaiduPCS-Relay-RS/actions/workflows/ci.yml/badge.svg)](https://github.com/Develata/BaiduPCS-Relay-RS/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)

百度网盘分享链接转直链服务：支持分享链接转存、Web 服务器、文件打包下载。

</div>

---

## 项目说明

- 本项目为学习/研究性质的 Rust 工具，提供百度网盘分享链接处理功能
- 使用你自己的百度账号 Cookie（BDUSS/STOKEN）在本地发起请求
- 请自行评估并遵守百度网盘相关服务条款

## 功能特性

### CLI 模式
- ✅ 支持带/不带提取码的分享链接
- ✅ 自动拉取分享列表并发起转存
- ✅ 可配置转存保存路径与 HTTP 超时

### Web 服务器模式
- ✅ 分享链接转换为直链
- ✅ 文件/文件夹自动打包为 ZIP
- ✅ 支持大文件分卷下载（可配置大小限制）
- ✅ 密码保护的 API 接口
- ✅ 自动递归展开文件夹

### 通用特性
- ✅ 支持 Docker / Podman 运行（从源码构建启动）
- ✅ 详细的日志输出
- ✅ 安全的签名验证

## 快速开始

### 方式一：从 Release 下载（二进制）

1. 下载对应平台的二进制：https://github.com/Develata/BaiduPCS-Relay-RS/releases

> 当前 Release 提供的预编译二进制以 Linux x86_64 为主；其他平台请使用“从源码编译”或 Docker 方式运行。

2. 创建配置文件 `config.toml`：

```toml
[baidu]
cookie_bduss = "你的BDUSS"
cookie_stoken = "你的STOKEN"
save_path = "/我的资源"
http_timeout_secs = 120

[web]
access_token = "your-secret-password"
sign_secret = "your-sign-secret"

[baidu_open]
client_id = ""
client_secret = ""
redirect_uri = ""
refresh_token = ""
access_token = ""
```

3. 运行 CLI 模式（分享转存）：

```bash
./baidu-direct-link-linux-x86_64 "https://pan.baidu.com/s/1xxxxx" "提取码(可选)"
```

4. 运行 Web 服务器模式：

```bash
./baidu-web-server-linux-x86_64
# 服务启动在 http://localhost:5200
```

### 方式二：从源码编译

```bash
git clone https://github.com/Develata/BaiduPCS-Relay-RS.git
cd BaiduPCS-Relay-RS

cargo build --release

cp config.example.toml config.toml
# 编辑 config.toml 填入你的 Cookie

# CLI 模式
./target/release/baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "提取码(可选)"

# Web 服务器模式
./target/release/baidu-web-server
```

## 配置说明

配置文件默认读取当前目录的 `config.toml`。

```toml
[baidu]
# 必填：百度网盘 BDUSS（建议从浏览器 Cookie 原样复制）
cookie_bduss = "YOUR_BDUSS"

# 必填：百度网盘 STOKEN
cookie_stoken = "YOUR_STOKEN"

# 必填：转存保存路径（网盘目录，需要你提前创建）
save_path = "/我的资源"

# 可选：HTTP 请求超时时间（秒）- 推荐 120-300，避免大文件下载超时
http_timeout_secs = 120

[web]
# Web 服务器访问密码（调用 API 时作为 token 传入）
access_token = "your-secret-password"

# 签名密钥（用于生成下载链接签名）
sign_secret = "your-sign-secret"

# 可选：ZIP 打包大小限制（字节）。超过会按 1GB/分卷进行拆分
# max_zip_size = 1073741824

[baidu_open]
# 可选：百度开放平台（如未使用可留空）
client_id = ""
client_secret = ""
redirect_uri = ""
refresh_token = ""
access_token = ""
```

### CLI 模式（分享转存）

```bash
./baidu-direct-link <分享链接> [提取码] [配置文件路径]

# 无提取码
./baidu-direct-link "https://pan.baidu.com/s/1xxxxx"

# 有提取码
./baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "1234"

# 指定配置文件路径
./baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "1234" "/path/to/config.toml"
```

#### 批量转存（脚本示例）

```bash
#!/usr/bin/env bash
set -euo pipefail

items=(
  "https://pan.baidu.com/s/1xxxx|1234"
  "https://pan.baidu.com/s/1yyyy|5678"
  "https://pan.baidu.com/s/1zzzz|"
)

for item in "${items[@]}"; do
  IFS='|' read -r link pwd <<< "$item"
  echo "转存: $link"
  ./baidu-direct-link "$link" "$pwd"
  sleep 2
done
```

### Web 服务器模式

启动服务器：

```bash
./baidu-web-server
# 服务启动在 http://localhost:5200
```

#### API 接口

**1. 分享链接转直链**

```bash
POST /api/convert
Content-Type: application/json

{
  "link": "https://pan.baidu.com/s/1xxxxx",
  "pwd": "提取码(可选)",
  "token": "your-secret-password"
}
```

响应：
```json
{
  "success": true,
  "links": [
    {
      "filename": "文件名.pdf",
      "download_url": "/d/download?fsid=xxx&sign=xxx&expires=xxx&filename=xxx"
    }
  ]
}
```

**2. 文件/文件夹打包为 ZIP**

```bash
POST /api/zip
Content-Type: application/json

{
  "fsids": [123456789],
  "archive_name": "archive",
  "token": "your-secret-password"
}
```

响应（小文件）：
- 直接返回 ZIP 文件流

响应（大文件，超过 `MAX_ZIP_SIZE`）：
```json
{
  "success": true,
  "total_parts": 3,
  "total_size": 3221225472,
  "parts": [
    {
      "part_num": 1,
      "filename": "archive.z01",
      "size_bytes": 1073741824
    },
    {
      "part_num": 2,
      "filename": "archive.z02",
      "size_bytes": 1073741824
    },
    {
      "part_num": 3,
      "filename": "archive.z03",
      "size_bytes": 1073741824
    }
  ],
  "message": "文件超过大小限制，已分卷。请分别下载各个 part 文件。"
}
```

**3. 健康检查**

```bash
GET /health
```

详细使用说明见 [TEST_GUIDE.md](TEST_GUIDE.md)。

## Docker 运行

仓库提供 [docker-compose.yml](docker-compose.yml) 用于在容器中从源码启动服务（适合本地开发/快速试跑）。

1) 准备配置：

```bash
cp config.example.toml config.toml
# 编辑 config.toml 填入你的 Cookie
```

2) 启动 Web 服务器：

```bash
docker compose up --build
# 服务启动在 http://localhost:5200
```

3) 在容器中运行 CLI（一次性）：

```bash
docker compose run --rm app bash -lc "apt-get update && apt-get install -y pkg-config libssl-dev && cargo run --bin baidu-direct-link -- 'https://pan.baidu.com/s/1xxxxx' '1234'"
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
- ✅ 预编译二进制：Linux x86_64（见 Release）
- ✅ 从源码编译：Rust 支持的平台（取决于本地 Rust 工具链与依赖）
- ✅ Docker / Podman：使用本仓库的 docker-compose 从源码运行

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