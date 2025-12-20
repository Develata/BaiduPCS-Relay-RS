# Release Notes

## v1.0.0

### 新增功能

- ✅ **Web 界面**：提供友好的 Web 界面进行转存操作
- ✅ **密码保护**：Web 界面支持密码保护
- ✅ **环境变量配置**：支持通过环境变量配置，无需配置文件
- ✅ **健康检查端点**：`/health` 端点用于服务监控

### 改进

- 🔧 优化网络配置，移除硬编码代理设置
- 🔧 改进错误处理和用户提示
- 🔧 优化代码结构，提升可维护性

### 使用方式

#### 快速开始（Release 二进制）

1. 下载对应平台的二进制文件
2. 配置 Cookie（配置文件或环境变量）
3. 运行程序

**CLI 模式：**
```bash
./baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "提取码"
```

**Web 模式：**
```bash
./baidu-direct-link-web
# 访问 http://localhost:5200
```

#### 环境变量配置（推荐用于服务器部署）

```bash
export BDUSS="your_bduss"
export STOKEN="your_stoken"
export SAVE_PATH="/我的资源"
export PORT=5200
export WEB_PASSWORD="your_password"  # 可选

./baidu-direct-link-web
```

### 技术栈

- Rust 1.91+
- Tokio（异步运行时）
- Axum（Web 框架）
- Reqwest（HTTP 客户端）

### 系统要求

- **支持平台**：Linux、macOS、Windows（代码层面完全支持）
- **最低内存**：32 MB
- **最低存储**：10 MB

**平台说明**：
- ✅ **Linux**：完全支持，可直接使用 Release 二进制或从源码构建
- ✅ **macOS**：完全支持，需要从源码构建（`cargo build --release`）
- ✅ **Windows**：代码支持，但需要从源码构建（`cargo build --release`）
  - 启动脚本 `start-web.sh` 需要 Git Bash 或 WSL 运行
  - 或直接运行二进制：`baidu-direct-link-web.exe`

### 注意事项

- Cookie 需要定期更新（通常 30-90 天）
- 转存路径需要在百度网盘中提前创建
- Web 界面建议设置密码保护

