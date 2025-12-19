#!/bin/bash
# 测试 MAX_ZIP_SIZE 和分卷功能

BASE_URL="http://localhost:5200"
ACCESS_TOKEN="123456"
FOLDER_FSID=433454007834933

echo "============================================================"
echo "测试: MAX_ZIP_SIZE 限制和分卷功能"
echo "============================================================"
echo ""
echo "📁 请求打包 fsid=$FOLDER_FSID (test/ 目录，包含 7.9MB PDF)"
echo "⚙️  当前 MAX_ZIP_SIZE=10485760 (10MB)"
echo "💡 预期: 文件大小 < 10MB，应该返回单个 ZIP"
echo ""

# 发送请求
echo "🌐 POST $BASE_URL/api/zip"
echo ""

curl -s -D /tmp/headers.txt -o /tmp/test_max_size.zip \
  -X POST "$BASE_URL/api/zip" \
  -H "Content-Type: application/json" \
  -d "{\"fsids\": [$FOLDER_FSID], \"archive_name\": \"test_folder\", \"token\": \"$ACCESS_TOKEN\"}"

echo ""
echo "📊 HTTP Headers:"
cat /tmp/headers.txt | head -10
echo ""

# 检查响应类型
CONTENT_TYPE=$(grep -i "content-type:" /tmp/headers.txt | cut -d: -f2 | tr -d ' \r')
FILE_SIZE=$(stat -c%s /tmp/test_max_size.zip 2>/dev/null || echo 0)

echo "📊 Content-Type: $CONTENT_TYPE"
echo "📊 File Size: $FILE_SIZE bytes ($(echo "scale=2; $FILE_SIZE/1024/1024" | bc) MB)"
echo ""

if [[ "$CONTENT_TYPE" == *"application/json"* ]]; then
    echo "✅ 返回分卷信息 (JSON):"
    cat /tmp/test_max_size.zip | jq '.' 2>/dev/null || cat /tmp/test_max_size.zip
elif [[ "$CONTENT_TYPE" == *"application/zip"* ]]; then
    echo "✅ 返回单个 ZIP 文件"
    echo "   已保存到: /tmp/test_max_size.zip"
    echo ""
    echo "📝 ZIP 内容:"
    unzip -l /tmp/test_max_size.zip 2>/dev/null || echo "   无法读取 ZIP 内容"
else
    echo "❌ 未知响应类型"
    echo "响应内容:"
    head -20 /tmp/test_max_size.zip
fi

echo ""
echo "============================================================"
echo "提示: 如需测试分卷功能，请重启服务器并设置 MAX_ZIP_SIZE=5242880 (5MB)"
echo "命令: pkill baidu-web-server && sleep 1 && MAX_ZIP_SIZE=5242880 ./target/release/baidu-web-server &"
echo "============================================================"
