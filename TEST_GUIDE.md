# Web 服务器测试指南

## 快速开始

### 1. 启动服务器

```bash
# 本地启动
./target/release/baidu-web-server

# 带环境变量启动
MAX_ZIP_SIZE=1073741824 ./target/release/baidu-web-server

# Docker 启动
docker-compose up -d
```

服务默认运行在 `http://localhost:5200`

### 2. 健康检查

```bash
curl http://localhost:5200/health
# 响应: OK
```

## API 接口测试

### 接口 1: 分享链接转直链

将百度网盘分享链接转换为可下载的直链。

**请求：**

```bash
curl -X POST 'http://localhost:5200/api/convert' \
  -H 'Content-Type: application/json' \
  -d '{
    "link": "https://pan.baidu.com/s/1xxxxx",
    "pwd": "提取码(可选)",
    "token": "your-access-token"
  }'
```

**响应（成功）：**

```json
{
  "success": true,
  "links": [
    {
      "filename": "示例文件.pdf",
      "download_url": "/d/download?fsid=123456&sign=xxx&expires=1234567890&filename=xxx"
    }
  ]
}
```

**响应（失败）：**

```json
{
  "success": false,
  "error": "错误信息"
}
```

### 接口 2: 文件打包为 ZIP

将指定的文件/文件夹打包为 ZIP 下载。支持自动分卷。

**请求：**

```bash
curl -X POST 'http://localhost:5200/api/zip' \
  -H 'Content-Type: application/json' \
  -d '{
    "fsids": [123456789, 987654321],
    "archive_name": "my_archive",
    "token": "your-access-token"
  }' \
  -o output.zip
```

**参数说明：**

- `fsids`: 文件 ID 数组（从 `/api/convert` 接口获取）
- `archive_name`: 压缩包名称（不含 .zip 后缀）
- `token`: 访问密码（config.toml 中配置）

**响应类型 1 - 单文件（文件总大小 < MAX_ZIP_SIZE）：**

直接返回 ZIP 文件流，可保存到本地：

```bash
curl ... -o output.zip
unzip -l output.zip
```

**响应类型 2 - 分卷（文件总大小 > MAX_ZIP_SIZE）：**

返回 JSON，包含分卷信息：

```json
{
  "success": true,
  "total_parts": 3,
  "total_size": 3221225472,
  "parts": [
    {
      "part_num": 1,
      "filename": "my_archive.z01",
      "size_bytes": 1073741824
    },
    {
      "part_num": 2,
      "filename": "my_archive.z02",
      "size_bytes": 1073741824
    },
    {
      "part_num": 3,
      "filename": "my_archive.z03",
      "size_bytes": 1073741824
    }
  ],
  "message": "文件超过大小限制，已分卷。请分别下载各个 part 文件。"
}
```

## MAX_ZIP_SIZE 配置

控制 ZIP 文件的最大大小（按**未压缩**文件大小计算）。

### 环境变量设置

```bash
# 1MB（用于快速测试）
export MAX_ZIP_SIZE=1048576

# 100MB
export MAX_ZIP_SIZE=104857600

# 1GB（推荐）
export MAX_ZIP_SIZE=1073741824

# 2GB（默认值）
export MAX_ZIP_SIZE=2147483648
```

### 测试分卷功能

```bash
# 1. 启动服务器，设置小限制（如 10MB）
MAX_ZIP_SIZE=10485760 ./target/release/baidu-web-server

# 2. 请求打包大文件（>10MB）
curl -X POST 'http://localhost:5200/api/zip' \
  -H 'Content-Type: application/json' \
  -d '{
    "fsids": [你的大文件fsid],
    "archive_name": "test",
    "token": "your-access-token"
  }'

# 3. 查看返回的 JSON，应包含 parts 数组
```

### 分卷说明

- **分卷策略**: 按文件为单位分配到各个 part
- **单个 part 大小**: 最大 1GB
- **单文件大于限制**: 会单独占一个 part（可能超过限制）
- **文件命名**: `archive_name.z01`, `archive_name.z02`, ...

## 获取 fsid

### 方法 1: 通过 /api/convert

```bash
# 1. 调用转换接口
curl -X POST 'http://localhost:5200/api/convert' \
  -H 'Content-Type: application/json' \
  -d '{
    "link": "https://pan.baidu.com/s/1xxxxx",
    "pwd": "1234",
    "token": "your-access-token"
  }'

# 2. 从响应的 download_url 中提取 fsid
# 例如: /d/download?fsid=123456&sign=...
# fsid 就是 123456
```

### 方法 2: 使用浏览器开发者工具

1. 打开分享链接
2. F12 打开开发者工具
3. 查看网络请求，找到包含 `fsid` 的响应

## 配置文件

确保 `config.toml` 配置正确：

```toml
[baidu]
cookie_bduss = "你的BDUSS"
cookie_stoken = "你的STOKEN"
save_path = "/我的资源"
http_timeout_secs = 120  # 建议 120-300，避免大文件下载超时

[web]
access_token = "your-secret-password"  # API 访问密码
sign_secret = "your-sign-secret"       # 签名密钥

[baidu_open]
# 可选，用于 OAuth
client_id = ""
client_secret = ""
redirect_uri = ""
refresh_token = ""
access_token = ""
```

## 常见问题

### Q: 访问密码错误

**错误**: `401 Unauthorized`

**解决**: 检查请求中的 `token` 是否与 `config.toml` 中 `[web] access_token` 一致

### Q: 下载超时

**错误**: `error decoding response body` 或 `timeout`

**解决**: 增加 `http_timeout_secs` 到 120 或更高

### Q: 分享链接失效

**错误**: `分享链接已过期或被删除`

**解决**: 确认分享链接有效，提取码正确

### Q: 文件下载后是错误消息

**症状**: 下载的 ZIP 只有几十字节，内容是错误文本

**原因**: 服务器返回的是错误响应，不是 ZIP 文件

**解决**: 
1. 不要用 `-o` 保存，先查看响应内容
2. 检查服务器日志
3. 确认 fsid 和 token 正确

### Q: 如何查看服务器日志

```bash
# 本地运行
./target/release/baidu-web-server 2>&1 | tee server.log

# Docker 运行
docker logs container-name

# 如果重定向到文件
tail -f /tmp/server.log
```

## 完整测试流程

```bash
# 1. 启动服务器
MAX_ZIP_SIZE=104857600 ./target/release/baidu-web-server &

# 2. 健康检查
curl http://localhost:5200/health

# 3. 转换分享链接
curl -X POST 'http://localhost:5200/api/convert' \
  -H 'Content-Type: application/json' \
  -d '{
    "link": "https://pan.baidu.com/s/1xxxxx",
    "pwd": "1234",
    "token": "123456"
  }' | jq

# 4. 记录 fsid，测试 ZIP 打包
curl -X POST 'http://localhost:5200/api/zip' \
  -H 'Content-Type: application/json' \
  -d '{
    "fsids": [获取到的fsid],
    "archive_name": "test",
    "token": "123456"
  }' -o test.zip

# 5. 验证 ZIP 文件
file test.zip
unzip -l test.zip

# 6. 停止服务器
pkill baidu-web-server
```

## 性能建议

1. **http_timeout_secs**: 设置为 120-300 秒
2. **MAX_ZIP_SIZE**: 生产环境推荐 1GB
3. **并发请求**: 服务器使用 Tokio 异步运行时，支持高并发
4. **内存使用**: 打包时会将文件加载到内存，注意系统内存

## 安全建议

1. **access_token**: 使用强密码，不要使用默认值
2. **sign_secret**: 随机生成，不要分享
3. **网络**: 建议使用 HTTPS（可通过反向代理如 Nginx）
4. **防火墙**: 限制只允许信任的 IP 访问

## Docker 部署

```bash
# 构建镜像
docker-compose build

# 启动服务
docker-compose up -d

# 查看日志
docker-compose logs -f

# 停止服务
docker-compose down
```

## 参考资料

- [README.md](README.md) - 项目总览
- [config.example.toml](config.example.toml) - 配置示例
