<div align="center">

# BaiduPCS-Relay-RS

[![CI](https://github.com/Develata/BaiduPCS-Relay-RS/actions/workflows/ci.yml/badge.svg)](https://github.com/Develata/BaiduPCS-Relay-RS/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)

将百度网盘分享链接转存到自己的网盘，并生成带本地签名的下载跳转链接。

</div>

## 工作流程

```text
分享链接
  -> 解析 surl / 提取码
  -> 验证分享并读取 fsid
  -> 转存到唯一 relay job 目录
  -> 递归枚举转存结果
  -> 生成 /d/download 本地签名链接
  -> 查询百度官方 dlink
  -> HTTP 302 跳转到百度 PCS 下载地址
```

Web 是交互壳层，分享解析、转存、OAuth、签名和下载跳转均由 Rust 服务端负责。

## 当前能力

- CLI 分享转存。
- Web 分享链接转直链。
- 文件和目录分享，目录结果保留相对路径。
- URL 内嵌提取码，例如 `...?pwd=9un1`；链接内提取码优先于单独填写值。
- 每次转换使用 `{SAVE_PATH}/.baidupcs-relay/{job_id}`，避免依赖“最新文件”或固定等待。
- 百度 OAuth authorization code 流程：OOB 手工授权码和 HTTP(S) callback。
- 内存 access/refresh token provider，并支持配置 refresh token 后按需刷新。
- HMAC 签名、过期时间和参数篡改校验。
- `/d/download` 只返回 302，不代理文件内容。
- Docker Compose release 构建、健康检查和非 root 运行。

### v1 非目标

- `/api/zip` 固定返回 HTTP 501 `zip_unsupported`。
- 服务不会自动删除已转存的 relay job；生产环境应按自己的保留策略清理。
- 百度分享页面和部分转存接口属于私有 Web 接口，页面或参数变化时可能需要更新适配层。

## 前置条件

- 百度网盘账号的 `BDUSS` 和 `STOKEN`，用于分享验证和转存。
- 百度开放平台应用，用于获取下载所需的 access token。
- 二选一运行环境：
  - Rust 1.91 或更新版本；
  - Docker Engine 与 Docker Compose。

百度开放平台 OAuth 流程见[官方授权文档](https://openauth.baidu.com/doc/doc.html)。

## 快速开始

### 从源码运行

```bash
git clone https://github.com/Develata/BaiduPCS-Relay-RS.git
cd BaiduPCS-Relay-RS

cp config.example.toml config.toml
# 编辑 config.toml

cargo run --bin baidu-web-server
```

打开 <http://127.0.0.1:5200>。服务启动后可检查：

```bash
curl --fail http://127.0.0.1:5200/health
```

预期响应：

```json
{"status":"ok","version":"1.1.0"}
```

### Docker Compose

```bash
cp .env.example .env
# 编辑 .env，填入 Cookie、服务密钥和百度 OAuth 配置

docker compose config --quiet
docker compose up -d --build
docker compose ps
curl --fail http://127.0.0.1:5200/health
```

默认仅绑定宿主机 `127.0.0.1:5200`。可通过 `BIND_ADDRESS` 和 `HOST_PORT` 修改。
容器内端口固定为 `5200`。

```bash
# 查看日志
docker compose logs -f app

# 停止并删除容器与 Compose 网络
docker compose down
```

镜像使用多阶段 release 构建。运行容器采用数值非 root 用户、只读根文件系统、
`cap_drop: ALL` 和 `no-new-privileges`。健康检查由服务二进制自身执行，不依赖运行时安装
curl 或其他系统包。

默认不设置代理。确有需要时，在 `.env` 中填写 `HTTP_PROXY` 和 `HTTPS_PROXY`；Docker
Desktop 访问宿主机代理通常使用 `host.docker.internal`。

## 发布与容器镜像

普通分支 push 和 Pull Request 只运行 CI，不会发布 Release 或容器镜像。只有推送格式为
`vMAJOR.MINOR.PATCH` 的 tag 才会触发发布工作流，且 tag 版本必须与 `Cargo.toml` 中的包版本
完全一致。

发布产物包括：

- GitHub Release：Linux x86_64 压缩包，内含 Web/CLI 二进制、README、配置示例和 MIT LICENSE；
- `SHA256SUMS`：Release 二进制包的 SHA-256 校验值；
- GHCR 镜像：`linux/amd64` 与 `linux/arm64` 多架构镜像；
- 镜像标签：完整版本、主次版本、主版本和稳定版 `latest`。

发布前先更新 `Cargo.toml` 版本并确认普通 CI 通过，然后创建并单独推送 tag：

```bash
git tag -a v1.1.0 -m 'Release v1.1.0'
git push origin v1.1.0
```

拉取稳定版镜像：

```bash
docker pull ghcr.io/develata/baidupcs-relay-rs:latest
```

首次发布 GHCR package 后，需要在 GitHub Packages 设置中确认其可见性为 Public。

## 配置

配置文件默认为 `config.toml`，环境变量覆盖 TOML。可通过 `CONFIG_PATH` 指定其他文件。

### TOML

```toml
[baidu]
cookie_bduss = "YOUR_BDUSS"
cookie_stoken = "YOUR_STOKEN"
save_path = "/我的资源"
http_timeout_secs = 120

[web]
access_token = "replace-with-a-strong-service-token"
sign_secret = "replace-with-a-long-random-signing-secret"

[baidu_open]
client_id = ""
client_secret = ""
redirect_uri = "oob"
refresh_token = ""
access_token = ""
```

Web 启动会拒绝空值以及默认的 `change-me` / `change-me-sign`。

### 环境变量

| 变量 | 用途 |
| --- | --- |
| `CONFIG_PATH` | TOML 配置路径 |
| `BDUSS` / `STOKEN` | 百度账号 Cookie |
| `SAVE_PATH` | 转存根目录 |
| `HTTP_TIMEOUT_SECS` | 百度 HTTP 请求超时 |
| `PORT` | Web 容器内/进程监听端口 |
| `WEB_ACCESS_TOKEN` | 本服务 Bearer token |
| `WEB_SIGN_SECRET` | 本地下载链接 HMAC 密钥 |
| `BAIDU_ACCESS_TOKEN` | 静态百度 access token |
| `BAIDU_REFRESH_TOKEN` | 百度 refresh token |
| `BAIDU_CLIENT_ID` | 百度开放平台 AppKey |
| `BAIDU_CLIENT_SECRET` | 百度开放平台 SecretKey |
| `BAIDU_REDIRECT_URI` | `oob` 或登记的 HTTP(S) 回调地址 |
| `HTTP_PROXY` / `HTTPS_PROXY` | 可选出站代理 |

Web 下载至少需要以下一种 token 来源：

1. `BAIDU_ACCESS_TOKEN`；
2. `BAIDU_REFRESH_TOKEN + BAIDU_CLIENT_ID + BAIDU_CLIENT_SECRET`；
3. `BAIDU_CLIENT_ID + BAIDU_CLIENT_SECRET + BAIDU_REDIRECT_URI`，启动后从前端授权。

## OAuth 授权

### OOB 模式

本地部署推荐：

```bash
BAIDU_REDIRECT_URI=oob
```

打开 Web 页面并点击“授权百度网盘”。完成百度授权后，将页面显示的一次性 authorization
code 粘贴回本地页面。授权码和服务端生成的 `flow_id` 都只能使用一次。

### HTTP callback

```bash
BAIDU_REDIRECT_URI=http://127.0.0.1:5200/oauth/callback
```

该地址必须与百度开放平台安全设置中登记的回调地址完全一致。回调使用一次性 `state`
防止请求伪造。

Client Secret 仅参与 Rust 服务端 code/token 交换，不写入 HTML，也不会从凭据接口返回。
前端授权得到的 token 默认只保存在当前进程；需要跨重启使用时，应安全持久化 refresh token。

## API

除 `/health`、`/oauth/callback` 和签名后的 `/d/download` 外，API 均要求：

```http
Authorization: Bearer <WEB_ACCESS_TOKEN>
```

请求体中的 `token` 字段仅为旧客户端兼容，推荐使用 Bearer header。

### 转换分享链接

```bash
curl -X POST http://127.0.0.1:5200/api/convert \
  -H 'Authorization: Bearer your-service-token' \
  -H 'Content-Type: application/json' \
  -d '{
    "link": "https://pan.baidu.com/s/1xxxxx?pwd=1234",
    "pwd": ""
  }'
```

如果 `link` 含非空 `pwd`，服务端忽略请求体或 CLI 参数中的提取码，使用链接内值。

```json
{
  "success": true,
  "items": [
    {
      "fsid": 123456,
      "filename": "dir/file.mp4",
      "download_url": "/d/download?fsid=...&expires=...&filename=...&sign=...",
      "expires": 1234567890
    }
  ],
  "transfer_job": "job-id"
}
```

### 下载

`download_url` 先验证本地签名，再解析百度 dlink，最后返回 HTTP 302。百度 dlink 请求和
下载客户端应使用：

```http
User-Agent: pan.baidu.com
```

命令行示例：

```bash
curl --fail --location \
  --user-agent 'pan.baidu.com' \
  --output file.bin \
  'http://127.0.0.1:5200/d/download?...'
```

签名默认有效期由服务端工作流设置；过期或修改 `fsid`、`filename`、`expires` 会被拒绝。

### OAuth 管理

| 方法 | 路径 | 说明 |
| --- | --- | --- |
| `POST` | `/api/oauth/start` | 创建一次性 state/flow 并返回授权 URL |
| `POST` | `/api/oauth/exchange` | OOB 模式提交 authorization code |
| `GET` | `/api/oauth/status` | 查询 OAuth 配置和 token 状态 |
| `GET` | `/api/oauth/credentials` | 读取本进程授权结果；响应敏感 |
| `GET` | `/oauth/callback` | HTTP callback 入口 |

### 其他接口

| 方法 | 路径 | 结果 |
| --- | --- | --- |
| `GET` | `/health` | JSON 健康状态 |
| `POST` | `/api/zip` | HTTP 501 `zip_unsupported` |

## CLI

CLI 只执行分享转存，不生成 Web 签名链接：

```bash
cargo run --bin baidu-direct-link -- \
  'https://pan.baidu.com/s/1xxxxx?pwd=1234'
```

也可以单独传入提取码和配置路径：

```bash
./baidu-direct-link '<share-url>' '[pwd]' '[/path/to/config.toml]'
```

Docker 中运行 CLI：

```bash
docker compose run --rm --no-deps \
  --entrypoint /usr/local/bin/baidu-direct-link \
  app 'https://pan.baidu.com/s/1xxxxx?pwd=1234'
```

## Relay 目录

每次 Web 转换创建：

```text
{SAVE_PATH}/.baidupcs-relay/{job_id}
```

服务只枚举该 job，避免混入并发任务或保存目录中的旧文件。转换成功后不会自动删除百度
网盘中的 job；请根据业务保留周期，通过百度客户端或官方文件管理 API 清理。

## 安全

- `BDUSS`、`STOKEN`、Client Secret、access/refresh token 都是敏感凭据。
- `.env`、`.env.e2e` 和 `config.toml` 已被 Git 忽略，但仍应设置为仅当前用户可读。
- 不要将服务直接暴露到公网；默认 Compose 仅监听 `127.0.0.1`。
- `/api/oauth/credentials` 会返回 access/refresh token，只应在可信网络中使用。
- 使用足够长且互不相同的 `WEB_ACCESS_TOKEN` 与 `WEB_SIGN_SECRET`。
- 日志不得记录 Cookie、提取码、bdstoken、sekey 或 token。

## 验证

默认测试不依赖真实百度账号：

```bash
cargo fmt --all -- --check
cargo test
cargo clippy --all-targets --all-features -- -D warnings
```

真实账号测试必须显式提供凭据，并在隔离的百度网盘目录中运行。详细步骤见
[TEST_GUIDE.md](TEST_GUIDE.md)。

## 故障排查

### Web 启动失败并提示默认密钥

设置非默认的 `WEB_ACCESS_TOKEN` 和 `WEB_SIGN_SECRET`。

### 分享提取码错误

检查 URL 是否已包含 `pwd`。URL 内非空值优先于表单、CLI 或 API 请求体中的值。

### Cookie 或权限错误

重新登录百度网盘并更新 `BDUSS`、`STOKEN`。不要在 issue 或日志中粘贴完整 Cookie。

### 下载命中防盗链

确认下载客户端使用 `User-Agent: pan.baidu.com`，并重新请求本地签名 URL 以获得新的百度
dlink。

### Docker 容器不健康

```bash
docker compose ps
docker compose logs --tail 100 app
docker compose exec app /usr/local/bin/baidu-web-server --healthcheck
```

不要默认硬编码宿主机代理；只有确认容器无法直接出站时才配置代理。

## 项目结构

```text
src/bin/web_server.rs   Web/API 入口和内置 healthcheck
src/direct_link.rs      分享转直链工作流
src/baidupcs/           百度 API 适配层
src/signing.rs          本地下载链接签名
templates/index.html    Web 交互壳层
Dockerfile              多阶段 release 镜像
docker-compose.yml      本地安全部署基线
```

变更记录见 [CHANGELOG.md](CHANGELOG.md)。

## 免责声明

- 本项目仅供学习、研究和个人自动化使用。
- 使用者必须遵守百度网盘服务条款和所在地法律法规。
- 不得用于传播违法、侵权内容或绕过平台访问控制。
- 百度私有接口可能随时变化，使用风险由使用者自行承担。

## License

本项目采用 [MIT License](LICENSE) 开源。
