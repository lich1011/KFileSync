# KFileSync 全项目审核总结

> 审核时间：2026-04-30  
> 审核范围：`src-tauri/src/` 全部 60 个 `.rs` 源文件 + 9 个测试文件 + `Cargo.toml`  
> 覆盖阶段：Phase 0 (基础架构) → Phase 5 (网络层)

---

## 一、项目现状

| 指标 | 结果 |
|------|------|
| `cargo clippy` | **0 警告** ✅ |
| `cargo test` | **31/31 通过** ✅ |
| 源文件数 | 60 |
| 测试文件数 | 9 |
| 依赖数 | 23 crates |

### 架构概览

```
┌────────────────────────────────────────────────┐
│                 interfaces/                     │
│              (tauri_cmds.rs)                    │
├────────────────────────────────────────────────┤
│               application/                      │
│  identity_service  share_service  transfer_svc  │
│  indexer_service   http_sync_flow  sync_flow    │
├────────────────────────────────────────────────┤
│                 domain/                         │
│  model/   port/   service/   event/   error     │
├────────────────────────────────────────────────┤
│              infrastructure/                    │
│  persistence/  network/  events/  security/     │
│  system/                                        │
└────────────────────────────────────────────────┘
```

---

## 二、已修复问题 (15 项)

### Phase 5 专项审核 (7 项)

| # | 级别 | 问题 | 修复方式 |
|---|------|------|----------|
| P5-S1 | 🔴 严重 | PEM 私钥编码未折行，TLS 无法启动 | 改用 `rcgen::serialize_private_key_pem()` |
| P5-S2 | 🔴 严重 | HTTP Server Handler 返回硬编码数据 | 注入 `DeviceRepo`/`FileIndexRepo`/`ShareRepo`，handler 调用真实服务 |
| P5-M1 | 🟡 中等 | 端口 `53317` 在 4 处硬编码 | 定义 `pub const DEFAULT_PORT: u16 = 53317` |
| P5-M2 | 🟡 中等 | DTO 定义在 `domain::port` 违反六边形 | DTO 移至 `infrastructure::network::dto`；`NetworkClient` trait 改用领域值对象 |
| P5-M3 | 🟡 中等 | `RevokedData` 缺失 `address` 字段 | 添加字段并在 `revoke()` 中透传 |
| P5-L1 | 🟢 轻微 | `HttpSyncFlow::generate_plan` 返回空计划 | 注入 `FileIndexRepository`，调用 `SyncPlanGenerator::generate()` |
| P5-L2 | 🟢 轻微 | `base64` 用法在函数体内 | 随 P5-S1 一并消除 |

### 全项目审核 (8 项)

| # | 级别 | 问题 | 修复方式 |
|---|------|------|----------|
| G-S1 | 🔴 严重 | DeviceId 每次启动重新生成，配对全部失效 | 持久化到 `.keystore/device_id` 文件 |
| G-M2 | 🟡 中等 | `specification::SyncAction` 与 `file_entry::SyncAction` 类型名冲突 | 重命名为 `SyncDirection` |
| G-M3 | 🟡 中等 | IndexerService 存储绝对路径导致跨设备索引无法匹配 | `strip_prefix(share_root)` 转为相对路径 |
| G-M4 | 🟡 中等 | AuditEventHandler 双层 `tokio::spawn` | 去掉内层 spawn，`start()` 直接 await 事件循环 |
| G-L1 | 🟢 轻微 | `base64` crate 已无代码引用，残留在 Cargo.toml | 从 `[dependencies]` 移除 |
| G-L2 | 🟢 轻微 | `discover_devices` 中 mDNS listener task 泄漏 | `JoinHandle::abort()` 在 timeout 后清理 |
| G-L3 | 🟢 轻微 | `lib.rs` 多处 `unwrap()` 启动时可能 panic | 改为 `expect("...")` 附带描述性错误信息 |

---

## 三、尚未修复的问题 (2 项)

### 🟡 G-M1: SQLite 多连接不共享 WAL 模式

**状态**: ⏩ 推迟到下一阶段

**问题描述**:  
5 个 SQLite Repository (`device`, `audit`, `transfer`, `share`, `file_index`) 各自独立调用 `Connection::open("lansync.db")`，导致：

1. **WAL 不一致**: 只有 `SqliteFileIndexRepository` 设置了 `PRAGMA journal_mode = WAL`，其他 4 个使用默认的 DELETE 模式。同一个数据库文件上混合两种日志模式，行为未定义。
2. **并发风险**: 多个独立连接并发写入可能触发 `SQLITE_BUSY` 错误。
3. **无法跨 Repo 事务**: 例如"配对成功后同时创建 Device + Share 成员"需要原子操作，但当前做不到。

**建议修复方案**:
```rust
// 创建共享连接池
pub struct DbPool {
    conn: Arc<Mutex<Connection>>,
}

impl DbPool {
    pub fn new(db_path: &str) -> Result<Self, String> {
        let conn = Connection::open(db_path).map_err(|e| e.to_string())?;
        conn.execute_batch(
            "PRAGMA journal_mode = WAL;
             PRAGMA synchronous = NORMAL;
             PRAGMA foreign_keys = ON;"
        ).map_err(|e| e.to_string())?;
        Ok(Self { conn: Arc::new(Mutex::new(conn)) })
    }

    pub fn connection(&self) -> Arc<Mutex<Connection>> {
        self.conn.clone()
    }
}
```

然后在 `lib.rs` 中初始化一次 `DbPool`，注入所有 Repo。

**影响面**: 需要修改 5 个 Repo 的构造函数 + `lib.rs` 的 DI 逻辑。  
**当前风险**: MVP 阶段实际并发写入极少，短期不会触发。

---

### 🟡 P5-知识项: HTTP Server 安全性待加固

**状态**: ⏩ Phase 6 规划

**问题描述**:  
当前 `ReqwestNetworkClient` 使用 `danger_accept_invalid_certs(true)` 跳过证书验证。接收端的 `handle_pair_request` 也没有验证请求方身份。

**待办事项**:
1. **证书指纹校验**: 配对成功后，将对端证书指纹存入 `PairedData`。后续连接时校验 TLS 证书指纹是否匹配。
2. **API 鉴权**: HTTP 端点添加 Bearer Token 或双向 TLS (mTLS) 认证。
3. **Anti-Replay**: 服务端应验证 `PairRequestDto` 中的 `nonce` 和 `timestamp`（当前生成了但未校验）。
4. **IP 地址获取**: `handle_pair_request` 中应从连接信息提取对端 IP，填入 `DiscoveredData.address`。

---

## 四、项目健康度评估

```
领域模型 (Domain)      ████████████████████ 100%  — 状态机、VersionVector、冲突解析均完整
应用服务 (Application)  ███████████████████░  95%  — IndexerService 文件哈希仍为 mock
基础设施 (Infra)        ██████████████████░░  90%  — SQLite 连接池待重构
网络层 (Network)        ████████████████░░░░  80%  — TLS 安全待加固
接口层 (Interfaces)     ████████████████████ 100%  — Tauri 命令全部连通
测试覆盖               ████████████████░░░░  80%  — 纯领域逻辑覆盖好，集成测试缺失
```

### 下阶段优先级建议

1. **P0**: SQLite 连接池重构 (G-M1)
2. **P0**: HTTP API 鉴权 + 证书指纹校验
3. **P1**: IndexerService 集成真实 SHA-256 哈希
4. **P1**: 文件传输流式 API (streaming transfer)
5. **P2**: 集成测试 (端到端 pair → share → sync)
