# TRPG_PLATFORM P1.5 Stabilization Fix Plan

目标：在进入 P2 RAG 之前，修复 P1 交付中影响安全、事务一致性、RLS 可运行性和文档可信度的问题。

## Codex 总指令

你正在修复 TRPG_PLATFORM 的 P1.5 稳定化问题。不要实现完整 P2 RAG；只修正 P1 的阻塞缺陷，并把设计文档统一到可执行状态。

必须遵守：

- 不引入真实 LLM/OIDC/SMTP/付费云服务调用。
- 不加入任何未授权商业规则正文。
- 保持 Rust workspace 可编译。
- 每个修复必须附测试。
- 不编辑 Phase 0 初始 migration；新增 additive migration。
- 不绕过 ABAC/RLS 的安全意图。

## 任务 1：修正文档真相与阶段编号

### 修改

- `docs/status/P1_AUDIT.md`
  - 删除或改写 “Fixes Applied in This Pass”。
  - 明确 `rag_core` 当前仍是 skeleton。
  - 标注 P2 不应开始，直到 P1.5 gate 通过。
- `docs/PRODUCT_SYSTEM_DESIGN.md`
  - 保持 Phase 2 = Rules/RAG/Ingestion。
- `prompts/02_REALTIME_CONCURRENCY.md`
  - 改名或改标题为 Phase 3 / P2B，不再与 RAG P2 冲突。
- `prompts/03_RULES_RAG.md`
  - 改为 “Codex Phase 2 — Rules and RAG”。

### 验收

- 搜索 `Codex Phase 2` 只出现一个主线定义：Rules/RAG/Ingestion。
- P1_AUDIT 不再声称已实现不存在的 RAG kernel 类型。

## 任务 2：生产启动安全

### 修改

- `crates/server/src/main.rs`
  - 当 `TRPG_AUTH_MODE=production` 时，缺 `DATABASE_URL` 必须返回错误并退出。
  - InMemoryAuthStore 只允许 development/test 明确开启。
- `crates/server/src/lib.rs`
  - `TRPG_AUTH_SECRET` 生产最小长度 32 字节。
  - 拒绝 `development-secret-do-not-use` 在 production 使用。
  - `TRPG_COOKIE_SAME_SITE` 只接受 `Strict`、`Lax`、`None`；`None` 必须 `Secure`。
  - 解析 bool 使用严格 parser，非法值报错。

### 测试

- production_without_database_url_fails
- production_rejects_short_auth_secret
- production_rejects_insecure_samesite_none
- development_can_use_in_memory_only_when_explicit

## 任务 3：idempotency 事务化

### 修改

实现 repository 级事务 API，至少覆盖：

- create room
- create room invitation
- accept room invitation

推荐抽象：

```rust
pub enum IdempotentOutcome<T> {
    Created(T),
    Replayed(T),
    Conflict,
}

#[async_trait]
pub trait RoomCommandRepository {
    async fn create_room_idempotent(...)
        -> Result<IdempotentOutcome<RoomResponse>, RepositoryError>;
    async fn create_room_invitation_idempotent(...)
        -> Result<IdempotentOutcome<CreateInvitationResponse>, RepositoryError>;
    async fn accept_room_invitation_idempotent(...)
        -> Result<IdempotentOutcome<RoomResponse>, RepositoryError>;
}
```

事务顺序：

1. 删除过期 idempotency row。
2. Insert/lock idempotency row。
3. 如果 duplicate 且 hash 相同，返回 stored response。
4. 如果 duplicate 且 hash 不同，返回 conflict。
5. 执行业务写入与审计。
6. 更新 idempotency row 为 completed + response_json。
7. commit。

### 测试

- create_room_idempotency_does_not_replay_failed_write
- create_room_duplicate_replays_same_response
- create_room_duplicate_with_different_hash_conflicts
- accept_invite_duplicate_replays_after_invite_is_accepted
- accept_invite_duplicate_different_hash_conflicts

## 任务 4：refresh rotation 原子化

### 修改

在 PostgreSQL repository 中实现单事务 refresh rotate：

```sql
SELECT ... FROM refresh_sessions
WHERE current_token_hash = $1 OR previous_token_hash = $1
FOR UPDATE;
```

或：

```sql
UPDATE refresh_sessions
SET current_token_hash = $next,
    previous_token_hash = $presented,
    rotated_at = now(),
    updated_at = now()
WHERE id = $id
  AND current_token_hash = $presented
  AND status = 'active'
RETURNING ...;
```

复用检测也必须在同一事务里写入 revoked 状态。

### 测试

- concurrent_refresh_only_one_rotation_wins
- refresh_reuse_revokes_session_family
- stale_refresh_cookie_after_race_is_rejected
- previous_refresh_token_reuse_policy_is_explicit

### Batch 5 复验缺口

当前实现应使用 PostgreSQL repository 的单事务 `SELECT ... FOR UPDATE` 路径完成 refresh rotation，而不是 HTTP handler 中的 `find -> memory rotate -> blind save`。复验时必须检查 `crates/storage/src/lib.rs` 中 `PostgresRepositories::rotate_refresh_session` 是否仍满足：

- `pool.begin()` 开启事务。
- 按 `current_token_hash = presented OR previous_token_hash = presented` 查询同一 session 行。
- 查询带 `FOR UPDATE`，或改为等价 CAS `UPDATE ... WHERE current_token_hash = presented AND status='active' RETURNING ...`。
- 成功路径更新 `current_token_hash`、`previous_token_hash`、`rotated_at`、`updated_at`。
- previous token reuse 检测和 `session_family_id` revoke 在同一事务内提交。

测试缺口：`crates/storage/src/lib.rs` 中的并发与复用测试调用的是 `PostgresRepositories`，不是 `InMemoryAuthStore`；但这些测试通过 `migrated_repo()` 依赖 `DATABASE_URL`。如果测试环境没有设置 `DATABASE_URL` 或迁移失败，测试会提前 `return Ok(())`，因此只证明编译通过，不证明真实 PostgreSQL 行锁行为。CI/验收必须提供 PostgreSQL `DATABASE_URL`，并确认这些测试实际连接数据库运行。

## 任务 5：RLS 访问模型修正

### 修改

选择一个方案并写进 `docs/SECURITY_RLS_POLICY.md`：

#### 方案 A：受控 app_private role

- 为 magic link、refresh session、idempotency 等认证私有表使用受控 app role。
- 该 role 可访问私有 auth tables，但不使用 postgres 超级用户。
- room/user/document 业务表仍必须通过 RLS 上下文。

#### 方案 B：SECURITY DEFINER functions

- 所有 magic link、refresh、idempotency 私有表访问通过函数。
- 普通 app role 无直接 table 权限。

无论选哪种，禁止生产 `DATABASE_URL` 指向 postgres 超级用户。

### 测试

- app_role_can_complete_magic_link_flow
- app_role_can_rotate_refresh_session
- app_role_can_claim_idempotency_inside_allowed_function_or_role
- app_role_cannot_select_cross_room_documents
- app_role_cannot_select_kp_only_as_pl

## 任务 6：RAG license RLS 防线

### 修改

新增 migration，更新普通 SELECT policy：

- `document_sources`: 普通 retrieval path 只能 SELECT `license_status='allowed'`。
- `documents`: 普通 retrieval path 只能 SELECT `license_status='allowed'`。
- `chunks`: 普通 retrieval path 只能 SELECT `license_status='allowed'`。

pending/denied 审核使用单独 KP/admin review policy 或专用 endpoint，不得混在 retrieval policy 中。

### 测试

- pl_retrieval_cannot_select_pending_or_denied_chunks
- kp_retrieval_cannot_select_denied_chunks
- kp_review_can_list_pending_sources_only_through_review_path
- public_rule_requires_allowed_license

## 任务 7：工程交付清理

### 修改

- 新增 `scripts/package_source.sh`。
- 新增 `.gitattributes`：`* text=auto eol=lf`，对 lockfile/SQL/RS/TS/MD 固定 LF。
- `.gitignore` 加入：`node_modules/`, `target/`, `.next/`, `dist/`, `*.tsbuildinfo`, `.env`。

### 验收

- 源码包小于 5 MB。
- 解压后不包含 `.git`、`node_modules`、`target`、`.next`。

## P1.5 完成定义

P1.5 只有在以下命令通过后才算完成：

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
