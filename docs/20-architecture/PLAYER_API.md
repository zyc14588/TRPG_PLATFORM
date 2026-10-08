---
document_id: SPEC-PLATFORM-PLAYER-API-V1
schema_version: 1
authority: normative-specification
status: ACTIVE
source_commit: "31178c0f65b7cad711b73e8525cc823414b2c528"
---

# M2 玩家网关 API v1

本契约按 Owner 已批准的 CHANGE-M2-PLAYER-API-V1 采纳。

公开协议只使用同源 HTTPS、既有 HttpOnly Secure SameSite=Strict 会话 Cookie 和 CSRF 机制。
非 GET 必须携带既有 X-CSRF-Token、Idempotency-Key 和 application/json；每次请求校验租户、房间成员、当前席位和角色。
不接受 URL 中的令牌、查询参数、客户端指定的 principal/session/seat、Factory、权限过滤器、模型端点、密钥、Lua 或随机源。
工作区游戏目录只返回当前账户获准使用的已安装配置。访客只能读取已准入私人房间对应的配置。
沿用已批准的账户、邀请、访客交换和房间接口，不修改其请求、响应或含义。

## 范围与数据

完整的 method/path/request/response 固定列表见本文末尾；唯一 Schema 为 schemas/platform/platform-player-api-v1.schema.json。
configuration_id 仅选择已注册的可信服务端安装配置。游戏标题和内容/安全标签来自服务端记录。
管理者可以在 lobby 配置席位；自身包授权、ready、安全确认和边界由每个参与者单独提交。
准备状态仅返回公开席位布局、自己的同意/边界和脱敏的就绪原因，不能返回其他参与者边界、AI 凭据或隐藏角色。
任何准备修订都使旧同意失效；readiness 查询不能代替实际 Launch 的完整事务内复验。
访客不能创建长战役、管理工作区或获得管理者/主持人的旁观权限。管理员无席位不可读取游戏状态。

## 游戏与恢复

使用有界 HTTPS snapshot 轮询和短期服务端连接租约；不新增浏览器模型调用或 WebSocket 协议。
connection_id 仅标识当前 cookie/工作区/房间/席位下的连接，不单独构成授权。不得写入 URL、日志或持久化浏览器存储。
server 绑定 session_id、seat_id、principal，命令只携带 command_id、expected_state_version、type、payload、correlation_id。
通过原生平台 Session/Actor 提交；提交成功返回服务端结果，禁止浏览器维护权威状态或重发未知结果的新 command_id。
状态版本和事件游标使用十进制字符串，服务端严格限制到既有 signed64 存储范围；不改变原生 Envelope、事件或 checkpoint 格式。
Value 沿用现有 tagged checkpoint 数据表示，同时执行 checkpoint.Validate 的数字、深度、节点、cap: 和字节约束。
snapshot/result/export 都经过当前服务端席位策略过滤后构建明确 DTO；禁止直接 json.Marshal 私有 handle 或未经滤除的 StorageValue。
snapshot 请求/响应有界、分页最多128事件；导出 public/personal/host/administrator 沿用各权限，管理员不因管理身份获得他人私密信息。
只有现有主持权限可以创建恢复点；普通席位仅读取脱敏恢复点元数据。恢复以当前持久化绑定和游标为依据。

## 安全暂停与掉线

任一当前参局人可立即暂停，无需主持人批准；暂停只保存原因枚举和必要主体绑定，不记录私密边界文本。
暂停状态和控制修订必须持久化，在返回确认之前，与已经提交的 Actor 命令建立确定的顺序。
暂停后的新人工/AI mutation、任务派发及重试均被服务端拒绝或保留待处理；不得继续后台叙事/计费或换模型绕过边界。
短期连接租约过期或显式断开必要人工席位后，默认暂停涉及该席位的决策；重连只恢复视图，不能自动宣布继续或改成 AI。
恢复要求当前必要参与者明确确认，仍检查最新准备/控制修订和自身边界；主持人不能替其他人确认。
服务重启恢复持久化暂停状态；租约过期在任何 mutation 前复验，后台清理只是优化，不能成为安全前提。
玩家端显示暂停/掉线/准备拒绝/未知提交结果，执行重新取上下文和游标恢复，不以本地按钮状态代替服务端裁决。

## 资源与错误

JSON 请求最大16KiB，严格拒绝未知字段、重复 key、重复/异常 header、跨站请求和错误 Content-Type；Schema 本身最大64KiB。
响应最大256KiB、每次操作有超时、连接/席位/会话数量有限；拒绝超限并关闭自身连接，不输出任何原始 body/state/credential。
错误仅返回 Schema 内的安全枚举消息；SQL/模型/私有边界/客户端数据不能拼进普通错误和测试日志。
幂等重放复验当前成员、席位、准备版本、控制状态和过滤策略；暂停/过期后的重放不能创造新的 commit。
集成测试必须使用真实、独立、所有权明确的 PostgreSQL 和原生安装包/Actor；缺依赖必须 NOT_RUN，不得以 skip 或 mock 计入验收。

## 固定接口

| Method | Path | Request | Response |
| --- | --- | --- | --- |
| GET | /api/v1/workspaces/{workspace}/games | none | CatalogResponse |
| GET | /api/v1/workspaces/{workspace}/rooms/{room}/preparation | none | LobbyResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/preparation | ConfigureRequest | LobbyResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/consent | AcknowledgeRequest | AppliedResponse |
| GET | /api/v1/workspaces/{workspace}/rooms/{room}/readiness | none | LobbyResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/launch | LaunchRequest | LaunchResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/session/connect | ConnectRequest | ConnectionResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/session/snapshot | SnapshotRequest | SnapshotResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/session/commands | CommandRequest | CommandResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/session/disconnect | DisconnectRequest | ControlResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/session/pause | ControlRequest | ControlResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/session/resume | ControlRequest | ControlResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/session/export | ExportRequest | ExportResponse |
| POST | /api/v1/workspaces/{workspace}/rooms/{room}/session/recovery-point | VersionOnlyRequest | RecoveryPointResponse |
| GET | /api/v1/workspaces/{workspace}/rooms/{room}/session/recovery-point | none | RecoveryPointResponse |
