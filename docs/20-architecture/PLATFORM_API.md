---
document_id: SPEC-PLATFORM-AUTH-API-V1
schema_version: 1
document_kind: normative-specification
authority: normative-specification
status: ACTIVE
source_commit: "0bd10100a13066c9e12503b3f8d3fb942d572e80"
---

# 平台认证与工作区接口 v1

Owner 已批准 CHANGE-M2-AUTH-API-V1。本规范与 `schemas/platform/platform-auth-api-v1.schema.json`（SCHEMA-PLATFORM-AUTH-API-V1）共同约束首次公开的认证协议。接口字段、枚举、长度、未知字段拒绝及响应结构以该可执行 Schema 为准；此规范约束身份、授权、事务和安全行为。B002 在 Linux 上实施，后续新公开字段仍须具体 CHANGE 批准。

采用自托管本机账户和受邀单局访客，不依赖中央账户或第三方登录。注册默认关闭；首个账户及一次性注册授权由部署者提供只读秘密文件。合法访客可以在认领时创建本机账户，或验证既有本机账户。认领保留原参与标识和原工作区、房间、游戏范围，不能自动增加成员管理权、席位或私密视图访问权。

## HTTP 和身份边界

`/api/v1` 接受 UTF-8 JSON，写请求必须有 `schema_version: 1`。拒绝未知、重复字段、不支持的版本、超过16384字节的正文和多余的 JSON 内容。请求不能声明登录身份、租户、角色或席位；服务器只使用权威记录中验证的身份和范围。受控的成员权限操作仅允许 Schema 列出的 admin/member，并继续使用核心 owner/admin 策略。

每次状态变更必须同时提供与配置 HTTPS origin 精确匹配的 `Origin`、`X-CSRF-Token` 和 `Idempotency-Key`；幂等键为16至128字符的 ASCII 标识。只读请求除发放 CSRF 上下文和会话活跃时间记账外不改变业务状态。不启用通配 CORS 或跨源凭据访问。

| Method | Path | Request Schema | Response Schema | Authorization |
| --- | --- | --- | --- | --- |
| GET | `/api/v1/auth/context` | — | ContextResponse | same-origin anonymous or authenticated; no credential in body |
| POST | `/api/v1/auth/register` | RegisterRequest | AccountContextResponse | anonymous plus single-use operator registration grant; account creation and grant consumption atomic |
| POST | `/api/v1/auth/login` | LoginRequest | AccountContextResponse | local credentials verified; rotate session; a current guest must use claim |
| POST | `/api/v1/auth/logout` | LogoutRequest | ContextResponse | revoke current session and clear cookie; fresh anonymous CSRF context |
| POST | `/api/v1/auth/guest/exchange` | GuestExchangeRequest | GuestContextResponse | trusted room-admission verifier required; reject when absent; no client-provided tenant/room/game/role |
| POST | `/api/v1/auth/guest/claim` | GuestClaimRequest | ClaimResponse | valid guest cookie plus verified existing or newly created account; preserve exact participation; no membership grant |
| POST | `/api/v1/workspaces` | CreateWorkspaceRequest | WorkspaceResponse | account only; server-generated workspace identity; atomic owner relation |
| GET | `/api/v1/workspaces/{workspace_id}` | — | WorkspaceResponse | current authoritative workspace membership required |
| PUT | `/api/v1/workspaces/{workspace_id}/members/{account_id}` | SetMembershipRequest | MembershipResponse | owner/admin policy from accepted core; only owner may grant admin; cannot modify owner |
| DELETE | `/api/v1/workspaces/{workspace_id}/members/{account_id}` | RemoveMembershipRequest | RemovalResponse | accepted core policy; cannot remove owner |
| GET | `/healthz` | — | HealthResponse | only fixed readiness status; no diagnostics or service configuration |

访客凭证兑换仅接受不透明的 admission_token。它必须交给可信房间授权验证器校验；尚无该验证器时拒绝兑换，不能把客户端声明或未验证凭证当作房间授权。房间邀请的业务实现由后续批次负责。

## Cookie、会话与 CSRF

会话只通过 `__Host-trpg_session` Cookie 传送，随机令牌32字节；属性固定为 Secure、HttpOnly、SameSite=Strict、Path=/，没有 Domain。令牌不能放入 URL、JSON session_token、浏览器 Web Storage 或任意 bearer header。全部认证请求使用 HTTPS。

默认绝对期限为28800秒（8小时），闲置1800秒（30分钟）到期；访客还受到原访客期限限制。会话验证、退出、轮换、账户禁用、访客撤销与认领以当前权威记录判定。移除工作区成员撤销该工作区访问，不修改其无关的账户身份。

每次写操作均使用同源校验和绑定当前上下文的 HMAC CSRF 令牌，包含登录、退出和认领。匿名预认证 Cookie 同样使用 Secure、HttpOnly、__Host 前缀，并在300秒（5分钟）内到期。成功登录或认领轮换会话；当前访客须通过认领登录，不能绕过参与身份保留。退出撤销当前会话、清除会话 Cookie，并产生新的匿名 CSRF 上下文。

## 凭据与有界资源

新密码必须是15至128个 Unicode 码点，按原 UTF-8 字节验证，不裁剪或归一化，不规定任意字符组合。Argon2id 固定最低参数为65536KiB内存、3轮、并行度1、16字节随机盐、32字节派生键。使用已固定的 golang.org/x/crypto v0.51.0，不新增第三方依赖或升级版本。

散列并发和登录、注册、认领速率必须有界，拒绝形态保持一致。实际 Linux 目标测量可以调整有界资源配置，但不能降低上述最低散列参数。未知账户、禁用账户和错误密码返回同一安全错误形态，避免通过响应区分账户状态。

数据库保存密码散列和主会话令牌散列。若短期幂等重放需要恢复 Cookie，相关材料必须使用独立只读秘密文件中的密钥进行认证加密。普通日志、诊断、错误或导出不能包含原密码、原 Cookie、数据库连接信息或参与者私密内容。

## 授权、原子性与幂等

账户注册和一次性授权消耗必须原子提交。访客认领中的账户创建或验证、原参与身份认领、会话轮换及持久回执必须作为同一事务提交。任一阶段失败均不能留下孤立凭据、越权成员关系、无来源的会话或部分认领。工作区创建与唯一 owner 关系继续原子提交。

每项幂等结果绑定已验证身份、接口及规范化请求；同一个键改变字段必须拒绝。重试仍检查当前授权，不能重放旧结果恢复已撤销的权限。提交结果未知时返回 OUTCOME_UNKNOWN，不能将未知提交当作确认回滚后盲目执行。数据库故障、取消原因或外部服务错误不能直接包装成公开错误。

账户服务不把工作区管理权当作房间/游戏管理权；核心成员权、单局参与、席位与私密视图继续分别授权。访客不得持久拥有工作区、上传包或保留长期凭据。

## 错误和就绪状态

| Code | HTTP status |
| --- | --- |
| INVALID_REQUEST | 400 |
| UNAUTHENTICATED | 401 |
| DENIED | 403 |
| CONFLICT | 409 |
| CLAIM_REQUIRED | 409 |
| RATE_LIMITED | 429 |
| UNAVAILABLE | 503 |
| OUTCOME_UNKNOWN | 503 |

错误只包含 Schema 规定的固定类别与安全消息。`/healthz` 只报告固定就绪状态，不返回内部诊断、服务配置或连接详情。B002 的认证协议不替代后续启动就绪门禁。

## 资格与验收

要求可执行请求/响应 Schema 与兼容验证；真实的本任务独占 PostgreSQL 验证账户、凭据、会话、认领、轮换、过期、撤权、幂等和未知提交恢复的原子性；HTTPS、Origin、CSRF、Cookie、资源限额及日志/格式化/导出的私密字段拒绝验证；干净签名候选及独立业务、状态验收。未执行的端到端或其他平台验证不得标为通过。

不扩展房间邀请、AI、中央账户、OAuth、公共房间发现、自动管理权/席位/私密视图授权、Host API、事件/包格式、固定 Lua 后端、许可或其他平台。M1 归档与 B001 的已接受冻结契约保持原字节。

安全设计参考：[OWASP Password Storage](https://cheatsheetseries.owasp.org/cheatsheets/Password_Storage_Cheat_Sheet.html)、[Session Management](https://cheatsheetseries.owasp.org/cheatsheets/Session_Management_Cheat_Sheet.html)、[CSRF Prevention](https://cheatsheetseries.owasp.org/cheatsheets/Cross-Site_Request_Forgery_Prevention_Cheat_Sheet.html)。本规范的明确参数与行为由该已批准提案约束。
