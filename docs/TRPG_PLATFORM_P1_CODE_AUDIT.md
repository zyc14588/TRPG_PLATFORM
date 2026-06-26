# TRPG_PLATFORM P1 代码审计报告

审计日期：2026-06-26  
审计对象：`TRPG_PLATFORM_P1(1).rar` 解压后的源码快照  
源码根目录：`/mnt/data/TRPG_SRC/TRPG_PLATFORM`

## 1. 总结结论

**不建议把当前快照判定为“P1 无疏漏、可直接进入 P2 实现”。**

当前仓库具备 P1 的工程骨架、Auth/Room REST 纵向链路、ABAC/RLS 迁移、前端 API client、OpenAPI 静态合同和若干测试。但是，审计发现几个会直接影响 P1 交付可信度和 P2 Codex 执行质量的问题：

1. `docs/status/P1_AUDIT.md` 声称已经把 `rag_core` 扩展为可执行 RAG kernel，但实际 `crates/rag_core/src/lib.rs` 仍只有 65 行左右的 trait/type 骨架；`document_ingestor` 也没有委托给 `rag_core`。
2. P2 阶段定义冲突：产品设计文档把 Phase 2 定为 RAG，旧版 `prompts/02_REALTIME_CONCURRENCY.md` 又把 Phase 2 定为实时并发，旧版 `prompts/03_RULES_RAG.md` 把 RAG 写成 Phase 3。
3. 服务层 idempotency 是“先声明完成、后写业务状态”，在写入失败或重试时可能返回未实际落库的成功响应。
4. `accept_room_invitation` 的幂等重试路径有逻辑缺陷：首次接受后 invite 不再是 pending，同 token/key 重试会在查找 pending invite 时返回 404，无法 replay 原响应。
5. 生产启动路径在缺少 `DATABASE_URL` 时会退回 `InMemoryAuthStore`，这会造成生产误配置下无持久化、无数据库 RLS 的危险运行状态。
6. 认证私有表启用了 `FORCE ROW LEVEL SECURITY` 且部分 policy 为 `USING(false)`，但 repository 访问 magic link、refresh session、idempotency key 时没有通过事务设置 RLS 上下文或使用安全函数；在非超级用户生产角色下极可能失败，而超级用户测试会掩盖问题。
7. Phase 2A 的 RAG schema/RLS 对 `documents`、`chunks` 以及 room-scoped `document_sources` 没有完整强制 `license_status='allowed'`，与“pending/denied 不可索引/检索”的法律边界不一致。

因此，建议先做一个 **P1.5 Stabilization**：修正 P1 安全与文档一致性问题，再把 RAG 作为统一的 P2 交给 Codex。

## 2. 审计范围与限制

### 已完成

- 解压并建立源码索引。
- 静态审计 Rust workspace、Axum server、auth/storage crate、migrations、frontend API client、OpenAPI 与 P1/P2 设计文档。
- 搜索高风险代码模式：`unsafe`、生产路径 `unwrap/expect/panic/todo/unimplemented`、硬编码 secret、token、SQL、cookie、RLS、idempotency、license、RAG。
- 对照 P1 报告、P1 Audit、RAG/P2 设计文档和实际代码。

### 未完成 / 未能复验

当前执行环境没有 Rust 工具链和 pnpm：

- `cargo` / `rustc` 不存在，无法实际执行 `cargo fmt/check/clippy/test/sqlx`。
- `pnpm` 不存在，无法复验 `pnpm lint/typecheck/test/e2e/build`。
- 因此，仓库文档中声称的测试通过只能视为项目方本地记录，我没有在此环境复验。

这不影响静态审计发现的逻辑缺陷；但依赖漏洞扫描、编译器诊断、SQLx prepared query 校验需要在有完整工具链和数据库的环境中补跑。

## 3. 仓库结构与交付物情况

### 主要源码规模

- `crates/server/src/lib.rs`：约 2.5k 非空行，是 P1 主要业务入口。
- `crates/storage/src/lib.rs`：约 1.4k 非空行，是 SQLx/PostgreSQL repository 实现。
- `crates/auth/src/lib.rs`：约 1k 非空行，是角色、可见性、session、invite 等领域模型。
- `apps/web/src/lib/backend.ts`：约 500 非空行，是前端 API client 与 session store。
- 多数 agent/RAG/rules crate 仍是轻量骨架。

### 交付包问题

原始 RAR 约 466 MB，而干净源码约 726 KB。交付包包含 `.git`、根目录 `node_modules`、`apps/web/node_modules`、生成文件等高噪音内容。建议后续交付只打源码包，排除：

```text
.git/
node_modules/
apps/web/node_modules/
target/
dist/
.next/
*.tsbuildinfo
.env
```

这不仅影响审计效率，也会带来历史 secret 泄漏、依赖污染和不可复现构建风险。

## 4. 严重问题清单

### P0-01：P1_AUDIT 声称的 RAG 修复与实际代码不一致

**证据**

- `docs/status/P1_AUDIT.md:30-49` 声称已扩展 `crates/rag_core` 为本地确定性 RAG kernel，并更新 `document_ingestor` 委托给 `rag_core`，还扩展了 `prompts/03_RULES_RAG.md`。
- 实际 `crates/rag_core/src/lib.rs:1-65` 只有 `DocumentType`、`RetrievalFilter`、`Evidence`、`Citation`、`VectorStore`、`KeywordIndex`。
- 实际 `crates/document_ingestor/src/lib.rs:4-41` 自己定义 `LicenseStatus` 与 `check_declared_license`，没有复用 `rag_core`。
- 原始快照中的 `prompts/03_RULES_RAG.md:1-3` 仍是极短 prompt。

**影响**

Codex 进入 P2 时会被文档误导，以为已有 `DocumentSource`、`LicenseStatus`、`Chunker`、`Embedder`、`LocalVectorStore`、`RagRepositoryTransaction` 等 API。结果很可能生成无法编译的代码或绕过真正需要设计的边界。

**建议**

- 把 `docs/status/P1_AUDIT.md` 改为“发现但未修复”，或实际补齐代码。
- 在 P2 之前统一 `rag_core` 类型边界，删除 `document_ingestor` 的重复 license enum。
- 扩写 `prompts/03_RULES_RAG.md`，明确 P2 的可编译 stop points。

### P0-02：P2 阶段定义冲突

**证据**

- `docs/PRODUCT_SYSTEM_DESIGN.md:925-933`：Phase 2 是文档导入、切块、embedding、混合检索。
- `docs/P2_DELIVERY_PLAN.md:1-17`：P2 是 Rules and RAG Kernel。
- 原始 `prompts/02_REALTIME_CONCURRENCY.md:1-3`：Realtime and Concurrency 被标为 Phase 2；该定义已废弃，当前为 Phase 3/P2B。
- 原始 `prompts/03_RULES_RAG.md:1-3`：RAG 被标为 Phase 3。

**影响**

在原始快照中，Codex 会同时看到两套 Phase 2 目标，可能先实现 WebSocket/Redis/outbox，也可能实现 RAG，导致任务漂移。当前阶段定义应以 P1.5/P2 文档和 active prompts 为准。

**建议**

- 统一阶段编号：Phase 2 = Rules/RAG/Ingestion，Realtime/Concurrency 移到 Phase 3 或 P2B。
- 重命名 prompt 或在 `CODEX_MASTER_PROMPT.md` 中声明当前唯一有效阶段。

### P0-03：idempotency 先写“完成响应”再写业务状态

**证据**

- `crates/server/src/lib.rs:1139-1186` 中 `create_room` 先调用 `claim_idempotent_response`，再 `create_room`。
- `crates/server/src/lib.rs:1160-1167` 已把 response 传入 idempotency claim。
- `crates/server/src/lib.rs:1172` 才写入房间。
- `crates/server/src/lib.rs:1658-1694` 的注释也承认当前应移动到事务内；实现创建 `status: Completed` 和 `response_json`。
- `crates/storage/src/lib.rs:871-930` 中 idempotency key insert/read 是独立 SQL 操作，没有和业务写入绑定。

**影响**

若 idempotency 记录写成功但业务写失败，客户端重试会拿到“成功创建”的响应，但数据库没有对应房间/邀请。审计日志也可能缺失或不一致。

**建议**

实现 repository 级事务接口，例如：

```rust
async fn create_room_idempotent(
    &self,
    actor: UserId,
    request_hash: String,
    idempotency_key: String,
    room: Room,
    audit: AuditLog,
) -> Result<IdempotentOutcome<RoomResponse>, RepositoryError>;
```

必须在同一 DB transaction 中完成：

1. 锁定或插入 idempotency key，状态为 `in_progress`。
2. 执行业务写入。
3. 写审计日志。
4. 更新 idempotency 为 `completed` 并写 response。
5. commit。

### P0-04：接受邀请的幂等重试会变成 404

**证据**

- `crates/server/src/lib.rs:1307-1310` 只查 pending invite。
- `crates/server/src/lib.rs:1311-1322` 找不到 pending invite 就直接 404。
- `crates/server/src/lib.rs:1348-1355` 只有在找到 pending invite 后才检查 idempotency。
- `crates/server/src/lib.rs:1360-1373` 首次成功后 invite 会被改为 accepted。

**影响**

相同 token + idempotency key 的重试，本应 replay 首次成功结果；但首次成功后 invite 不再 pending，第二次会在 idempotency 检查前返回 404。

**建议**

把接受邀请改成事务型 repository 操作：先按 `scope=invite:{token_hash}:accept` 和 idempotency key 查重；若已完成则直接 replay；若新请求，则锁定 invite 行，验证 pending/email/expiry，插入 room_member，更新 invite，写 audit，完成 idempotency。

### P0-05：生产缺少 DATABASE_URL 时退回内存存储

**证据**

- `crates/server/src/main.rs:3-10`：如果 `DATABASE_URL` 存在则连接 Postgres，否则 `server::router(config.clone())`。
- `crates/server/src/lib.rs:877-879`：`router(config)` 使用 `InMemoryAuthStore::default()`。

**影响**

生产环境环境变量漏配时，服务可能正常启动但所有用户/session/room/invite 都是内存状态，重启丢失，并绕过数据库 RLS 与审计持久化。

**建议**

- `TRPG_AUTH_MODE=production` 时必须要求 `DATABASE_URL`。
- 保留 in-memory store 只能用于测试或 `TRPG_AUTH_MODE=development && TRPG_ALLOW_IN_MEMORY_STORE=true`。
- `/readyz` 必须检查实际 repository 后端。

### P0-06：认证私有表的 RLS 策略与 repository 访问模型冲突

**证据**

- `migrations/20260626010000_phase_1a_identity_room.sql:132-142` 对 `auth_identities`、`magic_link_challenges`、`refresh_sessions`、`room_invites`、`idempotency_keys` 启用并 FORCE RLS。
- `migrations/20260626010000_phase_1a_identity_room.sql:149-152`：magic link 表 policy 是 `USING(false) WITH CHECK(false)`。
- `migrations/20260626010000_phase_1a_identity_room.sql:154-157`：refresh session 依赖 `app.current_user_id()`，但 refresh 时只能通过 token hash 查 session。
- `migrations/20260626010000_phase_1a_identity_room.sql:164-167`：idempotency 表 policy 是 `USING(false) WITH CHECK(false)`。

**影响**

如果 app 使用普通 DB role，repository 很可能无法插入/查询 magic link、idempotency key，也无法在已认证前通过 refresh token hash 查 session。若测试使用 postgres 超级用户或 BYPASSRLS，则会掩盖问题。

**建议**

做出明确设计选择：

- 方案 A：app 使用受控 `BYPASSRLS` role 访问认证私有表，同时所有 room/user 数据仍通过 RLS 上下文访问。
- 方案 B：保留 FORCE RLS，但所有私有表操作通过 `SECURITY DEFINER` 函数封装。
- 禁止生产使用 postgres 超级用户作为 `DATABASE_URL`。

### P0-07：RAG license RLS 不能完整阻断 pending/denied 检索

**证据**

- `migrations/20260626011000_phase_1a_rls_membership_fix.sql:114-127` 的 `documents_visibility_select` 没有检查 `license_status`。
- `migrations/20260626011000_phase_1a_rls_membership_fix.sql:143-156` 的 `chunks_visibility_select` 没有检查 `license_status`。
- `migrations/20260626020000_phase_2a_rag_ingest_contracts.sql:117-131` 对 `document_sources` 只有 public branch 强制 `license_status='allowed'`，room-scoped branch 没有 license 过滤。

**影响**

即使应用层未来做了 license 过滤，数据库层仍允许可见 room 下的 pending/denied 文档/切块被选出。对于规则书/模组，法律边界应该在数据库和检索层双重强制。

**建议**

- `documents`、`chunks`、`document_sources` 的普通 SELECT policy 都加入 `license_status='allowed'`。
- pending review 只开放给 KP/admin 审核接口，不能进入普通 retrieval path。
- 所有 retrieval SQL 必须显式 `WHERE license_status='allowed'`。

## 5. 中高优先级问题

### P1-01：refresh token 轮换存在并发竞态

**证据**

- `crates/server/src/lib.rs:1051-1077` 先查 session，再内存 rotate，再保存。
- `crates/storage/src/lib.rs:703-727` 按 `current_token_hash = $1 OR previous_token_hash = $1` 查询，没有 `FOR UPDATE`。
- `crates/storage/src/lib.rs:676-700` 保存是按 id 的 blind update，没有 CAS 条件。

**影响**

并发 refresh 请求可能同时接受同一个 current token 并各自发出新 refresh cookie，最后一个写入者覆盖数据库状态，另一个客户端拿到的 cookie 立即失效。

**建议**

在单事务中 `SELECT ... FOR UPDATE` 锁 session 行，或 `UPDATE ... WHERE id=$1 AND current_token_hash=$2 RETURNING ...` 做 CAS；复用检测也应在同一事务内完成。

### P1-02：认证 secret 与自定义 HMAC 实现需要加固

**证据**

- `crates/server/src/lib.rs:90-100` 只检查生产 `TRPG_AUTH_SECRET` 非空，没有长度/熵要求。
- `crates/server/src/lib.rs:1704-1752` 自定义 access token 格式。
- `crates/server/src/lib.rs:1745` 使用普通 `actual != expected` 比较签名。
- `crates/server/src/lib.rs:1767-1792` 手写 HMAC-SHA256。

**影响**

短 secret 会削弱 token 安全性；普通字节比较不是 constant-time；手写密码学逻辑增加维护风险。

**建议**

使用成熟 crate：`hmac` + `sha2` + `subtle`/`constant_time_eq`，或者使用标准 JWT/PASETO 库。生产 secret 至少要求 32 字节随机值，并支持 key id/rotation。

### P1-03：`readyz` 返回“ready”但依赖实际未检查

**证据**

- `crates/server/src/lib.rs:917-926`：database/redis/object_storage 都是 `not_checked_phase_1b_auth`，但整体 status 是 `ready_phase_1b_auth`。

**影响**

部署/监控系统会把依赖未验证的实例判定为 ready，流量可能打到未连数据库或未完成迁移的服务。

**建议**

- `healthz` 只代表进程活着。
- `readyz` 必须检查 DB ping、migration version、必要依赖状态；缺失则返回 503。

### P1-04：请求体限制、限流和安全中间件缺失

**证据**

- `crates/server/src/lib.rs:881-906` 直接构造 Router，未看到 body limit、timeout、CORS、trace、rate limit layer。
- Magic Link、OIDC dev callback、room/invite JSON 接口都暴露在同一 Router 下。

**影响**

可能受大 body、暴力请求、日志不可观测、跨域误配置影响。尤其 magic link 请求需要邮箱/IP 维度限流。

**建议**

加入默认请求体上限、超时、trace/correlation id、生产 CORS 白名单；对 magic link/login/refresh/invite 加 rate limit。

### P1-05：KP 权限模型与 API 实现不一致

**证据**

- `crates/auth/src/lib.rs:91-100`：`RoomRole::Kp` 允许 `InviteMember`。
- `crates/server/src/lib.rs:1224-1237`：创建邀请只允许 Owner。

**影响**

如果设计要求 KP 可邀请，则 API 错误拒绝；如果只允许 Owner，则 auth 权限矩阵过宽，会误导未来接口实现。

**建议**

在设计文档中明确：邀请成员是 Owner-only 还是 Owner/KP。然后同步 `RoomRole::can`、server handler、OpenAPI 和测试。

### P1-06：Email 校验过弱

**证据**

- `crates/auth/src/lib.rs:29-35`：只要包含 `@` 且无空白就算合法。

**影响**

可能接受 `@`、`a@`、`@b` 等无效地址，影响邀请、magic link、归属判断和审计质量。

**建议**

至少检查 local/domain 非空、domain 含点、总长限制；更好是使用成熟 email parser。

### P1-07：前端 access token 与 CSRF token 存在 sessionStorage

**证据**

- `apps/web/src/lib/backend.ts:314-347` 使用 `window.sessionStorage` 保存 session。

**影响**

Refresh token 在 HttpOnly cookie 中是好的，但 access token 和 CSRF token 对 XSS 可读。当前未发现 `dangerouslySetInnerHTML`，风险可控但不能忽视。

**建议**

P1 可接受，但 P2/P3 前应补 CSP、安全 header、依赖升级策略、XSS lint 规则；中长期考虑 BFF 或仅内存 access token。

## 6. 正向发现

- 未发现生产路径中的 `unsafe`。
- 未发现明显硬编码 API key / cloud provider secret。
- 生产 `TRPG_AUTH_MODE` 默认不是 development；开发魔法链接只在 development 下暴露，这是正确方向。
- Refresh token 设置为 HttpOnly cookie；CSRF 使用 double-submit cookie/header。
- 前端 `parseRoomDto` 会拒绝 KP-only 字段：`apps/web/src/lib/backend.ts:465-479`。
- `RoomDto` 暴露字段较小，P1 的 frontend projection 边界相对清晰。
- 大量 crate 采用 provider boundary/trait 形式，利于后续 Codex 分阶段实现。

## 7. 设计符合性矩阵

| 设计项 | 当前实现 | 结论 |
|---|---|---|
| Rust workspace 骨架 | crate 数量齐全，server/auth/storage/web 有实现 | 符合 P1 骨架 |
| Auth/Room REST | magic link、refresh、rooms、invite、members 已有 handler | 部分符合，存在 idempotency/refresh/RLS 风险 |
| 生产安全默认值 | auth 默认 production，但 DB 缺失退回内存 | 不符合 |
| ABAC/RLS | 有策略、有应用层检查 | 部分符合，认证私有表和 RAG license RLS 存在设计冲突 |
| Idempotency | 有 key/hash/replay 机制 | 部分符合，事务边界不正确 |
| OpenAPI | 静态 JSON + route contract | 部分符合，未来易漂移 |
| RAG P2 准备 | 文档存在，但代码与文档不一致 | 不符合 |
| 法律/版权边界 | 文档意识较强 | 部分符合，DB RLS 未完整强制 allowed license |
| 前端安全投影 | DTO parser 拒绝 KP-only fields | 基本符合 |
| 工程交付包 | 包含 node_modules/.git/生成物 | 不符合 |

## 8. P1.5 修复路线

### 必做 A：文档真相修正

1. 把 `docs/status/P1_AUDIT.md` 改成“发现 gap，尚未实现”。
2. 统一 Phase 2 = Rules/RAG/Ingestion。
3. 把 `prompts/02_REALTIME_CONCURRENCY.md` 改成 Phase 3 或 P2B。
4. 扩写 `prompts/03_RULES_RAG.md` 为真正可执行 prompt。

### 必做 B：生产启动安全

1. production 缺 `DATABASE_URL` 直接启动失败。
2. `TRPG_AUTH_SECRET` 至少 32 字节，并禁止默认/短 secret。
3. `TRPG_COOKIE_SAME_SITE` 只允许 Strict/Lax/None；None 必须 Secure。
4. `readyz` 使用真实依赖检查并按失败返回 503。

### 必做 C：事务一致性

1. `create_room`、`create_room_invitation`、`accept_room_invitation` 的 idempotency 与业务写入放入同一 repository transaction。
2. `accept_room_invitation` 支持首次成功后的同 key replay。
3. Refresh rotation 使用 DB row lock 或 CAS。

### 必做 D：RLS 与 license 边界

1. 明确认证私有表访问策略：BYPASSRLS app role 或 SECURITY DEFINER。
2. 禁止生产 `DATABASE_URL` 使用 postgres 超级用户。
3. 所有普通 RAG retrieval policy 和查询必须 `license_status='allowed'`。
4. pending/denied 只进入审核路径，不进入检索路径。

### 必做 E：交付包清理

1. 删除 `.git`、`node_modules`、`tsbuildinfo` 等生成物。
2. 增加 release packaging script，例如 `scripts/package_source.sh`。
3. 增加 `.gitattributes` 统一 LF，避免跨平台换行噪音。

## 9. P2 Codex 前置门禁

Codex 开始 P2 前应先跑过以下命令：

```bash
cargo fmt --all -- --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo test --workspace --all-features
cargo sqlx migrate run
cargo sqlx prepare --check --workspace
pnpm install --frozen-lockfile
pnpm lint
pnpm typecheck
pnpm test
pnpm test:e2e
pnpm build
```

并且需要新增/通过以下测试：

- production 缺 `DATABASE_URL` 启动失败。
- 短 `TRPG_AUTH_SECRET` 被拒绝。
- create room idempotency 业务写失败不会返回幽灵成功。
- accept invite 重试 replay 成功响应。
- 并发 refresh 只有一个新 refresh token 有效。
- 普通 retrieval 不返回 pending/denied license chunk。
- 普通 app DB role 能完成 auth/private table 流程，且不能绕过 room RLS。

## 10. 最终建议

当前快照应标记为：

```text
P1: 功能骨架完成，但不应直接进入 P2。
Gate: 需要 P1.5 修复安全/事务/RLS/文档一致性后，再启动 P2 RAG。
```

如果只让 Codex 按当前文档继续 P2，最大风险不是代码写得慢，而是它会基于错误的“RAG kernel 已存在”假设和冲突的 Phase 2 目标生成大量不可合并代码。
