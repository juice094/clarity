# Project Clarity - 用户开发指南

> 欢迎回来！这是为你准备的操作手册，方便你在硬件调试后继续开发。

---

## 快速恢复

### 1. 检查项目状态

```powershell
# 进入项目目录
cd C:\Users\22414\Desktop\clarity

# 查看文件结构
tree /f

# 检查配置
.\test_config.ps1
```

### 2. 安装 Rust（如未安装）

```powershell
# 访问 https://rustup.rs/ 下载安装程序
# 或使用 winget
winget install Rustlang.Rustup

# 验证安装
rustc --version
cargo --version
```

### 3. 编译运行

```powershell
# 方式一：使用启动脚本（推荐）
.\run_with_kimi.ps1

# 方式二：手动配置
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="sk-kimi-7wIafvpXmFAZAdBwBHsCQyXaPJ0zQrGbETdKgOjnhQdtXfkbRh2zayGqkFAeAvTz"
$env:ANTHROPIC_MODEL="kimi-for-coding"

cargo run --example claude_code_compat
```

---

## 项目结构速查

```
clarity/
├── 📁 crates/
│   ├── 📁 clarity-core/          # ⭐ 核心引擎
│   │   ├── 📄 src/agent.rs      # Agent Loop
│   │   ├── 📄 src/llm.rs        # LLM 集成
│   │   ├── 📄 src/registry.rs   # 工具注册表
│   │   └── 📁 src/tools/        # 7个内置工具
│   ├── 📁 clarity-tui/           # ⭐ 终端界面
│   │   └── 📄 src/main.rs       # TUI 入口
│   └── 📁 clarity-gateway/       # ⭐ HTTP网关
├── 📁 examples/
│   ├── 📄 claude_code_compat.rs # Claude Code 配置示例 ⭐
│   ├── 📄 kimi_demo.rs          # Kimi API 示例
│   └── 📄 ollama_demo.rs        # 本地模型示例
├── 📄 Cargo.toml                 # 项目配置
├── 📄 DEV_LOG.md                 # 开发日志
├── 📄 AI_HANDOFF.md              # AI 交接文档
└── 📄 HUMAN_GUIDE.md             # 本文档
```

---

## 常用命令

### 开发调试

```powershell
# 编译检查
cargo check

# 编译（开发版）
cargo build

# 编译（发布版）
cargo build --release

# 运行测试
cargo test

# 运行特定示例
cargo run --example claude_code_compat
cargo run --example kimi_demo
cargo run --example ollama_demo

# 运行 TUI
cargo run --bin clarity-tui

# 运行网关
cargo run --bin clarity-gateway
```

### 代码质量

```powershell
# 格式化代码
cargo fmt

# 检查代码风格
cargo clippy

# 生成文档
cargo doc --open
```

---

## 配置选项

### 选项一：Kimi Code（推荐）

**特点**：每周 1024 次免费，与 Claude Code 相同配置

```powershell
$env:ANTHROPIC_BASE_URL="https://api.kimi.com/coding/"
$env:ANTHROPIC_AUTH_TOKEN="sk-kimi-your-key"
$env:ANTHROPIC_MODEL="kimi-for-coding"
```

### 选项二：Kimi API（开放平台）

**特点**：按量付费，更稳定，无次数限制

```powershell
$env:KIMI_API_KEY="sk-your-moonshot-key"
$env:KIMI_BASE_URL="https://api.moonshot.cn/v1"
$env:KIMI_MODEL="moonshot-v1-8k"
```

### 选项三：Ollama 本地模型

**特点**：完全离线，无需网络，保护隐私

```powershell
# 1. 安装 Ollama: https://ollama.com/download
# 2. 拉取模型
ollama run llama3.2

# 3. 运行示例（无需 API Key）
cargo run --example ollama_demo
```

---

## 下一步开发任务

### 立即（恢复后首要）

1. **验证编译**
   ```powershell
   cargo check
   cargo build
   ```

2. **运行示例**
   ```powershell
   .\run_with_kimi.ps1
   ```

3. **验证端到端对话**
   - 输入测试消息
   - 确认收到 LLM 回复
   - 检查工具调用是否正常

### 短期

1. **添加自定义工具**
   - 参考 `crates/clarity-core/src/tools/file.rs`
   - 实现 `Tool` trait
   - 在 `registry.rs` 注册

2. **集成 Memory 系统**
   - 添加 SQLite 持久化
   - 实现对话历史存储
   - 添加向量搜索（可选）

3. **完善 TUI**
   - 添加快捷键帮助
   - 优化流式响应显示
   - 添加工具调用可视化

### 中期

1. **Web Admin UI**
   - 完善 `clarity-gateway` 的 Admin 页面
   - 添加配置管理界面
   - 添加日志查看

2. **WASM 插件**
   - 参考 OpenClaw 的插件系统
   - 实现动态加载

3. **数据迁移**
   - 从云端 OpenClaw 导出数据
   - 导入到 Clarity

---

## 故障排除

### 编译错误

**问题**: `cargo` 命令找不到
```powershell
# 解决：安装 Rust
winget install Rustlang.Rustup
# 重启 PowerShell
```

**问题**: 依赖下载失败
```powershell
# 解决：更换国内镜像
# 编辑 ~/.cargo/config.toml
[source.crates-io]
replace-with = 'ustc'

[source.ustc]
registry = "sparse+https://mirrors.ustc.edu.cn/crates.io-index/"
```

### 运行错误

**问题**: API 返回 403
```
原因：Kimi Code 验证客户端身份
解决：检查 User-Agent 和请求头
参考：llm.rs 中的 header 设置
```

**问题**: 模型名称错误
```
原因：使用了错误的模型名
解决：Kimi Code 用 "kimi-for-coding"，不是 "kimi-k2-0711"
```

**问题**: Ollama 连接失败
```powershell
# 检查 Ollama 是否运行
Invoke-WebRequest -Uri "http://localhost:11434" -Method GET

# 如未运行，启动 Ollama
ollama serve
```

---

## 与其他 AI 会话

如果你需要开启新的 AI 会话来继续开发：

### 给新 AI 的上下文

```
我正在开发 Project Clarity，一个 Rust AI Agent 框架。

项目位置：C:\Users\22414\Desktop\clarity
关键文档：\AI_HANDOFF.md（给AI的交接文档）
开发日志：\DEV_LOG.md

当前状态：
- 基础架构已完成（core/tui/gateway）
- LLM 集成已完成（Kimi Code/API/Ollama）
- API Key 已配置（见 run_with_kimi.ps1）
- 待办：TUI 集成真实 LLM 调用

请阅读 AI_HANDOFF.md 后继续协助开发。
```

### 快速验收清单

让新 AI 帮你验证：

```markdown
1. [ ] 运行 test_config.ps1，确认配置检测正常
2. [ ] 运行 cargo check，确认代码无编译错误
3. [ ] 运行 cargo run --example claude_code_compat
4. [ ] 输入测试消息，确认收到 LLM 回复
5. [ ] 检查工具调用是否正常
```

---

## 参考资源

- **Claude Code 源码**: `C:\Users\22414\Desktop\claude-code-haha-main`
  - 学习：Anthropic SDK 使用、工具系统设计

- **OpenClaw**: `C:\Users\22414\Desktop\openclaw-main`
  - 学习：Gateway 架构、插件系统

- **Kimi 文档**: 
  - Kimi Code: https://www.kimi.com/code/docs
  - Kimi API: https://platform.moonshot.cn/docs

---

## 联系与反馈

如有问题：
1. 查看 `DEV_LOG.md` 了解开发历史
2. 查看 `AI_HANDOFF.md` 获取技术细节
3. 开启新的 AI 会话，提供上述上下文

---

**祝硬件调试顺利，期待你回来继续开发！** 🚀
