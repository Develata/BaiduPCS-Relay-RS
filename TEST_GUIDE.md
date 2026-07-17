# Web 服务器测试指南

## v1 能力边界

- `/api/convert`：分享链接转本地签名下载链接。
- `/d/download`：验证本地签名后，由服务端按百度官方 `dlink + access_token + User-Agent: pan.baidu.com` 规则解析 302，并把最终地址重定向给客户端。
- `/api/zip`：v1 暂不支持服务器端 ZIP 打包，固定返回 `501 zip_unsupported`。

## 快速检查

```bash
cargo run --bin baidu-web-server
curl http://localhost:5200/health
```

健康检查响应：

```json
{ "status": "ok", "version": "1.0.0" }
```

## 前端 OAuth 授权

本地测试推荐 OOB 模式：

```bash
export BAIDU_CLIENT_ID='your-app-key'
export BAIDU_CLIENT_SECRET='your-secret-key'
export BAIDU_REDIRECT_URI='oob'
```

访问 `http://127.0.0.1:5200`，输入 `WEB_ACCESS_TOKEN` 后点击“授权百度网盘”。百度页面显示
authorization code 后，将它粘贴到本地前端并提交。也可以把 `BAIDU_REDIRECT_URI` 改成已在
百度开放平台“安全设置”中登记的完整 HTTP(S) 回调地址，例如
`http://127.0.0.1:5200/oauth/callback`，此时授权会自动回调。

授权成功后可查看并复制：

- `BAIDU_ACCESS_TOKEN`
- `BAIDU_REFRESH_TOKEN`
- `BAIDU_CLIENT_ID`

`BAIDU_CLIENT_SECRET` 只保存在服务端，不通过前端读取。授权状态接口可直接检查：

```bash
curl 'http://127.0.0.1:5200/api/oauth/status' \
  -H 'Authorization: Bearer your-access-token'
```

OOB API 流程为先调用 `POST /api/oauth/start` 取得 `authorize_url` 和一次性 `flow_id`，再调用：

```bash
curl -X POST 'http://127.0.0.1:5200/api/oauth/exchange' \
  -H 'Authorization: Bearer your-access-token' \
  -H 'Content-Type: application/json' \
  -d '{"flow_id":"start-response-flow-id","code":"baidu-authorization-code"}'
```

`flow_id` 和 authorization code 都只能使用一次；交换失败后重新调用 `/api/oauth/start`。

## 分享链接转直链

```bash
curl -X POST 'http://localhost:5200/api/convert' \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer your-access-token' \
  -d '{
    "link": "https://pan.baidu.com/s/1xxxxx",
    "pwd": "提取码(可选)"
  }'
```

响应：

```json
{
  "success": true,
  "items": [
    {
      "fsid": 123456,
      "filename": "file.pdf",
      "download_url": "/d/download?fsid=123456&expires=...&filename=file.pdf&sign=...",
      "expires": 1234567890
    }
  ],
  "transfer_job": "..."
}
```

`link` 自带非空 `pwd` 时，其优先级高于请求体 `pwd`；可用错误的请求体提取码验证
服务端确实采用了链接内提取码。dlink 解析请求使用 `User-Agent: pan.baidu.com`。

## ZIP 接口

```bash
curl -X POST 'http://localhost:5200/api/zip' \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer your-access-token' \
  -d '{}'
```

预期返回 HTTP 501，错误码 `zip_unsupported`。
