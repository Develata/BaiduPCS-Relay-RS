#!/bin/bash
# Podman 运行脚本 - 用于运行 BaiduPCS-Relay-RS

set -e

# 颜色输出
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

echo -e "${GREEN}🚀 启动 Podman 容器运行 BaiduPCS-Relay-RS...${NC}"

# 检查配置文件是否存在
if [ ! -f "config.toml" ]; then
    echo -e "${YELLOW}⚠️  未找到 config.toml，请从 config.example.toml 复制并配置${NC}"
    if [ -f "config.example.toml" ]; then
        read -p "是否现在复制并创建 config.toml? (y/n) " -n 1 -r
        echo
        if [[ $REPLY =~ ^[Yy]$ ]]; then
            cp config.example.toml config.toml
            echo -e "${GREEN}✅ 已创建 config.toml，请编辑后填入你的 Cookie${NC}"
            exit 0
        fi
    fi
fi

# 获取当前目录的绝对路径
PROJECT_DIR=$(pwd)

# 检查是否提供了命令行参数
if [ $# -eq 0 ]; then
    echo -e "${YELLOW}用法: $0 <share_url> [pwd] [config_path]${NC}"
    echo -e "${YELLOW}或者: $0 --build    # 仅构建镜像${NC}"
    echo -e "${YELLOW}或者: $0 --shell    # 进入容器交互式 shell${NC}"
    echo -e "${YELLOW}或者: $0 --web      # 启动 Web 服务器${NC}"
    exit 1
fi

# 特殊命令处理
if [ "$1" == "--build" ]; then
    echo -e "${GREEN}🔨 构建容器镜像...${NC}"
    podman build -t baidupcs-relay-rs:latest .
    echo -e "${GREEN}✅ 构建完成！${NC}"
    exit 0
fi

if [ "$1" == "--shell" ]; then
    echo -e "${GREEN}🐚 进入容器交互式 shell...${NC}"
    podman run -it --rm \
        -v "${PROJECT_DIR}:/app" \
        -w /app \
        -e http_proxy= \
        -e https_proxy= \
        -e HTTP_PROXY= \
        -e HTTPS_PROXY= \
        rust:1.91 \
        bash -c "unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY && apt-get update && apt-get install -y pkg-config libssl-dev && bash"
    exit 0
fi

if [ "$1" == "--web" ]; then
    echo -e "${GREEN}🌐 启动 Web 服务器...${NC}"
    PORT=${PORT:-5200}
    echo -e "${GREEN}📝 Web 服务器将在 http://localhost:${PORT} 启动${NC}"
    podman run -it --rm \
        --name baidupcs-relay-rs-web \
        -v "${PROJECT_DIR}:/app" \
        -v "${PROJECT_DIR}/config.toml:/app/config.toml:ro" \
        -w /app \
        -p "${PORT}:5200" \
        -e PORT=5200 \
        -e http_proxy= \
        -e https_proxy= \
        -e HTTP_PROXY= \
        -e HTTPS_PROXY= \
        rust:1.91 \
        bash -c "unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY && apt-get update -qq && apt-get install -y -qq pkg-config libssl-dev > /dev/null 2>&1 && cargo run --release --bin baidu-direct-link-web"
    exit 0
fi

# 运行容器并执行程序
echo -e "${GREEN}📦 运行容器...${NC}"

# 构建容器（如果不存在）
if ! podman image exists baidupcs-relay-rs:latest 2>/dev/null; then
    echo -e "${YELLOW}⚠️  镜像不存在，正在构建...${NC}"
    podman build -t baidupcs-relay-rs:latest .
fi

# 运行容器
# 构建命令字符串，正确转义所有参数
ARGS_STR=""
for arg in "$@"; do
    # 转义参数中的特殊字符
    ARGS_STR="${ARGS_STR} $(printf '%q' "$arg")"
done

podman run -it --rm \
    --name baidupcs-relay-rs \
    -v "${PROJECT_DIR}:/app" \
    -v "${PROJECT_DIR}/config.toml:/app/config.toml:ro" \
    -w /app \
    -e http_proxy= \
    -e https_proxy= \
    -e HTTP_PROXY= \
    -e HTTPS_PROXY= \
    rust:1.91 \
    bash -c "unset http_proxy https_proxy HTTP_PROXY HTTPS_PROXY && apt-get update -qq && apt-get install -y -qq pkg-config libssl-dev > /dev/null 2>&1 && cargo run --release --${ARGS_STR}"

echo -e "${GREEN}✅ 执行完成！${NC}"

