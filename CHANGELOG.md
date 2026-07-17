# Changelog

本项目的重要变更记录在此文件中。格式参考 [Keep a Changelog](https://keepachangelog.com/zh-CN/1.1.0/)，
版本号遵循 [Semantic Versioning](https://semver.org/lang/zh-CN/)。

## [Unreleased]

## [1.1.0] - 2026-07-17

### Added

- 新增分层的分享转直链工作流和领域对象。
- 新增唯一 relay job 目录，隔离并发转换和历史文件。
- 新增 HMAC 本地下载签名、过期校验和参数篡改校验。
- 新增百度 OAuth OOB 与 HTTP callback 两种 authorization code 流程。
- 新增内存 token provider、refresh token 刷新和失效重试。
- 新增结构化 `/api/convert`、OAuth 管理接口和 JSON `/health`。
- 新增 Web 授权与分享转换前端。
- 新增多阶段 Dockerfile、Compose 健康检查和 `.env.example`。
- 新增服务二进制 `--healthcheck`，运行镜像不再依赖 curl。
- 新增仅由 `vMAJOR.MINOR.PATCH` tag 触发的 GitHub Release 与 GHCR 发布工作流。
- 新增 Linux x86_64 发布包、SHA-256 校验文件及 amd64/arm64 多架构容器镜像。

### Changed

- Web 服务器成为 v1 主入口；CLI 保留分享转存能力。
- 创建远端目录改用百度开放平台 OAuth API。
- 下载改为官方 `filemetas + dlink + access_token` 流程。
- `/d/download` 明确返回 HTTP 302，不代理文件内容。
- 当前 Web 转存请求补齐 `sekey`、`async=1` 和 `app_id` 等参数。
- 分享 URL 内非空 `pwd` 优先于表单、CLI 或 API 请求体中的提取码。
- 环境变量覆盖统一到 Cookie、Web、OAuth、端口和超时配置。
- Docker 从源码挂载和启动时编译改为 release 镜像、非 root、只读根文件系统运行。
- 普通分支 push/PR 与发布流程分离；tag 发布前强制校验 Cargo 版本并重新执行质量门禁。

### Fixed

- 修复转存后固定 sleep 并读取保存目录“最新 N 项”导致的竞态和错配。
- 修复目录分享相对路径丢失。
- 修复 dlink 解析自动跟随重定向而无法取得最终 Location。
- 修复下载路由返回 307 而不是约定的 302。
- 修复分享验证得到的 `BDCLND/sekey` 未传入转存请求。
- 修复 `notbaidu.com` 等后缀碰撞域名被错误接受。
- 修复前端 OAuth popup 跨域窗口访问异常。
- 修复旧 Compose 硬编码代理、启动时 apt/cargo 和缺失运行配置的问题。
- 更新受影响的锁定依赖，消除 `cargo audit` 报告的 8 个已知漏洞。

### Security

- Web 启动拒绝默认服务 token 和签名密钥。
- OAuth state/flow 一次性消费并设置有效期和容量上限。
- Client Secret 不返回前端；OAuth/token 响应使用 `Cache-Control: no-store`。
- 移除 Cookie、提取码、bdstoken、sekey 和完整敏感 URL 的调试日志。
- Compose 默认仅绑定 `127.0.0.1`，丢弃全部 capabilities，并启用
  `no-new-privileges`。
- 扩大 Git 与 Docker 构建上下文的敏感文件忽略范围，覆盖本地环境、OAuth 凭据、私钥和证书。
- GitHub Actions 固定到已核验的提交 SHA，降低发布权限工作流的上游标签漂移风险。

### Removed

- v1 停止暴露全量内存 ZIP 和伪分卷行为；`/api/zip` 现在返回 HTTP 501。

### Verified

- 真实 OOB 授权码兑换、token 刷新和服务重启后的 token 来源可用。
- 真实分享转换生成 20 个签名下载项，下载路由 302 到百度 PCS。
- 使用 `User-Agent: pan.baidu.com` 成功下载真实对象的 50 MiB Range 到本地。
- Docker Compose release 镜像构建、健康检查、重启、真实转换和 302 下载链路通过。

## [1.0.1] - 2025-12-20

### Changed

- 优化分享链接解析代码格式。

## [1.0.0] - 2025-12-19

### Added

- 初始 CLI 分享转存能力。
- 初始 Web 服务器和下载链接能力。
- 初始 ZIP/分卷实验实现；该实现已在 1.1.0 中停用。

[Unreleased]: https://github.com/Develata/BaiduPCS-Relay-RS/compare/v1.1.0...HEAD
[1.1.0]: https://github.com/Develata/BaiduPCS-Relay-RS/compare/v1.0.1...v1.1.0
[1.0.1]: https://github.com/Develata/BaiduPCS-Relay-RS/compare/v1.0.0...v1.0.1
[1.0.0]: https://github.com/Develata/BaiduPCS-Relay-RS/releases/tag/v1.0.0
