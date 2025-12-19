# 上传 GitHub 前检查清单

## 已完成

✅ **删除临时文件**
- test_max_size.py
- test_max_size.sh
- test_size_limit.sh
- repomix-output.xml
- check.log
- share_page.html
- 测试指南.md（中文版，重复）

✅ **更新配置文件**
- config.example.toml: http_timeout_secs 从 30 改为 120
- .gitignore: 添加测试文件忽略规则

✅ **优化依赖**
- 移除未使用的 async_zip 依赖
- 移除未使用的 futures-lite 依赖

✅ **文档更新**
- README.md: 添加 Web 服务器模式说明
- README.md: 添加 MAX_ZIP_SIZE 环境变量说明
- TEST_GUIDE.md: 重写为清晰的测试指南

✅ **新增文件**
- .dockerignore: 优化 Docker 构建

✅ **代码格式化**
- 运行 cargo fmt

## 待执行

### 1. 最终检查

```bash
# 在 Docker 容器中运行（如果本地没有 Rust）
docker exec rust-manual-run bash -c 'cd /app && cargo check'
docker exec rust-manual-run bash -c 'cd /app && cargo test'
docker exec rust-manual-run bash -c 'cd /app && cargo clippy --all-targets'
```

### 2. 验证构建

```bash
# 测试 release 构建
docker exec rust-manual-run bash -c 'cd /app && cargo build --release'

# 验证二进制文件
docker exec rust-manual-run bash -c 'ls -lh /app/target/release/baidu-*'
```

### 3. 清理 config.toml

确保 config.toml 不包含真实凭证：

```bash
# 检查 config.toml 是否在 .gitignore 中
grep -q "config.toml" .gitignore && echo "✅ config.toml 已忽略" || echo "❌ 需要添加到 .gitignore"

# 确认 config.toml 不在 git 追踪中
git status config.toml 2>&1 | grep -q "Untracked\|not staged" && echo "✅ config.toml 未被追踪" || echo "⚠️  config.toml 在 git 中"
```

### 4. Git 操作

```bash
# 查看修改
git status
git diff

# 添加所有更改
git add -A

# 提交
git commit -m "feat: 整理代码，删除临时文件，优化依赖和文档"

# 推送到 GitHub
git push origin share-to-link
```

### 5. GitHub 检查

- [ ] 确认 Actions CI 通过
- [ ] 检查 README.md 在 GitHub 上显示正常
- [ ] 验证 .gitignore 生效（config.toml 不可见）
- [ ] 检查 LICENSE 文件存在

## 建议的 Git Commit 消息

```
feat: 整理代码并优化文档

## 更改内容

### 删除
- 临时测试文件（test_*.py, test_*.sh）
- 重复的中文文档（测试指南.md）
- 调试输出文件（repomix-output.xml, check.log, share_page.html）

### 优化
- 移除未使用的依赖（async_zip, futures-lite）
- 更新默认超时时间为 120 秒
- 添加 .dockerignore 优化构建

### 文档
- 更新 README.md，添加 Web 服务器和分卷功能说明
- 重写 TEST_GUIDE.md，提供清晰的测试步骤
- 更新 .gitignore，忽略测试文件

### 代码
- 运行 cargo fmt 格式化代码
- 通过 cargo clippy 检查
```

## 安全检查

上传前确保：

1. **没有敏感信息**
   - config.toml 不在仓库中
   - 没有真实的 BDUSS/STOKEN
   - 没有真实的 access_token

2. **没有临时文件**
   - target/ 目录已忽略
   - 日志文件已忽略
   - 测试输出已忽略

3. **文档完整**
   - README.md 完整且准确
   - LICENSE 文件存在
   - 示例配置文件清晰

## 后续步骤

1. 在 GitHub 创建 Release
2. 添加构建 workflow（如果还没有）
3. 更新项目描述和标签
4. 考虑添加 CHANGELOG.md
