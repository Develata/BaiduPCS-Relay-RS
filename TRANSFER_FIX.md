# 转存流程修复说明

## 🔴 已修复的关键Bug

### 1. 文件匹配逻辑Bug（最严重）

**问题**：
```rust
// 旧代码（错误）
name_lower.contains(&expected_lower) || expected_lower.contains(&name_lower)
// "西北大学632数学分析.pdf" 包含 "p"，导致匹配到文件名为 "P" 的错误文件！
```

**修复**：
```rust
// 新代码（正确）
// 1. 完全精确匹配
f.server_filename == *expected_name

// 2. 忽略大小写的精确匹配
f.server_filename.eq_ignore_ascii_case(expected_name)

// 3. 去除扩展名后精确匹配
file_stem.eq_ignore_ascii_case(expected_stem)
```

### 2. 转存参数优化

**修改前**：
```rust
("ondup", "newcopy"),  // 文件已存在时新建副本
("async", "1"),        // 异步模式
```

**修改后**：
```rust
("ondup", "overwrite"), // ✅ 覆盖模式，避免 "文件已存在" 错误
("async", "0"),         // ✅ 同步模式，立即完成转存
```

### 3. 保存路径简化

**修改前**：
- 使用 `config.toml` 中的 `savepath`
- 尝试多个可能的路径
- 容易出现路径不一致问题

**修改后**：
```rust
let save_path = "/";  // 直接保存到根目录
```

### 4. 等待时间优化

**修改前**：等待 8 秒（异步模式）

**修改后**：等待 3 秒（同步模式，实际上转存已完成）

## 📝 修改的文件

### 1. `src/service/workflow.rs`
- 转存到根目录
- 修复文件匹配逻辑（精确匹配）
- 优化日志输出

### 2. `src/baidupcs/transfer.rs`
- 保存路径改为根目录 "/"
- 转存模式改为同步 `async=0`
- 重复处理改为覆盖 `ondup=overwrite`

## 🧪 测试步骤

1. **编译**
```bash
cargo build --release
```

2. **启动服务**
```bash
cargo run --release
```

3. **测试转存**
```bash
curl -X POST http://localhost:5200/api/convert \
  -H "Content-Type: application/json" \
  -d '{
    "share_url": "https://pan.baidu.com/s/1xxxx",
    "pwd": "提取码"
  }'
```

4. **使用调试接口查看文件**
```bash
curl -X POST http://localhost:5200/api/debug/list \
  -H "Content-Type: application/json" \
  -d '{"path":"/"}'
```

## ✅ 预期效果

1. ✅ 文件正确匹配（不会匹配到错误文件）
2. ✅ 转存立即完成（同步模式）
3. ✅ 文件保存在根目录（路径清晰）
4. ✅ 覆盖重复文件（避免 errno=2 错误）

## 🔍 调试日志关键点

成功的日志应该是：
```
📦 开始转存 1 个文件到根目录
✅ 转存成功 (errno=0)
⏳ 等待转存完成...
📁 列举根目录
📋 在 / 找到 X 个文件
  [1] 西北大学632数学分析.pdf
📋 开始匹配文件，期望 1 个文件
✅ 匹配成功 - 期望: 西北大学632数学分析.pdf, 实际: 西北大学632数学分析.pdf, fs_id: xxx
✅ 成功匹配 1/1 个文件
```

失败的日志会显示：
```
⚠️ 未匹配到文件: xxx
可用文件列表:
  [1] 文件名1 (fs_id: xxx)
  [2] 文件名2 (fs_id: xxx)
```
