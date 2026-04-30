# KFileSync — 项目开发总结文档

> **文档版本**: `v1.0`  
> **编写时间**: `2026-04-14`  
> **当前进度**: 阶段 0 (基础PoC) ✅ & 阶段 1 (身份与配对 / Identity Context) ✅  
> **设计依据**: 《局域网零信任直传与同步桌面应用 — 完整设计开发文档 v3.md》  
> **适合阅读对象**: 人类开发者 / 接管任务的 AI Agent

---

## 一、项目背景与目标

**KFileSync** 是一款运行于局域网内、基于**零信任架构**的跨平台文件直传与同步桌面应用，目标平台覆盖 macOS / Windows。

核心安全原则：
- **设备即身份**：每台设备通过 Ed25519 密钥对生成唯一 `DeviceId`（SHA-256 指纹）
- **从不默认信任**：设备发现后须通过配对确认才建立信任链；信任可随时撤销并触发级联清理
- **传输有据可查**：所有配对与传输操作通过事件总线落入审计日志

---

## 二、技术选型

| 层次 | 技术 | 版本 / 说明 |
|------|------|-------------|
| 前端框架 | Vue 3 + Vite + TypeScript | 组合式 API，SFC 单文件组件 |
| 桌面容器 | Tauri 2 | 与 Rust 层通过 IPC Commands 交互 |
| 业务引擎 | Rust (Stable) | 六边形架构 + DDD，edition 2021 |
| 异步运行时 | Tokio 1 | `features = ["full"]` |
| 身份密码学 | ed25519-dalek 2 | 密钥对生成，rand_core 特性 |
| 文件哈希 | sha2 (SHA-256) + blake3 | 设备指纹 + 分块校验 |
| 数据持久化 | rusqlite 0.31 | SQLite with WAL mode, bundled |
| 设备发现 | mdns-sd 0.13 | mDNS/Bonjour 策略（接口已定义） |
| 传输安全 | rustls 0.23 + rcgen 0.13 | TLS 1.3，Ed25519 自签名证书 |
| 序列化 | serde 1 + serde_json 1 | 领域对象 JSON 落地 |

**架构设计模式**: 六边形架构（Ports & Adapters）+ 领域驱动设计（DDD）

---

## 三、项目目录结构

```
KFileSync/
├── KFILESYNC_STATE.md          # ← 本文档
├── index.html                  # Vite HTML 入口
├── vite.config.ts
├── package.json
├── tsconfig.json / tsconfig.app.json / tsconfig.node.json
│
├── src/                         # 前端 (Vue 3)
│   ├── main.ts                  # Vue 应用入口
│   ├── App.vue                  # 根组件，集成 DeviceList
│   ├── style.css
│   ├── assets/
│   └── components/
│       ├── DeviceList.vue       # ✅ 设备列表 + 配对按钮
│       └── HelloWorld.vue       # Vite 默认模板（保留）
│
└── src-tauri/                   # Rust 后端 (Tauri 2 + DDD)
    ├── Cargo.toml               # 含所有域依赖声明
    ├── tauri.conf.json          # Tauri 配置（已接 Vite devUrl）
    ├── build.rs
    │
    ├── src/
    │   ├── lib.rs               # 模块注册入口 (pub mod 声明)
    │   ├── main.rs
    │   │
    │   ├── domain/              # 🔐 核心业务领域（零外部依赖）
    │   │   ├── mod.rs
    │   │   ├── model/
    │   │   │   ├── device.rs    # ✅ DeviceId, DeviceState 状态机
    │   │   │   ├── pairing.rs   # ✅ PairingSession（5分钟超时）
    │   │   │   ├── file_entry.rs # ✅ VersionVector 冲突向量
    │   │   │   └── share.rs     # ✅ SharePermission 授权模型
    │   │   ├── port/
    │   │   │   ├── repository.rs # ✅ DeviceRepository trait
    │   │   │   ├── key_store.rs  # ✅ KeyStore trait（抽象密钥仓）
    │   │   │   └── event_bus.rs  # ✅ EventBus + DomainEvent trait
    │   │   └── event/
    │   │       └── identity.rs   # ✅ DeviceDiscovered / PairingCompleted / TrustRevoked
    │   │
    │   ├── application/          # 应用服务层（空骨架，阶段 2 填充）
    │   │   └── mod.rs
    │   │
    │   ├── infrastructure/       # 🔌 外部适配器
    │   │   ├── mod.rs
    │   │   ├── events/
    │   │   │   └── in_process_bus.rs  # ✅ Tokio broadcast 事件总线
    │   │   └── persistence/
    │   │       └── sqlite_device_repo.rs # ✅ SQLite 设备存储
    │   │
    │   └── interfaces/           # IPC/REST 接口层（空骨架，阶段 2 填充）
    │       └── mod.rs
    │
    └── tests/
        ├── unit.rs               # 测试注册表
        └── unit/
            ├── device_state_test.rs   # ✅ 状态机合法/非法转移验证
            └── version_vector_test.rs # ✅ 向量时钟并发冲突验证
```

---

## 四、核心模型详解

### 4.1 DeviceState 状态机

状态图：`Discovered --> Paired --> Revoked`

```
DeviceState::Discovered(alias, address)
    │
    ▼ confirm_pairing(cert, timestamp)
DeviceState::Paired(alias, certificate, paired_at)
    │
    ▼ revoke(timestamp)
DeviceState::Revoked(alias, certificate, revoked_at)
```

**非法转移**（均返回 `DomainError::InvalidStateTransition`）：
- `Discovered.revoke()` → ❌
- `Paired.confirm_pairing()` → ❌（已配对不可重复配对）
- `Revoked.*` → ❌（终态不可转移）

### 4.2 VersionVector（向量时钟）

用于多设备并发编辑时的冲突判断：

| 方法 | 语义 |
|------|------|
| `is_ancestor_of(other)` | self 是 other 的前驱版本（可安全合并） |
| `conflicts_with(other)` | 两者互不先行，存在冲突 |
| `increment(device_id)` | 本设备提交新版本 |
| `merge(other)` | 取各设备时钟最大值，合并两向量 |

### 4.3 PairingSession

| 字段 | 说明 |
|------|------|
| `id` | 会话唯一标识 |
| `target_device` | 被配对的设备 `DeviceId` |
| `pin_code` | 配对 PIN 码（6位数字）|
| `expires_at` | UNIX 时间戳，超时（5分钟）后拒绝 |

验证逻辑：`verify(code, current_time)` 同时校验时效性与 PIN 码匹配。

### 4.4 事件系统

```
DomainEvent (trait)
├── DeviceDiscovered { device_id, alias }
├── PairingCompleted { local_device, peer_device, paired_at }
└── TrustRevoked     { device_id, revoked_at }

EventBus (trait)
└── InProcessEventBus  ← tokio::sync::broadcast::channel(100)
```

---

## 五、测试状态

```
运行命令: cargo test (目录: src-tauri/)

test unit::device_state_test::test_valid_transitions    ... ✅ ok
test unit::device_state_test::test_invalid_transitions  ... ✅ ok
test unit::version_vector_test::test_is_ancestor_of     ... ✅ ok
test unit::version_vector_test::test_conflicts_with     ... ✅ ok
test unit::version_vector_test::test_merge              ... ✅ ok

test result: ok. 5 passed; 0 failed; finished in 0.00s
```

---

## 六、Phase 0 & 1 跳过/延迟项记录

以下验收项经评估后暂时通过 Mock / 抽象接口满足，不阻碍核心架构，待后续阶段环境成熟后回补：

| 编号 | 描述 | 原因 | 状态 |
|------|------|------|------|
| T1.6 | `AuditEventHandler` / `CascadeCleanupHandler` 注册 | EventBus 基础设施已就绪，具体 Handler 待应用层完善 | 🔜 下阶段 |
| T1.7 | Pinia 状态管理接入 | 当前 Vue 组件以 `ref` 本地状态完成 PoC，Pinia 在阶段 2 引入 | 🔜 下阶段 |

---

## 3. 代码质量及修复记录
近期进行了一轮深度代码审查并完成了以下重点修复：
- **🔴 架构整改**: `DomainError` 已从模型层剥离至统一错误中心 `domain/error.rs`。
- **🔴 安全增强**: `PairingSession` 移除可预测的 ID 生成，改为使用随机 UUID。
- **🔴 逻辑闭环**: `SqliteDeviceRepository` 已完整实现 `find_paired` 与 `update_trust_status`。
- **🟡 并发警告**: 对公共 Trait 中的 `async fn` 警告进行了合规性处理。
- **🟡 健壮性**: `Certificate` 现在具备 PEM 基础格式校验；`InProcessEventBus` 增加了发送失败监控。

---

## 4. 下游交接上下文 (Next Steps / Phase 2)

**阶段 2 目标**: 完成 Transfer Context，落地策略模式和工厂模式，实现文件单次直传 MVP。

### 接手认知要点

1. **工作目录**: `/Users/luokai/Github/demoproject/KFileSync/`
2. **构建命令**: 
   - Rust 测试: `cd src-tauri && cargo test`
   - 前端开发: `npm run dev`（Vite 端口 5173）
3. **下一阶段起点文件**: 
   - 新建 `src-tauri/src/domain/model/transfer.rs`
   - 新建 `src-tauri/src/infrastructure/persistence/sqlite_transfer_repo.rs`
4. **关键任务（根据 v3.md T2.x）**:
   - [ ] `TransferJob` 聚合根 + 状态机 (`Pending → Active → Verifying → Completed/Failed/Cancelled`)
   - [ ] `TransferJob::create_from_files()` 工厂方法（文件枚举 + `SizeBasedChunking` 分块 + manifest 构建）
   - [ ] `PolicyEnforcer` 验证设备信任状态
   - [ ] 数据通道传输 + 逐块 BLAKE3 校验 + ACK
   - [ ] 断点续传（`chunks_done` Checkpoint 值对象）
   - [ ] `SqliteTransferRepository` 持久化
   - [ ] `TransferAppService` 应用服务编排
   - [ ] 前端传输 UI：文件选择 + 进度面板 + 历史列表

---

*本文档由 AI Agent 自动生成并维护，与代码库同步更新。*
