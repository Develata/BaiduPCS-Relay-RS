# BaiduPCS-Relay-RS v1.0.0 Release

## 🎉 新功能

### Web 服务器模式（新增）
- **分享链接转直链 API** (`/api/convert`) - 将百度网盘分享链接转换为可下载的直链
- **文件打包下载 API** (`/api/zip`) - 自动将文件/文件夹打包为 ZIP
- **大文件分卷支持** - 支持将大文件分割成多个 <1GB 的分卷
- **密码保护** - API 接口支持访问密码验证
- **签名链接** - 生成带有有效期的签名下载链接

### CLI 模式（原有功能）
- 分享链接转存
- 支持带/不带提取码
- 可配置保存路径

## 🚀 快速开始

### 方式一：使用二进制（推荐）

#### Linux / macOS / Windows (WSL)

```bash
# 下载二进制文件
wget https://github.com/Develata/BaiduPCS-Relay-RS/releases/download/v1.0.0/baidu-direct-link-linux-x86_64
chmod +x baidu-direct-link-linux-x86_64

# 配置
cp config.example.toml config.toml
# 编辑 config.toml，填入 BDUSS/STOKEN

# 运行 CLI 模式
./baidu-direct-link-linux-x86_64 "https://pan.baidu.com/s/1xxxxx" "提取码"

# 运行 Web 服务器
./baidu-web-server-linux-x86_64
```

#### macOS (Apple Silicon)

```bash
wget https://github.com/Develata/BaiduPCS-Relay-RS/releases/download/v1.0.0/baidu-direct-link-macos-aarch64
chmod +x baidu-direct-link-macos-aarch64
```

### 方式二：使用 Docker

```bash
docker run -d \
  -p 5200:5200 \
  -v $(pwd)/config.toml:/app/config.toml:ro \
  ghcr.io/develata/baidupcs-relay-rs:v1.0.0
```

### 方式三：从源码编译

```bash
git clone https://github.com/Develata/BaiduPCS-Relay-RS.git
cd BaiduPCS-Relay-RS
cargo build --release
```

## 📚 完整文档

- [README.md](https://github.com/Develata/BaiduPCS-Relay-RS/blob/main/README.md) - 项目说明
- [TEST_GUIDE.md](https://github.com/Develata/BaiduPCS-Relay-RS/blob/main/TEST_GUIDE.md) - API 测试指南

## 🔧 配置示例

```toml
[baidu]
cookie_bduss = "你的BDUSS"
cookie_stoken = "你的STOKEN"
save_path = "/我的资源"
http_timeout_secs = 120

[web]
access_token = "your-secret-password"
sign_secret = "your-sign-secret"
```

## 📋 二进制文件

本版本包含以下二进制文件：

| 平台 | 文件 | 大小 | 说明 |
|-----|------|------|------|
| Linux x86_64 | `baidu-direct-link-linux-x86_64` | 5.7M | CLI 模式 |
| Linux x86_64 | `baidu-web-server-linux-x86_64` | 7.0M | Web 服务器 |
| macOS Intel | `baidu-direct-link-macos-x86_64` | 5.5M | CLI 模式 |
| macOS Intel | `baidu-web-server-macos-x86_64` | 6.8M | Web 服务器 |
| macOS Apple Silicon | `baidu-direct-link-macos-aarch64` | 5.3M | CLI 模式 |
| macOS Apple Silicon | `baidu-web-server-macos-aarch64` | 6.6M | Web 服务器 |
| Windows x86_64 | `baidu-direct-link-windows-x86_64.exe` | 5.9M | CLI 模式 |
| Windows x86_64 | `baidu-web-server-windows-x86_64.exe` | 7.2M | Web 服务器 |

## ✨ 改进

### 性能优化
- 编译优化：LTO + 单编译单元 + 代码剥离
- 内存效率：使用 spawn_blocking 处理 CPU 密集操作
- 并发支持：基于 Tokio 异步运行时

### 代码质量
- 移除未使用依赖（async_zip, futures-lite）
- 代码格式化（cargo fmt）
- clippy 检查通过
- 单元测试通过

### 文档
- 添加 Web 服务器 API 文档
- 添加环境变量配置说明
- 添加测试指南
- 优化 README

## 🔒 安全

- SSL/TLS 支持
- 密码保护的 API 接口
- 签名验证的下载链接
- 敏感信息存储在本地配置文件

## 📊 系统要求

- **最低配置**: 单核 CPU，32MB 内存，10MB 存储
- **推荐配置**: 双核 CPU，64MB 内存，50MB 存储
- **支持平台**: Linux, macOS, Windows (WSL), Docker

## 🐛 已知问题

无已知问题。

## 🙏 致谢

感谢以下项目的启发：
- [BaiduPCS-Go](https://github.com/qjfoidnh/BaiduPCS-Go)

## 📝 许可证

MIT License - 详见 [LICENSE](LICENSE)

## 💬 反馈

- 提交 Issue：https://github.com/Develata/BaiduPCS-Relay-RS/issues
- 讨论：https://github.com/Develata/BaiduPCS-Relay-RS/discussions

## 📅 版本历史

### v1.0.0 (2025-12-19)
- 首次发布
- 支持 CLI 分享转存
- 支持 Web 服务器模式
- 支持文件分卷下载
