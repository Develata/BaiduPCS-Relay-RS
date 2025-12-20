# BaiduPCS-Relay-RS

[![CI](https://github.com/Develata/BaiduPCS-Relay-RS/actions/workflows/ci.yml/badge.svg)](https://github.com/Develata/BaiduPCS-Relay-RS/actions/workflows/ci.yml)
[![License](https://img.shields.io/badge/License-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/Rust-stable-orange.svg)](https://www.rust-lang.org/)

百度网盘分享链接转存工具：支持 CLI 和 Web 界面两种方式，将分享链接中的文件转存到你的网盘。

## 功能特性

- ✅ 支持带/不带提取码的分享链接
- ✅ 自动拉取分享列表并发起转存
- ✅ CLI 命令行工具
- ✅ Web 界面（支持密码保护）
- ✅ 支持环境变量配置
- ✅ 支持 Docker/Podman 容器运行

## 快速开始

### 方式一：使用 Release 二进制（推荐）

1. **下载 Release**

   从 [Releases](https://github.com/Develata/BaiduPCS-Relay-RS/releases) 下载对应平台的二进制文件。

2. **配置 Cookie**

   编辑 `config.toml`（或使用环境变量）：

```toml
[baidu]
   cookie_bduss = "YOUR_BDUSS"
   cookie_stoken = "YOUR_STOKEN"
save_path = "/我的资源"
http_timeout_secs = 30

   [web]
   password = ""  # 可选，Web 界面访问密码
```

   或使用环境变量（无需配置文件）：

```bash
   export BDUSS="your_bduss"
   export STOKEN="your_stoken"
   export SAVE_PATH="/我的资源"
   export PORT=5200
   export WEB_PASSWORD="your_password"  # 可选
   ```

3. **运行**

   **CLI 模式：**
   ```bash
   ./baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "提取码"
```

   **Web 模式：**
   ```bash
   ./baidu-direct-link-web
   # 或使用启动脚本
   ./start-web.sh
   ```

   访问 http://localhost:5200 使用 Web 界面。

### 方式二：从源码编译

#### 前置要求

- Rust 1.91+（安装：https://rustup.rs/）
- 系统依赖（Linux）：
  ```bash
  # Debian/Ubuntu
  sudo apt-get install pkg-config libssl-dev
  
  # CentOS/RHEL
  sudo yum install pkg-config openssl-devel
  ```

#### 构建和运行

```bash
# 1. 克隆仓库
git clone https://github.com/Develata/BaiduPCS-Relay-RS.git
cd BaiduPCS-Relay-RS

# 2. 配置 Cookie（创建 config.toml 或使用环境变量）
# 参考上面的"配置说明"部分

# 3. 构建 Release 版本（推荐）
./build-release.sh
# 构建完成后，二进制文件在 release/ 目录

# 或手动构建
cargo build --release

# 4. 运行
# CLI 模式
cargo run --release --bin baidu-direct-link -- "https://pan.baidu.com/s/1xxxxx" "提取码"

# Web 模式
cargo run --release --bin baidu-direct-link-web
# 访问 http://localhost:5200
```

#### 开发模式运行

```bash
# CLI 模式（开发版本，带调试信息）
cargo run --bin baidu-direct-link -- "https://pan.baidu.com/s/1xxxxx" "提取码"

# Web 模式（开发版本）
cargo run --bin baidu-direct-link-web
# 访问 http://localhost:5200
```

### 方式三：使用 Docker/Podman

```bash
# 使用 Podman
./podman-run.sh --web

# 或使用 Docker Compose
docker-compose up
```

## 配置说明

### 配置文件方式

创建 `config.toml`：

```toml
[baidu]
cookie_bduss = "YOUR_BDUSS"      # 必填：从浏览器 Cookie 获取
cookie_stoken = "YOUR_STOKEN"    # 必填：从浏览器 Cookie 获取
save_path = "/我的资源"          # 必填：转存保存路径
http_timeout_secs = 30           # 可选：HTTP 超时（秒）

[web]
password = ""                    # 可选：Web 界面访问密码
```

### 环境变量方式

支持通过环境变量配置，无需配置文件：

| 环境变量 | 说明 | 必填 |
|---------|------|------|
| `BDUSS` | 百度网盘 BDUSS Cookie | ✅ |
| `STOKEN` | 百度网盘 STOKEN Cookie | ✅ |
| `SAVE_PATH` | 转存保存路径 | ✅ |
| `HTTP_TIMEOUT_SECS` | HTTP 超时（秒） | ❌ |
| `WEB_PASSWORD` | Web 界面访问密码 | ❌ |
| `PORT` | Web 服务器端口（默认 5200） | ❌ |
| `CONFIG_PATH` | 配置文件路径（默认 config.toml） | ❌ |

**示例：**

```bash
export BDUSS="your_bduss"
export STOKEN="your_stoken"
export SAVE_PATH="/我的资源"
./baidu-direct-link-web
```

## 使用方法

### CLI 模式

```bash
# 基本用法
./baidu-direct-link <分享链接> [提取码] [配置文件路径]

# 示例
./baidu-direct-link "https://pan.baidu.com/s/1xxxxx"
./baidu-direct-link "https://pan.baidu.com/s/1xxxxx" "1234"
```

### Web 模式

1. 启动服务器：
```bash
   ./baidu-direct-link-web
   ```

2. 访问 Web 界面：
   - 打开浏览器访问 http://localhost:5200
   - 如果设置了密码，会跳转到登录页面

3. 使用界面：
   - 输入分享链接
   - 输入提取码（可选）
   - 点击"开始转存"

## 获取 Cookie

1. 浏览器登录 [pan.baidu.com](https://pan.baidu.com)
2. 按 `F12` 打开开发者工具
3. 进入 `Application` → `Cookies` → `https://pan.baidu.com`
4. 复制 `BDUSS` 和 `STOKEN` 的值

## Docker/Podman 运行

### Podman

```bash
# CLI 模式
./podman-run.sh "https://pan.baidu.com/s/1xxxxx" "提取码"

# Web 模式
./podman-run.sh --web
```

### Docker Compose

```bash
# Web 模式
docker-compose -f podman-compose-web.yml up
```

## 系统要求

- **最低配置**：CPU 单核，内存 32 MB，存储 10 MB
- **推荐配置**：CPU 双核，内存 64 MB，存储 50 MB
- **支持平台**：
  - ✅ **Linux**：完全支持（可直接使用 Release 二进制）
  - ✅ **macOS**：完全支持（需要从源码构建）
  - ✅ **Windows**：代码支持（需要从源码构建，启动脚本需 Git Bash/WSL）
  - ✅ **Docker/Podman**：完全支持

## 常见问题

### Q: Cookie 在哪里获取？
A: 浏览器登录 pan.baidu.com → F12 → Application → Cookies → 复制 BDUSS 和 STOKEN

### Q: 为什么提示"保存路径不存在"？
A: 需要在百度网盘中**先创建**对应文件夹，工具不会自动创建。

### Q: 支持批量转存吗？
A: 支持，可以写 Shell 脚本循环调用 CLI 版本。

### Q: Web 界面如何设置密码？
A: 在 `config.toml` 的 `[web]` 部分设置 `password`，或使用环境变量 `WEB_PASSWORD`。

### 常见错误码

| errno | 含义 | 解决方法 |
|-------|------|----------|
| 0 | 成功 | - |
| 2 | Cookie失效/路径错误 | 检查 Cookie 和路径 |
| 12 | 文件已存在 | 正常，表示转存成功 |
| -7 | 分享链接失效 | 检查链接是否有效 |
| -9 | 提取码错误 | 检查提取码 |
| -20 | 路径不存在 | 在网盘中创建目录 |

## 安全提示

- ⚠️ 请勿分享或提交 `config.toml`（包含敏感 Cookie）
- ⚠️ BDUSS/STOKEN 等同于账号凭证，请妥善保管
- ⚠️ 建议将配置文件权限设置为仅自己可读写：`chmod 600 config.toml`
- ⚠️ Web 界面建议设置密码保护

## 开发

```bash
# 格式化代码
cargo fmt

# 代码检查
cargo clippy -- -D warnings

# 运行测试
cargo test

# 构建 Release
./build-release.sh
```

## 致谢

本项目参考了 [BaiduPCS-Go](https://github.com/qjfoidnh/BaiduPCS-Go) 的实现，特别感谢开源贡献。

## 免责声明

- 本项目仅供学习交流，请勿用于违法用途
- 使用本工具需遵守百度网盘服务条款
- 使用产生的任何后果由使用者自行承担

## 开源协议

[MIT License](LICENSE)
