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

## ZIP 接口

```bash
curl -X POST 'http://localhost:5200/api/zip' \
  -H 'Content-Type: application/json' \
  -H 'Authorization: Bearer your-access-token' \
  -d '{}'
```

预期返回 HTTP 501，错误码 `zip_unsupported`。
