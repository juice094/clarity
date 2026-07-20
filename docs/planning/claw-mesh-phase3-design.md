# Claw Mesh Phase 3：分布式角色上下文同步设计

> 状态：设计草案 → **骨架已落地（2026-07-20 注）**：`clarity-claw/src/mesh/` 已实现 CRDT merger、gateway/syncthing 双 transport、crypto 占位；E2EE 按本文决策延后到 Phase 3.5。本文作为剩余工作的设计参考继续有效。  
> 设计参考：Matrix 协议（用户身份 → 联邦服务器 → 多设备同步 → 端到端加密）

## 1. 目标

让同一个 Claw `role` 下的多台设备/节点共享同一份持久化会话上下文，离线可用、上线自同步、冲突可收敛。同时保留现有以 `clarity-gateway` 为中心的在线协作路径。

## 2. Matrix → Claw Mesh 概念映射

| Matrix 概念 | Claw Mesh 对应物 | 说明 |
|------------|------------------|------|
| User (`@user:server`) | **Role** | Claw 的“角色”就是分布式身份。同 role 多设备共享同一份意识。 |
| Room | **Role Context** | 一个 role 对应一个上下文房间，包含消息、设置、记忆引用、生命周期状态。 |
| Room state events | **Context Events** | 对上下文的每次变更都是一条事件，带全局排序元数据。 |
| Client / Device | **Claw Device Node** | 运行 Clarity 的桌面、移动端、headless 实例。 |
| Homeserver | **clarity-gateway** | 始终在线的协调者，负责离线设备的持久化和联邦中继。 |
| Federation | **Gateway ↔ Gateway 或 P2P** | 跨 Gateway 的角色上下文同步；同 LAN/账户内可走 syncthing-rust 直连。 |
| `/sync` | **`/claw/sync`** | 设备上线后拉取自上次同步以来缺失的事件。 |
| E2EE (Olm/Megolm) | **可选设备密钥加密** | 敏感上下文 payload 可用设备密钥加密，Gateway 只存储密文。 |

## 3. 拓扑选择

沿用用户先前选定的 **A + C 方案**：

- **A. 中心 Gateway**：每个 role 的权威事件日志由 `clarity-gateway` 持久化，提供在线 `/claw/sync`、冲突裁决、离线暂存。
- **C. syncthing-rust P2P**：同账户/同 LAN 的设备之间通过 syncthing-rust 的 BEP 协议直接同步大块数据（历史消息、附件），降低 Gateway 带宽压力，支持离线场景。

 Gateway 与 syncthing-rust 互补：
- Gateway 负责“在线时的小事件实时同步”和“冲突仲裁”。
- syncthing-rust 负责“大批量历史/文件同步”和“离线后重连的批量补齐”。

## 4. 数据模型：Role Context as CRDT

### 4.1 Context Event

```rust
pub struct ContextEvent {
    /// 事件唯一 id：hash(origin_device_id, origin_clock, payload)
    pub event_id: String,
    /// 产生事件的设备 id
    pub origin_device: String,
    /// 产生事件的逻辑时间（Lamport / HLC）
    pub origin_clock: u64,
    /// 事件类型
    pub kind: ContextEventKind,
    /// 事件 payload（加密后可选）
    pub payload: Vec<u8>,
}

pub enum ContextEventKind {
    AppendMessage,
    EditMessage,
    ArchiveSession,
    SetLifecycle,
    UpdateMetadata,
}
```

### 4.2 冲突解决策略

按字段类型选择 CRDT：

| 字段类型 | CRDT | 示例 |
|---------|------|------|
| 标量/最后写入 | LWW-register | `lifecycle`, `archived`, `project_id` |
| 消息列表 | LWW-element-set + 全序 tie-breaker | `messages`（以 `event_id` 字典序定序） |
| 计数器 | PN-counter | token 预算、未读计数 |
| 集合 | OR-set | 关联的 device_id 集合、标签 |

 ponytail: 先实现 LWW + LWW-element-set，覆盖 95% 场景；PN-counter/OR-set 待真有计数器需求时再引入。

## 5. 同步协议

### 5.1 在线路径：`/claw/sync`

设备上线或重连时：

```http
GET /claw/sync?role=operator&since=last_event_id&device=device_abc
```

Gateway 返回：
- `events`: 自 `since` 之后的事件列表
- `next_batch`: 下一次请求的游标
- `presence`: 当前在线的 role 设备列表

实现上复用现有 Gateway WebSocket 通道，新增 `ClawSync` 消息类型，避免额外 HTTP 轮询。

### 5.2 离线路径：syncthing-rust BEP

`syncthing-rust` 当前是完整的文件同步实现；其 BEP `MessageType` 枚举固定（0–7），不支持自定义消息类型，因此 Claw Mesh 采用**虚拟文件同步**策略，而不是扩展 BEP 消息。

#### 5.2.1 角色 → Syncthing Folder

每个 role 映射为一个 Syncthing folder：

| 字段 | 取值 |
|------|------|
| `folder_id` | `claw:role:{role}`，例如 `claw:role:operator` |
| `path` | `~/.clarity/mesh/{role}/` |
| `folder_type` | `sendreceive`（角色内所有节点可读可写） |
| 共享设备 | 该 role 下所有已配对 Claw 设备 + Gateway 节点 |

#### 5.2.2 文件布局

为了避免追加写入导致整个文件反复同步，每个事件存为独立文件：

```text
~/.clarity/mesh/{role}/
├── events/
│   ├── 00000001-{event_id}.json   # origin_clock 填充 + event_id
│   ├── 00000002-{event_id}.json
│   └── ...
├── state.json                      # 合并后的当前状态快照（本地缓存，不强制同步）
└── .stignore                       # 忽略 state.json 等派生文件
```

- 文件名前缀 `origin_clock` 保证目录列举即有序。
- `state.json` 只是本地合并缓存；真实来源是 `events/` 目录。
- Gateway 也作为一个 Syncthing peer 持有完整 `events/` 目录，提供“始终在线”的 fallback。

#### 5.2.3 同步 API 封装

在 `clarity-openclaw` 中新增 `mesh::sync` 模块，封装 `syncthing-sync::SyncManager`：

```rust
use syncthing_sync::{SyncManager, Folder, DeviceId};

pub struct RoleSyncTransport {
    manager: Arc<dyn SyncManager>,
    role: String,
    events_dir: PathBuf,
}

impl RoleSyncTransport {
    /// 加入一个 role 的 Syncthing folder，并监听同步事件。
    pub async fn join_role(&self, role: &str, peers: &[DeviceId]) -> Result<()>;

    /// 将本地新生成的事件写入 events/，触发 Syncthing 主动 push。
    pub async fn publish_event(&self, event: &ContextEvent) -> Result<()>;

    /// 读取本地 events/ 目录，返回按 origin_clock 排序的全部事件。
    pub async fn collect_events(&self) -> Vec<ContextEvent>;

    /// 订阅 Syncthing 同步完成事件，用于驱动 CRDT 合并。
    pub fn subscribe(&self) -> syncthing_sync::EventSubscriber;
}
```

- 依赖方式：`syncthing-sync` 通过 path 依赖引入本地 `syncthing-rust` workspace；待其发布 crates.io 后改为 registry 依赖。
- `clarity-core` 不直接依赖 `syncthing-sync`，只依赖 `clarity-openclaw` 暴露的抽象 trait。

#### 5.2.4 离线 → 在线切换

1. 设备离线时，事件写入本地 `events/`；Syncthing 在后台尝试连接 peer。
2. 重新上线后，Syncthing 自动同步缺失的事件文件。
3. `RoleSyncTransport` 收到 `SyncEvent::FolderCompleted` 后，调用 `collect_events()`。
4. 合并器与 Gateway `/claw/sync` 返回的在线事件合并，去重后应用。

### 5.3 合并流程

1. 收集所有来源的事件：Gateway `/sync` + syncthing 文件 + 本地未发送队列。
2. 按 `(origin_clock, event_id)` 全序排序。
3. 对每个事件幂等应用：若 `event_id` 已存在则跳过。
4. 对同一字段的并发更新按 CRDT 语义收敛。
5. 将合并后的状态持久化到本地 SQLite，并触发 `UiEvent::SessionUpdated`。

## 6. 安全

- **传输层**：Gateway 路径继续用 `rustls-tls`（已落地）；syncthing-rust BEP 自带 TLS。
- **端到端加密（可选）**：为每个 role 生成 Ed25519 设备密钥对，敏感 payload 用 Megolm 风格的会话密钥加密，Gateway 只存储密文。Clarity 已有 `clarity-secrets` 的 ChaCha20-Poly1305，可复用其加密原语。
- **授权**：沿用现有 OpenClaw 设备配对和 capability token，只有已配对设备才能订阅 role 上下文。

## 7. 实现步骤（建议顺序）

1. **定义事件模型**：在 `clarity-contract` 新增 `ClawContextEvent`、`RoleContextId`；保持零内部依赖。
2. **Gateway `/claw/sync`**：在 `clarity-gateway` 新增 REST/WebSocket API，持久化每个 role 的事件日志。
3. **本地合并器**：在 `clarity-core` 新增 `RoleContextMerger`，负责把事件应用到本地会话状态。
4. **OpenClaw 协议扩展**：在 `clarity-openclaw` 的 `ProtocolEvent/ProtocolCommand` 中新增 sync 相关命令/事件。
5. **egui 同步触发**：在 `clarity-egui` 连接建立后自动发送 `sync` 请求，并处理 `SessionUpdated` 事件。
6. **syncthing-rust 集成**：
   - 在 `clarity-openclaw` 新增 `mesh::syncthing` 模块，封装 `syncthing-sync::SyncManager`。
   - 每个 role 对应一个 Syncthing folder，事件以独立文件存储。
   - 监听 `SyncEvent` 驱动 CRDT 合并。
   - Cargo 依赖临时使用 path 指向 `../../../syncthing-rust/crates/syncthing-sync`，并用 `// ponytail:` 标记待发布到 crates.io 后迁移。
7. **端到端加密**：在基础同步跑通后，为 payload 加可选加密层。

## 8. 与现有 Phase 1/2 的衔接

- Phase 1 已经把 `claw_session_key` 固定为 `agent:main:{role}`，天然适合作为 Role Context Id。
- Phase 2 的 `ClawConnectionManager` 统一了协议入口，新增 `Sync` 命令只需扩展 `ProtocolCommand` 即可，不需要再改 egui 的连接创建逻辑。

## 9. 已确认决策

1. **syncthing-rust 集成方式**：采用虚拟文件同步。BEP 消息类型固定（0–7），不扩展协议；每个 role 对应一个 Syncthing folder，每个事件对应一个文件。
2. **依赖引入**：**B，git submodule**。`third_party/syncthing-rust` 作为 submodule，路径稳定、跨机器可构建。A 的 `../../../syncthing-rust` 与分布式愿景矛盾；C 阻塞开发。
3. **CRDT 排序**：**`event_id` 字典序 tie-breaker 足够，HLC 延后**。Claw mesh 事件是独立、幂 agent 状态更新；actor_id + seq 已保证单 actor 内有序。后续若出现跨 role 因果依赖再引入 HLC。
4. **E2EE**：**延后到 Phase 3.5**。Phase 3 骨架（BEP 虚拟文件同步、CRDT 合并、多用户白名单路由）已足够重；BEP 本身有 TLS 保证传输安全。E2EE 解决 at-rest 隔离，属于生产化需求。

## 10. 待决策 / 待实现时细化

- `third_party/syncthing-rust` 的初始 commit/tag 选哪个？（建议用当前 `main` 的某个稳定 commit）
- submodule 是否 `--shallow` 以减小体积？
- 是否需要为 `clarity-openclaw` 新增 `mesh` feature，以便无 submodule 时仍可编译？
