# Clarity 营养吸收计划 —— 风险确认报告

> 分析日期：2026-04-15 | 更新：2026-04-23（标记 S1/S2 已修复）
> 分析范围：表 A 五个营养源的全部 13 项能力吸收
> 风险等级：🟢 低风险（可立即推进） / 🟡 中风险（需评估后推进） / 🔴 高风险（需充分准备后推进） / ⚫ 极高风险（暂停或放弃）

---

## 一、法律与协议风险

### 1.1 各营养源开源协议

| 营养源 | 协议 | 对 Clarity 的影响 |
|--------|------|------------------|
| **OpenHanako** | Apache-2.0 | ✅ 可自由参考架构和代码，需保留 NOTICE |
| **5ire** | Modified Apache 2.0 (non-commercial) | ⚠️ 架构设计可清洁室吸收，但**直接复制代码可能触发商业限制** |
| **Claurst** | MIT | ✅ 最宽松，可自由参考代码和架构 |
| **ZeroClaw** | 未明确（推测 MIT/Apache） | ⚠️ 与 OpenClaw 兼容层可能涉及原项目协议，需谨慎 |
| **OpenClaw** | 未明确（推测 MIT/Apache） | ⚠️ Skill 生态和 IDENTITY 格式是其核心资产，**复制格式可能引发争议** |

### 1.2 关键法律风险点

| 风险项 | 等级 | 说明 |
|--------|------|------|
| **5ire 的 non-commercial 条款** | 🟡 | 5ire 的 Modified Apache 2.0 限制商业使用。Clarity 若未来商业化，直接复制 5ire 代码有法律风险。**对策**：清洁室吸收（看设计文档，不看代码；自己用 Rust 重新实现） |
| **Claurst 的清洁室声明** | 🟢 | Claurst 本身是 Claude Code 的清洁室重写，其 MIT 协议允许自由使用。但 Claurst 的"清洁室"声明提醒我们：**借鉴行为设计而非表达式**是安全的 |
| **OpenClaw/ZeroClaw 的 claw 命名** | 🟡 | "claw"概念已被 OpenClaw/ZeroClaw 占据（250K + 20K Stars）。Clarity 使用 "claw" 作为层名**存在品牌冲突风险**。**对策**：保留 "claw" 作为系统托盘层代号，但对外品牌统一使用 "Clarity"，避免与 OpenClaw 生态混淆 |
| **AIEOS identity 格式** | 🔴 | ZeroClaw 支持的 AIEOS 是第三方标准（v1.1），但 Clarity 若兼容此格式，等于承认其生态地位。**对策**：不直接兼容 AIEOS，而是改造其核心思想（四维配置）为 Clarity 的场景配置格式 |
| **OpenClaw Skill 格式** | 🟡 | OpenClaw 的 Skill 是 Markdown 驱动的，格式本身不受版权保护。但 Skill 目录和品牌（ClawHub）是其商业资产。**对策**：不复制 ClawHub，自建 Clarity Skill 目录，使用开放的 Markdown 格式 |

---

## 二、技术与架构风险

### 2.1 按营养项逐一评估

#### ✅ 已解决（v0.1.1）

| 营养项 | 来源 | 状态 | 说明 |
|--------|------|------|------|
| **目录遍历漏洞** | 内部审计 | ✅ 已修复 | `resolve_path()` 和 `sanitize_path()` 已增加工作目录前缀校验，新增 10 个安全测试 |

#### 🔴 P0 级（高难度）

| 营养项 | 来源 | 技术风险 | 风险等级 | 缓解措施 |
|--------|------|---------|---------|---------|
| **RAG 向量知识库** | 5ire (bge-m3) | ① Rust 中无成熟的嵌入模型运行时（`ort` ONNX Runtime 绑定质量未知）<br>② 本地向量索引（HNSW/IVF）在 Rust 中无生产级库<br>③ 嵌入式设备（Claw 层 <5MB RAM）无法运行嵌入模型 | 🔴 | ① 先用 ollama 本地服务跑嵌入（已有成熟支持）<br>② 向量索引用 sqlite-vss 或纯内存 HNSW（`hnsw` crate）<br>③ claw 层不运行嵌入，只消费预计算的索引 |
| **文档解析** | 5ire | ① PDF/Excel/PPT 解析在 Rust 生态中**几乎空白**<br>② 纯 Rust 方案（`pdf-extract`、`calamine`）功能弱于 Python（pdfplumber/pymupdf/openpyxl）<br>③ WASM 方案增加构建复杂度 | 🔴 | ① 短期：Python 桥接（调用本地 Python 进程解析）<br>② 中期：评估 Rust 原生方案是否够用<br>③ 长期：WASM 嵌入轻量解析器 |
| **Trait 架构改造** | ZeroClaw | ① 当前 clarity-core 的模型/工具/记忆是硬编码组合，改造成 Trait 需要**大规模重构**<br>② Trait 对象（`dyn`）在 async 场景中有性能开销和复杂度<br>③ 需要重新设计配置系统以支持动态替换组件 | 🔴 | ① 分阶段改造：先提取 Provider trait，再提取 Memory trait，最后提取 Tool trait<br>② 使用泛型而非 dyn（零成本抽象）<br>③ 配置系统用 TOML/JSON 描述组件组合 |

#### 🟡 P1 级（中难度）

| 营养项 | 来源 | 技术风险 | 风险等级 | 缓解措施 |
|--------|------|---------|---------|---------|
| **Plugin 包装层** | OpenHanako | ① Rust 的 Plugin 动态加载需 `dylib` 或 WASM，**跨平台编译复杂**<br>② Plugin 安全模型（权限分级）在 Rust 中无现成方案<br>③ MCP 协议与 Plugin 系统的边界需要清晰定义 | 🟡 | ① 先用 WASM 作为 Plugin 运行时（沙箱天然）<br>② 权限模型参考 Deno/Node 的 Capability 模型<br>③ MCP 是底层协议，Plugin 是应用层包装（用户不直接配置 MCP JSON） |
| **Sandbox 机制** | OpenHanako | ① PathGuard 四级访问模型（只读/读写/受限/隔离）需要文件系统拦截层<br>② OS 级沙箱（macOS Seatbelt / Linux Bubblewrap / Windows AppContainer）跨平台差异巨大<br>③ 过度限制会降低用户体验 | 🟡 | ① 先实现应用级沙箱（文件访问白名单）<br>② OS 级沙箱作为可选高级功能<br>③ 默认宽松，用户可手动收紧 |
| **Sub-agents** | Claurst | ① 子 Agent 的上下文隔离需要会话管理的深层改造<br>② 子 Agent 结果回传主 Agent 的上下文合并逻辑复杂<br>③ 错误传播（子 Agent 失败如何影响主 Agent）需要仔细设计 | 🟡 | ① 先实现简单的"工具调用包装"——子任务用独立 LLM 调用，结果作为 tool_result 返回<br>② 逐步引入真正的上下文隔离<br>③ 错误处理默认 fail-fast，可配置容错 |
| **模块化启动** | ZeroClaw | ① 按需加载组件需要重构当前的启动时全量初始化<br>② 组件间依赖关系管理（claw 层不加载 window 的 Web 资源，但可能共享配置）<br>③ lazy initialization 在 Rust async 中的生命周期管理 | 🟡 | ① 用 `Arc<OnceCell<T>>` 或 `tokio::sync::OnceCell` 实现 lazy init<br>② 明确组件依赖图，用依赖注入管理<br>③ 先优化 Gateway 的启动流程（当前可能加载了未使用的资源） |
| **SOPs 工作流** | ZeroClaw | ① 事件驱动架构需要引入消息总线或事件系统<br>② 定时任务（cron）需要调度器（`tokio-cron-scheduler`）<br>③ 文件系统监控（inotify/kqueue/FSEvents）跨平台差异 | 🟡 | ① 用 `notify` crate 做跨平台文件监控<br>② 用 `tokio-cron-scheduler` 做定时任务<br>③ 事件系统先用简单 Channel，逐步升级为 EventBus |
| **Skill 目录** | OpenClaw | ① 需要建立中央托管服务（GitHub 仓库即可）<br>② Skill 格式需要标准化（Markdown + TOML 元数据）<br>③ 无社区则目录为空——**鸡生蛋问题** | 🟡 | ① 先用 GitHub 仓库作为目录（无需自建服务）<br>② Skill 格式先简单（Markdown 提示词 + 工具列表）<br>③ 先由 Clarity 团队提供 10-20 个官方 Skill 填充目录 |

#### 🟢 P2 级（低难度）

| 营养项 | 来源 | 技术风险 | 风险等级 | 缓解措施 |
|--------|------|---------|---------|---------|
| **Compaction** | Claurst | ① 对话总结需要额外的 LLM 调用（增加成本）<br>② 总结质量不稳定（可能丢失关键信息）<br>③ 自动触发的时机选择（按 token 数还是按消息数） | 🟢 | ① 默认关闭，用户手动触发<br>② 总结提示词专门优化（保留决策/代码/关键结论）<br>③ 按 token 阈值触发（如超过 80% 上下文窗口） |
| **Cost tracking** | Claurst | ① 各 Provider 的价格表变动频繁，维护成本高<br>② Token 计数需要与 Provider 的计费方式对齐（不同模型计数规则不同）<br>③ 显示精度（USD 估算 vs 精确计费） | 🟢 | ① 价格表用 TOML 配置文件，定期更新<br>② 用 tiktoken/tokenizers 做本地计数（近似值即可）<br>③ 显示为估算值，标注"approximate" |
| **Markdown 渲染** | Claurst | ① ratatui 无内置 Markdown 渲染组件<br>② 需要自己解析 Markdown AST 并映射到 ratatui widget<br>③ 代码块高亮需要集成 syntect | 🟢 | ① 用 `pulldown-cmark` 解析 Markdown<br>② 自定义 ratatui widget 渲染 AST<br>③ `syntect` 做代码高亮 |
| **Markdown 配置** | OpenClaw | ① 系统提示词从代码抽离到 Markdown 需要重新组织配置加载逻辑<br>② Markdown 中嵌入变量（如用户名、工作目录）需要模板引擎<br>③ 不同入口（claw/window/cli）的 Markdown 配置需要差异化加载 | 🟢 | ① 用 `tera` 或 `handlebars` 做模板渲染<br>② 配置目录结构：`~/.clarity/prompts/{claw,window,cli}.md`<br>③ 加载时根据 EntryPoint 选择对应文件 |

---

## 三、概念与哲学风险

### 3.1 Clarity 核心立场 vs 营养源的冲突

| Clarity 立场 | 营养源概念 | 冲突 | 风险等级 | 改造方案 |
|-------------|-----------|------|---------|---------|
| **无人格（中性 Agent）** | AIEOS identity（人格/心理/语言/动机） | 直接冲突 | 🔴 | 不采用"人格"概念，将 AIEOS 四维改造为"场景配置"——入口行为偏好而非 Agent 人格 |
| **单 Agent 三层分化** | OpenHanako 多智能体协作 | 理念冲突 | 🟡 | 不吸收多智能体架构，但吸收其"任务委托"机制——用 Sub-agents 替代多 Agent |
| **模型中立** | 各营养源都已实现多模型 | "模型中立"不是差异化 | 🟢 | 接受现实，将"模型中立"从"卖点"降级为"基础能力"，寻找新的差异化 |
| **窗口即边界（无需模式切换）** | 5ire 的提示词库/书签/历史搜索 | 功能互补，无冲突 | 🟢 | 吸收这些功能作为 window 层的增强，不改变"无需模式切换"的立场 |
| **涌现人格（长期使用形成）** | AIEOS 预设人格 | 根本冲突 | 🔴 | 坚决反对预设人格，坚持"无人格也是一种人格"。长期使用中自然形成的偏好用 devbase 日志统计，不用人格模板 |

### 3.2 "claw"命名风险

| 风险 | 说明 | 风险等级 |
|------|------|---------|
| OpenClaw（250K Stars）已占据 "claw" 心智 | 新用户搜索 "claw AI" 首先看到 OpenClaw | 🔴 |
| ZeroClaw（20K Stars）强化 "claw = AI 助手" | Rust 生态中 claw 概念已与 ZeroClaw 绑定 | 🔴 |
| Clarity 使用 "claw" 作为层名 | 容易被视为 OpenClaw/ZeroClaw 的模仿或分支 | 🟡 |

**应对方案**：
- 方案 A：保留 claw，但强调 "Clarity 的 claw 不是 OpenClaw 的 claw"——claw 是系统托盘常驻层，不是聊天机器人
- 方案 B：改名（如 tray/lurker/edge），彻底避免命名冲突
- **建议**：先保留 claw，在文档中明确区分。若未来品牌冲突升级，再考虑改名。

---

## 四、竞争与时间风险

### 4.1 竞品迭代速度

| 竞品 | Stars | 迭代特征 | 对 Clarity 的时间压力 |
|------|------|---------|---------------------|
| **OpenClaw** | 250K | 社区驱动， Skill 生态每日新增 | 生态壁垒每天都在增厚 |
| **ZeroClaw** | 20K | Harvard/MIT 背景，快速迭代 | 功能覆盖速度可能超过 Clarity 吸收速度 |
| **Claurst** | 8,272 | 个人/小团队，敏捷迭代 | 8K Stars 意味着社区反馈多，功能进化快 |
| **5ire** | 5,151 | Electron 生态成熟，UI 迭代快 | UI/UX 差距可能在扩大而非缩小 |
| **OpenHanako** | — | 个人项目，迭代不稳定 | 风险较低 |

### 4.2 关键时间风险

| 风险 | 说明 | 风险等级 | 时间窗口 |
|------|------|---------|---------|
| **ZeroClaw 引入入口差异化** | ZeroClaw 若为其 CLI/Web/通道设计不同行为模式，Clarity 的"三层认知分化"将不再是独特优势 | ⚫ | 不可预测，可能在 3-6 个月内 |
| **5ire 扩展 CLI/TUI 入口** | 5ire 作为 Electron 应用扩展 CLI 模式在技术上可行。一旦实现，单点优势 + 多入口 = Clarity 噩梦 | 🔴 | 6-12 个月 |
| **OpenClaw 推出 Rust 原生版本** | 若 OpenClaw 官方推出 Rust 版本（而非社区 ZeroClaw），将直接终结 Clarity 的 Rust 生态差异化 | 🔴 | 不可预测 |
| **Claurst 引入三层设计** | Claurst 社区若认可"不同场景不同行为"的设计理念，可能快速引入类似机制 | 🟡 | 3-6 个月 |

### 4.3 Clarity 的吸收速度 vs 竞品进化速度

```
          Clarity 吸收速度          竞品进化速度
              ▲                        ▲
              │    Trait 架构          │    ZeroClaw 功能扩展
    高        │    RAG 向量库          │    5ire UI 迭代
    难        │    文档解析            │    Claurst 社区贡献
    度        │                        │
              │    Plugin 系统         │
              │    Sub-agents          │
              │    Sandbox             │
              │                        │
    低        │    Compaction          │    OpenHanako 不稳定
    难        │    Cost tracking       │
    度        │    Markdown 渲染       │
              │    Markdown 配置       │
              └────────────►           └────────────►
                   时间                      时间
```

**核心判断**：Clarity 在高难度项（RAG、Trait 架构、文档解析）上的吸收速度**大概率慢于**竞品在对应维度的进化速度。这意味着 Clarity 不能靠"功能追赶"取胜，必须靠"设计哲学差异化"（三层认知分化）+ "快速吸收低难度项"建立渐进优势。

---

## 五、风险综合矩阵

| 营养项 | 法律风险 | 技术风险 | 概念风险 | 时间风险 | 综合等级 | 建议行动 |
|--------|---------|---------|---------|---------|---------|---------|
| Compaction | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 | **立即推进** |
| Cost tracking | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 | **立即推进** |
| Markdown 渲染 | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 | **立即推进** |
| Markdown 配置 | 🟢 | 🟢 | 🟢 | 🟢 | 🟢 | **立即推进** |
| Sandbox | 🟢 | 🟡 | 🟢 | 🟢 | 🟡 | 评估后推进 |
| SOPs 工作流 | 🟢 | 🟡 | 🟢 | 🟢 | 🟡 | 评估后推进 |
| Skill 目录 | 🟡 | 🟡 | 🟢 | 🟡 | 🟡 | 评估后推进 |
| 模块化启动 | 🟢 | 🟡 | 🟢 | 🟢 | 🟡 | 评估后推进 |
| Plugin 包装层 | 🟢 | 🟡 | 🟡 | 🟡 | 🟡 | 评估后推进 |
| Sub-agents | 🟢 | 🟡 | 🟡 | 🟡 | 🟡 | 评估后推进 |
| RAG 向量库 | 🟢 | 🔴 | 🟢 | 🔴 | 🔴 | 充分准备后推进 |
| 文档解析 | 🟢 | 🔴 | 🟢 | 🔴 | 🔴 | 充分准备后推进 |
| Trait 架构 | 🟢 | 🔴 | 🟡 | 🔴 | 🔴 | 充分准备后推进 |
| "claw" 命名 | 🟡 | 🟢 | 🟡 | 🟡 | 🟡 | 监控，必要时改名 |
| AIEOS 兼容 | 🔴 | 🟢 | 🔴 | 🟢 | 🔴 | 不兼容，仅吸收思想 |

---

## 六、执行建议

### 6.1 立即推进（🟢 P2 项，无实质风险）

1. **Compaction**：对话自动压缩，减少 LLM 上下文占用
2. **Cost tracking**：Token 成本追踪，提升用户透明度
3. **Markdown 渲染**：TUI 中的 Markdown 显示，提升阅读体验
4. **Markdown 配置**：系统提示词抽离为 Markdown 文件，便于维护

### 6.2 评估后推进（🟡 P1 项，可控风险）

1. **Sandbox**：先实现应用级文件访问白名单，OS 级沙箱作为高级选项
2. **SOPs 工作流**：先用简单的事件触发（文件变化/Git 提交），逐步扩展
3. **Skill 目录**：先用 GitHub 仓库 + 10-20 个官方 Skill 启动
4. **模块化启动**：先优化 Gateway 启动流程，逐步引入 lazy init
5. **Plugin 包装层**：先用 WASM 运行时，简化跨平台问题
6. **Sub-agents**：先实现"工具调用包装"，逐步引入真正的上下文隔离

### 6.3 充分准备后推进（🔴 P0 项，高风险）

1. **RAG 向量库**：先调研 ollama 嵌入 + sqlite-vss 的可行性，再决定是否自建
2. **文档解析**：先用 Python 桥接方案，验证需求后再评估 Rust 原生方案
3. **Trait 架构**：先设计 Trait 边界，分阶段改造（Provider → Memory → Tool）

### 6.4 暂停或改造（⚫ 极高风险项）

1. **AIEOS 兼容**：不直接兼容，仅吸收"四维配置"思想改造为场景配置
2. **多智能体架构**：不吸收，用 Sub-agents 替代
3. **"claw" 命名**：监控品牌冲突，必要时改名

---

*报告结束*
