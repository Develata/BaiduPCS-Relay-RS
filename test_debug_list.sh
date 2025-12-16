#!/bin/bash
# 测试调试接口 - 列举网盘目录

echo "=== 测试调试接口 ==="

# 测试根目录
echo ""
echo "1️⃣ 测试列举根目录 /"
curl -X POST http://localhost:5200/api/debug/list \
  -H "Content-Type: application/json" \
  -d '{"path":"/"}' | jq .

# 测试 /我的资源
echo ""
echo "2️⃣ 测试列举 /我的资源"
curl -X POST http://localhost:5200/api/debug/list \
  -H "Content-Type: application/json" \
  -d '{"path":"/我的资源"}' | jq .

# 测试可能的异步转存目录
echo ""
echo "3️⃣ 测试列举 /async转存"
curl -X POST http://localhost:5200/api/debug/list \
  -H "Content-Type: application/json" \
  -d '{"path":"/async转存"}' | jq .

echo ""
echo "✅ 测试完成！查看哪个目录有文件，然后修改 config.toml 中的 savepath"
