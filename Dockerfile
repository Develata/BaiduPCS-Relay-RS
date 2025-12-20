# 使用已拉取的 rust:1.91 镜像
FROM rust:1.91

# 安装必要的系统依赖
RUN apt-get update && \
    apt-get install -y \
    pkg-config \
    libssl-dev \
    ca-certificates \
    && rm -rf /var/lib/apt/lists/*

# 设置工作目录
WORKDIR /app

# 先复制依赖文件以利用 Docker 缓存层
COPY Cargo.toml Cargo.lock ./

# 创建一个虚拟的 main.rs 来预下载依赖（优化构建缓存）
RUN mkdir -p src && \
    echo "fn main() {}" > src/main.rs && \
    cargo build --release && \
    rm -rf src

# 复制源代码
COPY src ./src
COPY templates ./templates

# 构建项目
RUN cargo build --release && \
    cp target/release/baidu-direct-link /usr/local/bin/baidu-direct-link

# 设置入口点（程序需要命令行参数）
ENTRYPOINT ["/usr/local/bin/baidu-direct-link"]

