# MAX_ZIP_SIZE 功能测试指南

## 快速测试步骤

### 1. 测试默认配置（不分卷）

```bash
# 启动服务器（默认 MAX_ZIP_SIZE=2GB）
docker exec -d rust-manual-run bash -c "pkill -f baidu-web-server; sleep 2; cd /app && ./target/release/baidu-web-server"

# 等待服务器启动
sleep 3

# 测试请求（test 文件夹约 7.9MB）
curl -X POST 'http://localhost:5200/api/zip' \
  -H 'Content-Type: application/json' \
  -d '{
    "fsids": [471571218073491],
    "archive_name": "test_folder",
    "token": "123456"
  }' \
  -o /tmp/test_output.bin

# 检查结果
file /tmp/test_output.bin
ls -lh /tmp/test_output.bin
```

**预期结果**: 返回单个 ZIP 文件（约 7-8MB）

---

### 2. 测试 5MB 限制

```bash
# 重启服务器，设置 MAX_ZIP_SIZE=5MB
docker exec -d rust-manual-run bash -c "pkill -f baidu-web-server; sleep 2; cd /app && MAX_ZIP_SIZE=5242880 ./target/release/baidu-web-server"

sleep 3

# 测试请求
curl -X POST 'http://localhost:5200/api/zip' \
  -H 'Content-Type: application/json' \
  -d '{
    "fsids": [471571218073491],
    "archive_name": "test_5mb",
    "token": "123456"
  }' \
  -o /tmp/test_5mb.json

# 检查结果
cat /tmp/test_5mb.json | jq .
```

**预期结果**: 可能返回单个 ZIP（因为未压缩大小检查），或者返回 JSON 分卷信息

---

### 3. 测试 1MB 限制（强制分卷）

```bash
# 重启服务器，设置 MAX_ZIP_SIZE=1MB
docker exec -d rust-manual-run bash -c "pkill -f baidu-web-server; sleep 2; cd /app && MAX_ZIP_SIZE=1048576 ./target/release/baidu-web-server"

sleep 3

# 测试请求
curl -X POST 'http://localhost:5200/api/zip' \
  -H 'Content-Type: application/json' \
  -d '{
    "fsids": [471571218073491],
    "archive_name": "test_1mb",
    "token": "123456"
  }' | jq .
```

**预期结果**: 返回 JSON 分卷信息，类似：
```json
{
  "success": true,
  "total_parts": 2,
  "total_size": 8242414,
  "parts": [
    {
      "part_num": 1,
      "filename": "test_1mb.z01",
      "size_bytes": 1048576
    },
    {
      "part_num": 2,
      "filename": "test_1mb.z02",
      "size_bytes": 7193838
    }
  ],
  "message": "文件超过大小限制，已分卷。请分别下载各个 part 文件。"
}
```

---

## 快速一键测试

```bash
# 测试 1: 默认配置
docker exec rust-manual-run bash -c "pkill -f baidu-web-server; sleep 2; cd /app && ./target/release/baidu-web-server &" && sleep 4 && \
curl -s -X POST 'http://localhost:5200/api/zip' -H 'Content-Type: application/json' \
  -d '{"fsids": [471571218073491], "archive_name": "test", "token": "123456"}' \
  -o /tmp/test1.bin && file /tmp/test1.bin && ls -lh /tmp/test1.bin

# 测试 2: 1MB 限制（应该分卷）
docker exec rust-manual-run bash -c "pkill -f baidu-web-server; sleep 2; cd /app && MAX_ZIP_SIZE=1048576 ./target/release/baidu-web-server &" && sleep 4 && \
curl -s -X POST 'http://localhost:5200/api/zip' -H 'Content-Type: application/json' \
  -d '{"fsids": [471571218073491], "archive_name": "test", "token": "123456"}' | jq .
```

---

## 验证要点

### ✅ 单个 ZIP 文件返回
- HTTP 状态码: 200
- Content-Type: application/zip
- Content-Disposition: attachment; filename="..."
- 文件可以用 unzip 正常解压

### ✅ 分卷 JSON 返回
- HTTP 状态码: 200
- Content-Type: application/json
- JSON 包含 `total_parts`, `parts[]`, `message` 字段
- 每个 part 有 `part_num`, `filename`, `size_bytes`

---

## 环境变量说明

- **MAX_ZIP_SIZE**: ZIP 压缩包最大大小限制（字节）
- 默认值: `2147483648` (2GB)
- 示例:
  - `MAX_ZIP_SIZE=1048576` (1MB)
  - `MAX_ZIP_SIZE=5242880` (5MB)
  - `MAX_ZIP_SIZE=10485760` (10MB)

**注意**: 限制检查的是**未压缩文件总大小**，不是压缩后的 ZIP 大小。

---

## 日志查看

```bash
# 查看服务器日志
docker exec rust-manual-run bash -c "tail -50 /tmp/server.log 2>/dev/null || echo '日志文件不存在'"

# 实时监控日志
docker exec rust-manual-run bash -c "tail -f /tmp/server.log"
```
