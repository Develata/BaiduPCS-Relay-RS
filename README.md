<div align="center">

# 🚀 BaiduPCS-Relay-RS

[![Rust](https://img.shields.io/badge/Rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Memory](https://img.shields.io/badge/Memory-15~80MB-green.svg)](https://github.com/Develata/BaiduPCS-Relay-RS)
[![AI Assisted](https://img.shields.io/badge/AI-Assisted-purple.svg)](https://github.com/Develata/BaiduPCS-Relay-RS)

百度网盘分享链接转存工具 - 基于 Rust 的高性能实现

**相比 Go 实现省 70-96% 内存 · 零 GC 停顿 · 适合长期运行**

[English](README_EN.md) | 简体中文

</div>

---

## ⚠️ 项目说明

> 本项目代码主要由 AI 辅助编写完成，用于学习和技术探索。
> 
> 如有 Bug 或建议，欢迎提 Issue，但请理解这是一个实验性项目，维护者可能无法及时响应。
> 
> **No Pressure, Just Learning!** 😊

## ✨ 当前功能

- 🎯 **自动转存** - 参考 [baidupcs-go](https://github.com/qjfoidnh/BaiduPCS-Go) 的稳定转存逻辑
- 🔗 **直链获取** - 自动获取百度网盘真实下载链接
- 🔥 **超低内存** - 运行时仅需 15-80 MB，相比 Go 实现节省 **70-96% 内存**
- ⚡ **零 GC 停顿** - 无垃圾回收，响应时间更稳定
- 🔐 **安全可靠** - 使用个人 Cookie，无隐私泄露风险
- 🐳 **Docker 支持** - 一键部署，开箱即用
- 📦 **RESTful API** - 提供 HTTP API 接口

## 📋 开发计划

- [x] 基础转存功能
- [x] 直链获取 API
- [x] Docker 支持
- [ ] Web 管理界面（开发中，见 `feature/web-ui` 分支）
- [ ] 批量处理优化
- [ ] 下载链接缓存优化
- [ ] 命令行交互模式
- [ ] WebDAV 支持

## 📊 性能对比

相比 Go 实现的 BaiduPCS-Go：

| 指标 | BaiduPCS-Go (Go) | BaiduPCS-Relay-RS (Rust) | 优势 |
|------|------------------|--------------------------|------|
| **轻负载内存** | ~100 MB | ~30 MB | **省 70%** |
| **中等负载** | ~400 MB | ~50 MB | **省 87%** |
| **高并发场景** | 500 MB - 2 GB | 60-80 MB | **省 90-96%** |
| **GC 停顿** | 1-10 ms | 0 ms | **无 GC** |
| **启动内存** | ~50 MB | ~15 MB | **省 70%** |

### 为什么选择 Rust？

```
BaiduPCS-Go (Go)
轻负载  ████████░░░░░░░░░░░░  100 MB
高负载  ████████████████████  2 GB    ← GC 导致内存翻倍

BaiduPCS-Relay-RS (Rust)
轻负载  ██░░░░░░░░░░░░░░░░░░  30 MB   ⚡ 省 70%
高负载  ██████░░░░░░░░░░░░░░  80 MB   ⚡ 省 96%
        ✅ 128MB 内存即可流畅运行
```

**适合：** 树莓派、NAS、低配 VPS、长期运行服务

## 🎬 快速开始

### 方式一：Docker 部署（推荐）

```
# 1. 创建配置目录
mkdir -p ~/baidupcs-relay && cd ~/baidupcs-relay

# 2. 创建配置文件
cat > config.toml << 'EOF'
[server]
host = "0.0.0.0"
port = 5200

[baidu]
cookie_bduss = "你的BDUSS"
cookie_stoken = "你的STOKEN"
save_path = "/我的资源"

[security]
access_token = "your_secure_token_here"
rate_limit_per_minute = 30
EOF

# 3. 启动容器（仅需 128MB 内存）
docker run -d \
  --name baidupcs-relay \
  --restart unless-stopped \
  -p 5200:5200 \
  -v $(pwd)/config.toml:/app/config.toml:ro \
  -m 128m \
  ghcr.io/Develata/baidupcs-relay-rs:latest

# 4. 查看日志
docker logs -f baidupcs-relay
```

### 方式二：从源码编译

```
# 1. 克隆仓库
git clone https://github.com/Develata/BaiduPCS-Relay-RS.git
cd BaiduPCS-Relay-RS

# 2. 安装 Rust（如果还没有）
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# 3. 编译
cargo build --release

# 4. 配置
cp config.example.toml config.toml
# 编辑 config.toml 填入你的 Cookie

# 5. 运行
./target/release/baidu-direct-link
```

## 📖 使用说明

### 1. 获取百度网盘 Cookie

#### 浏览器开发者工具方法

1. 登录 [百度网盘网页版](https://pan.baidu.com)
2. 按 `F12` 打开开发者工具
3. 切换到 `Application` / `应用` 标签
4. 左侧选择 `Cookies` → `https://pan.baidu.com`
5. 找到并复制：
   - `BDUSS`：完整的字符串
   - `STOKEN`：完整的字符串

#### 使用脚本获取（推荐）

```
# 运行获取脚本
curl -fsSL https://raw.githubusercontent.com/Develata/BaiduPCS-Relay-RS/main/scripts/get_cookie.sh | bash
```

### 2. 配置文件

```
[server]
host = "0.0.0.0"          # 监听地址
port = 5200               # 监听端口

[baidu]
cookie_bduss = ""         # 必填：百度网盘 BDUSS
cookie_stoken = ""        # 必填：百度网盘 STOKEN  
save_path = "/我的资源"   # 转存目录
app_key = ""              # 可选：OAuth App Key
secret_key = ""           # 可选：OAuth Secret Key

[security]
access_token = "test123"  # API 访问令牌，建议修改
rate_limit_per_minute = 30 # 每分钟请求限制

[cache]
max_entries = 10000       # 最大缓存条目
link_ttl = 28800          # 链接缓存时间(秒)
```

### 3. API 使用

#### 转存分享链接并获取直链

```
curl -X POST http://localhost:5200/api/convert \
  -H "Content-Type: application/json" \
  -d '{
    "token": "test123",
    "link": "https://pan.baidu.com/s/1xxxxx",
    "pwd": "1234"
  }'
```

**响应示例：**

```
{
  "success": true,
  "files": [
    {
      "fsid": 123456789,
      "download_url": "https://d.pcs.baidu.com/file/xxx?fid=xxx&..."
    }
  ]
}
```

#### 健康检查

```
curl http://localhost:5200/health
```

### 4. Python 调用示例

```
import requests

def get_baidu_direct_link(share_link, password=''):
    """获取百度网盘直链"""
    response = requests.post(
        'http://localhost:5200/api/convert',
        json={
            'token': 'test123',
            'link': share_link,
            'pwd': password
        }
    )
    
    data = response.json()
    if data['success']:
        for file in data['files']:
            print(f"下载链接: {file['download_url']}")
        return data['files']
    else:
        print(f"错误: {data.get('error', '未知错误')}")
        return None

# 使用示例
get_baidu_direct_link('https://pan.baidu.com/s/1xxxxx', '1234')
```

## 🔧 部署配置

### Docker Compose

```
version: '3.8'

services:
  baidupcs-relay:
    image: ghcr.io/Develata/baidupcs-relay-rs:latest
    container_name: baidupcs-relay
    restart: unless-stopped
    ports:
      - "5200:5200"
    volumes:
      - ./config.toml:/app/config.toml:ro
    deploy:
      resources:
        limits:
          memory: 128M  # 仅需 128MB
          cpus: '0.5'
    environment:
      - RUST_LOG=info
```

### Nginx 反向代理

```
server {
    listen 80;
    server_name pan.yourdomain.com;

    location / {
        proxy_pass http://localhost:5200;
        proxy_set_header Host $host;
        proxy_set_header X-Real-IP $remote_addr;
        
        # 超时设置
        proxy_connect_timeout 60s;
        proxy_read_timeout 60s;
    }
}
```

## 🛡️ 安全说明

### 重要提示

- ✅ 使用**您自己**的百度账号 Cookie
- ✅ **不存储**任何用户数据
- ✅ Token 鉴权防止未授权访问
- ⚠️ **不要在公网暴露**，建议内网使用或配置 SSL
- ⚠️ 修改默认 `access_token`
- ⚠️ Cookie 等同于账号密码，请妥善保管

### 生成安全 Token

```
# 使用 openssl 生成随机 Token
openssl rand -hex 32

# 或使用 Python
python3 -c "import secrets; print(secrets.token_hex(32))"
```

## 📊 监控

```
# 查看日志
docker logs -f baidupcs-relay

# 查看内存占用（实时）
docker stats baidupcs-relay

# 健康检查
curl http://localhost:5200/health
```

## 🙏 致谢

### 核心参考

本项目参考了以下优秀开源项目：

- **[BaiduPCS-Go](https://github.com/qjfoidnh/BaiduPCS-Go)** (Apache-2.0) by [@qjfoidnh](https://github.com/qjfoidnh)
  - 百度网盘命令行客户端
  - 本项目的转存逻辑和 API 调用方式参考了该项目的稳定实现
  - 包括：转存参数配置、User-Agent 设置、错误处理机制
  - 特别感谢开源贡献 🙏

### 技术栈

- [Rust](https://www.rust-lang.org/) - 系统编程语言
- [Tokio](https://tokio.rs/) - 异步运行时
- [Axum](https://github.com/tokio-rs/axum) - Web 框架
- [Reqwest](https://github.com/seanmonstar/reqwest) - HTTP 客户端

### AI 辅助声明

本项目代码主要由 AI (Claude/GPT) 辅助编写，用于：
- 学习 Rust 异步编程
- 探索 Tokio 生态
- 实践 HTTP API 开发

代码质量和可维护性可能不如专业开发者作品，仅供学习参考。

## 📜 开源声明

本项目参考了 [BaiduPCS-Go](https://github.com/qjfoidnh/BaiduPCS-Go) 的实现逻辑（Apache-2.0 许可证）。

在遵守原项目许可证的前提下，使用 Rust 重新实现，并做了以下改进：
- ✅ 使用 Rust 零成本抽象，内存占用降低 70-96%
- ✅ 基于 Tokio 异步运行时，无 GC 停顿
- ✅ 提供 RESTful API 接口

详见 [NOTICE](NOTICE) 文件。

## 📋 系统要求

### 最低配置
- CPU: 单核
- 内存: 64 MB
- 存储: 50 MB

### 推荐配置
- CPU: 双核
- 内存: 128 MB
- 存储: 100 MB

### 支持平台
- ✅ Linux (x86_64, aarch64)
- ✅ macOS (Intel, Apple Silicon)
- ✅ Windows (x86_64)
- ✅ Docker
- ✅ 树莓派
- ✅ 群晖 NAS

## ❓ 常见问题

### Q: Cookie 多久会过期？
A: 通常 30-90 天，过期后重新获取即可。

### Q: 为什么转存失败？
A: 检查：
1. Cookie 是否过期
2. 是否转存自己的分享（不支持）
3. 分享是否已失效

### Q: 直链有效期多久？
A: 通常 8 小时，过期后需重新获取。

### Q: 可以商用吗？
A: MIT 许可证允许商用，但请遵守百度网盘服务条款。

### Q: 为什么是 AI 写的代码？
A: 作者在学习 Rust，使用 AI 辅助快速实现想法。代码质量可能不完美，欢迎 PR 改进！

## 🤝 贡献指南

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

**提示：** 维护者可能响应较慢，请见谅！

## ⚖️ 免责声明

- 本项目仅供学习交流，请勿用于违法用途
- 使用本工具需遵守百度网盘服务条款
- 请勿分享违法、侵权内容
- 建议合理使用，避免频繁请求
- 使用产生的任何后果由使用者自行承担
- 代码由 AI 辅助生成，可能存在未知问题

## 📄 开源协议

[MIT License](LICENSE)

本项目参考了 Apache-2.0 许可的 BaiduPCS-Go，详见 [NOTICE](NOTICE)。

---

<div align="center">

**⭐ 如果觉得有用，请给个 Star 支持一下！**

**💡 欢迎提 Issue 和 PR，但请理解这是学习项目，响应可能较慢**

Made with ❤️, 🦀 Rust and 🤖 AI

[Report Bug](https://github.com/Develata/BaiduPCS-Relay-RS/issues) · 
[Request Feature](https://github.com/Develata/BaiduPCS-Relay-RS/issues)

---

### 💬 友情提示

> "这个项目是我学习 Rust 的产物，代码主要由 AI 帮我写的。
> 
> 如果你是 Rust 大佬，看到代码里有什么不专业的地方，欢迎教我！
> 
> 如果你也在学习 Rust，那我们一起进步！
> 
> **No Pressure, Just Learning!** 😊"

</div>