#!/bin/bash
# 测试 MAX_ZIP_SIZE 和分卷功能

set -e

BASE_URL="http://localhost:5200"
ACCESS_TOKEN="123456"
TEST_FOLDER_FSID=471571218073491  # test 文件夹 (约 7.9MB)

echo "========================================"
echo "测试 1: 默认配置 (MAX_ZIP_SIZE=2GB)"
echo "预期：返回单个 ZIP 文件"
echo "========================================"
echo ""

# 重启服务器（默认 2GB 限制）
docker exec rust-manual-run bash -c "pkill -f baidu-web-server || true"
sleep 2
docker exec -d rust-manual-run bash -c "cd /app && ./target/release/baidu-web-server > /tmp/server.log 2>&1"
sleep 3

echo "发送请求..."
RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "$BASE_URL/api/zip" \
  -H "Content-Type: application/json" \
  -d "{\"fsids\": [$TEST_FOLDER_FSID], \"archive_name\": \"test_default\", \"token\": \"$ACCESS_TOKEN\"}")

HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | head -n -1)

echo "HTTP 状态码: $HTTP_CODE"
if [[ "$HTTP_CODE" == "200" ]]; then
    # 检查是 JSON 还是 ZIP
    if echo "$BODY" | jq . >/dev/null 2>&1; then
        echo "✅ 返回 JSON (分卷)"
        echo "$BODY" | jq .
    else
        SIZE=$(echo "$BODY" | wc -c)
        echo "✅ 返回 ZIP 文件，大小: $SIZE 字节 (~$((SIZE/1024/1024)) MB)"
    fi
else
    echo "❌ 请求失败"
    echo "$BODY"
fi

echo ""
echo "========================================"
echo "测试 2: 设置 MAX_ZIP_SIZE=5MB"
echo "预期：文件夹 7.9MB > 5MB，应该触发分卷"
echo "========================================"
echo ""

# 重启服务器（5MB 限制）
docker exec rust-manual-run bash -c "pkill -f baidu-web-server || true"
sleep 2
docker exec -d rust-manual-run bash -c "cd /app && MAX_ZIP_SIZE=5242880 ./target/release/baidu-web-server > /tmp/server.log 2>&1"
sleep 3

echo "发送请求..."
RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "$BASE_URL/api/zip" \
  -H "Content-Type: application/json" \
  -d "{\"fsids\": [$TEST_FOLDER_FSID], \"archive_name\": \"test_5mb\", \"token\": \"$ACCESS_TOKEN\"}")

HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | head -n -1)

echo "HTTP 状态码: $HTTP_CODE"
if [[ "$HTTP_CODE" == "200" ]]; then
    if echo "$BODY" | jq . >/dev/null 2>&1; then
        echo "✅ 返回 JSON (分卷)"
        echo "$BODY" | jq .
    else
        SIZE=$(echo "$BODY" | wc -c)
        echo "⚠️  返回 ZIP 文件，大小: $SIZE 字节 (~$((SIZE/1024/1024)) MB)"
        echo "说明：未压缩大小可能未超过限制"
    fi
else
    echo "❌ 请求失败"
    echo "$BODY"
fi

echo ""
echo "========================================"
echo "测试 3: 设置 MAX_ZIP_SIZE=1MB (强制分卷)"
echo "预期：7.9MB >> 1MB，必然分卷"
echo "========================================"
echo ""

# 重启服务器（1MB 限制）
docker exec rust-manual-run bash -c "pkill -f baidu-web-server || true"
sleep 2
docker exec -d rust-manual-run bash -c "cd /app && MAX_ZIP_SIZE=1048576 ./target/release/baidu-web-server > /tmp/server.log 2>&1"
sleep 3

echo "发送请求..."
RESPONSE=$(curl -s -w "\n%{http_code}" -X POST "$BASE_URL/api/zip" \
  -H "Content-Type: application/json" \
  -d "{\"fsids\": [$TEST_FOLDER_FSID], \"archive_name\": \"test_1mb\", \"token\": \"$ACCESS_TOKEN\"}")

HTTP_CODE=$(echo "$RESPONSE" | tail -1)
BODY=$(echo "$RESPONSE" | head -n -1)

echo "HTTP 状态码: $HTTP_CODE"
if [[ "$HTTP_CODE" == "200" ]]; then
    if echo "$BODY" | jq . >/dev/null 2>&1; then
        echo "✅ 返回 JSON (分卷)"
        echo "$BODY" | jq .
        
        # 提取分卷数量
        PARTS=$(echo "$BODY" | jq -r '.total_parts')
        echo ""
        echo "📦 总共 $PARTS 个分卷"
    else
        SIZE=$(echo "$BODY" | wc -c)
        echo "❌ 仍然返回单个 ZIP，大小: $SIZE 字节"
    fi
else
    echo "❌ 请求失败"
    echo "$BODY"
fi

echo ""
echo "========================================"
echo "测试完成"
echo "========================================"

# 恢复默认配置
docker exec rust-manual-run bash -c "pkill -f baidu-web-server || true"
sleep 1
docker exec -d rust-manual-run bash -c "cd /app && ./target/release/baidu-web-server > /tmp/server.log 2>&1"

echo ""
echo "服务器已恢复默认配置 (MAX_ZIP_SIZE=2GB)"
