#!/bin/bash
# Release 构建脚本

set -e

GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m'

# 获取版本号
VERSION=$(grep '^version' Cargo.toml | cut -d '"' -f 2 || echo "unknown")

echo -e "${GREEN}🔨 开始构建 Release 二进制 (v${VERSION})...${NC}"

# 创建 release 目录
RELEASE_DIR="release"
mkdir -p "${RELEASE_DIR}"

# 构建 CLI 版本
echo -e "${YELLOW}📦 构建 CLI 版本...${NC}"
cargo build --release --bin baidu-direct-link
cp target/release/baidu-direct-link "${RELEASE_DIR}/"

# 构建 Web 版本
echo -e "${YELLOW}📦 构建 Web 版本...${NC}"
cargo build --release --bin baidu-direct-link-web
cp target/release/baidu-direct-link-web "${RELEASE_DIR}/"

# 复制配置文件示例
cp config.example.toml "${RELEASE_DIR}/"

# 复制模板文件
mkdir -p "${RELEASE_DIR}/templates"
cp -r templates/* "${RELEASE_DIR}/templates/"

# 创建启动脚本
cat > "${RELEASE_DIR}/start-web.sh" << 'EOF'
#!/bin/bash
# Web 服务器启动脚本

set -e

# 检查配置（配置文件或环境变量）
if [ ! -f "config.toml" ] && [ -z "$BDUSS" ] && [ -z "$STOKEN" ]; then
    echo "⚠️  未找到 config.toml 且未设置环境变量，从示例文件创建..."
    if [ -f "config.example.toml" ]; then
        cp config.example.toml config.toml
        echo "✅ 已创建 config.toml，请编辑后填入你的 Cookie"
        echo "💡 或者使用环境变量：export BDUSS=... STOKEN=..."
        exit 1
    else
        echo "❌ 错误：需要配置 Cookie（配置文件或环境变量）"
        exit 1
    fi
fi

# 获取端口（默认 5200）
PORT=${PORT:-5200}

echo "🚀 启动 Web 服务器..."
echo "📝 访问地址: http://localhost:${PORT}"

./baidu-direct-link-web
EOF

chmod +x "${RELEASE_DIR}/start-web.sh"

# 创建 README
cat > "${RELEASE_DIR}/README.md" << EOF
# BaiduPCS-Relay-RS v${VERSION}

百度网盘分享链接转存工具 - Release 版本

## 快速开始

### 1. 配置 Cookie

编辑 \`config.toml\`，填入你的百度网盘 Cookie：

\`\`\`toml
[baidu]
cookie_bduss = "YOUR_BDUSS"
cookie_stoken = "YOUR_STOKEN"
save_path = "/我的资源"
\`\`\`

### 2. 启动 Web 服务器

\`\`\`bash
./start-web.sh
\`\`\`

或直接运行：

\`\`\`bash
./baidu-direct-link-web
\`\`\`

### 3. 使用 CLI 版本

\`\`\`bash
./baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "提取码"
\`\`\`

## 环境变量配置（可选）

也可以通过环境变量配置，无需 config.toml：

\`\`\`bash
export BDUSS="your_bduss"
export STOKEN="your_stoken"
export SAVE_PATH="/我的资源"
export PORT=5200
export WEB_PASSWORD="your_password"  # 可选

./baidu-direct-link-web
\`\`\`

## 获取 Cookie

1. 浏览器登录 [pan.baidu.com](https://pan.baidu.com)
2. 按 \`F12\` 打开开发者工具
3. 进入 \`Application\` → \`Cookies\` → \`https://pan.baidu.com\`
4. 复制 \`BDUSS\` 和 \`STOKEN\` 的值

## 更多信息

完整文档请访问：https://github.com/Develata/BaiduPCS-Relay-RS
EOF

echo -e "${GREEN}✅ 构建完成！${NC}"
echo -e "${GREEN}📦 Release v${VERSION} 文件位于: ${RELEASE_DIR}/${NC}"
echo ""
echo -e "${YELLOW}文件列表:${NC}"
ls -lh "${RELEASE_DIR}/"
echo ""
echo -e "${GREEN}💡 提示：${NC}"
echo -e "  - 配置文件：编辑 ${RELEASE_DIR}/config.toml"
echo -e "  - Web 模式：运行 ${RELEASE_DIR}/start-web.sh"
echo -e "  - CLI 模式：运行 ${RELEASE_DIR}/baidu-direct-link"

