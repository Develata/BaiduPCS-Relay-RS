#!/bin/bash

echo "╔════════════════════════════════════════════════════════════╗"
echo "║  百度网盘直链系统 - 问题诊断工具                           ║"
echo "╚════════════════════════════════════════════════════════════╝"
echo ""

# 1. 检查配置文件
echo "【1/5】检查配置文件..."
if [ ! -f config.toml ]; then
    echo "❌ config.toml 不存在"
    exit 1
fi

BDUSS=$(grep 'cookie_bduss' config.toml | cut -d'"' -f2)
STOKEN=$(grep 'cookie_stoken' config.toml | cut -d'"' -f2)

if [ -z "$BDUSS" ] || [ "$BDUSS" = "YOUR_BDUSS" ]; then
    echo "❌ BDUSS 未配置或为默认值"
    exit 1
fi

echo "✅ 配置文件存在"
echo "   BDUSS 长度: ${#BDUSS}"
echo "   STOKEN 长度: ${#STOKEN}"

# 2. 测试 Cookie 有效性
echo ""
echo "【2/5】测试 Cookie 有效性..."
response=$(curl -s -w "%{http_code}" -o /tmp/cookie_test.html \
    "https://pan.baidu.com/api/user/getinfo" \
    -H "Cookie: BDUSS=$BDUSS; STOKEN=$STOKEN")

if [ "$response" = "200" ]; then
    if grep -q '"errno":0' /tmp/cookie_test.html; then
        echo "✅ Cookie 有效"
    else
        echo "❌ Cookie 无效或已过期"
        echo "   需要重新获取 Cookie"
        exit 1
    fi
else
    echo "❌ 无法连接到百度服务器 (HTTP $response)"
fi

# 3. 测试分享链接
echo ""
echo "【3/5】测试分享链接访问..."
SHARE_URL="https://pan.baidu.com/s/158pDc"
curl -s "$SHARE_URL" \
    -H "Cookie: BDUSS=$BDUSS; STOKEN=$STOKEN" \
    -H "User-Agent: Mozilla/5.0" \
    > /tmp/share_test.html

if grep -q "yunData" /tmp/share_test.html; then
    echo "✅ 分享链接可访问"
elif grep -q "不存在\|已过期\|已取消" /tmp/share_test.html; then
    echo "❌ 分享链接已失效"
else
    echo "⚠️  分享链接状态未知"
fi

# 4. 测试 API
echo ""
echo "【4/5】测试 wxlist API..."
SURL="58pDc"
result=$(curl -s -X POST "https://pan.baidu.com/share/wxlist?channel=weixin&version=2.2.2&clienttype=25&web=1&t=$(date +%s)" \
    -H "Cookie: BDUSS=$BDUSS; STOKEN=$STOKEN" \
    -d "shorturl=$SURL" \
    -d "pwd=" \
    -d "root=1")

errno=$(echo "$result" | grep -o '"errno":[0-9-]*' | cut -d':' -f2)

if [ "$errno" = "0" ]; then
    echo "✅ API 调用成功"
elif [ "$errno" = "2" ]; then
    echo "❌ API 返回 errno:2 (Cookie 失效)"
else
    echo "❌ API 返回 errno:$errno"
fi

# 5. 总结
echo ""
echo "【5/5】诊断总结"
echo "════════════════════════════════════════════════════════════"

if [ "$errno" = "2" ]; then
    echo "问题原因: Cookie 已失效"
    echo ""
    echo "解决方法:"
    echo "1. 浏览器打开 https://pan.baidu.com"
    echo "2. 登录你的百度账号"
    echo "3. F12 → Application → Cookies → https://pan.baidu.com"
    echo "4. 复制 BDUSS 和 STOKEN 的完整值"
    echo "5. 更新 config.toml 文件"
    echo "6. 重启服务器: cargo run --release"
else
    echo "请查看上述检查结果"
fi

rm -f /tmp/cookie_test.html /tmp/share_test.html
