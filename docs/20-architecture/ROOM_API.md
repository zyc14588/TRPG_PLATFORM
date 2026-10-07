---
document_id: SPEC-PLATFORM-ROOM-API-V1
schema_version: 1
document_kind: public-contract
authority: normative-spec
status: ACTIVE
language: zh-Hans
---

# 私人房间与入场 API v1

Owner 已批准 CHANGE-M2-ROOM-API-V1 及 CHANGE-M2-ROOM-ACCOUNT-NICKNAME-V1；本契约已登记为 ACTIVE。后者仅更正账户参与者和账户申请响应的昵称上限为128，访客昵称及房间名称仍为80；其余公共字段、端点及授权语义保持已批准契约。

## 1. 传输与身份

沿用已批准的 PLATFORM_API.md：HTTPS、固定 Origin、__Host Cookie、CSRF、16 KiB 正文上限、5秒正文读取时限、无宽松 CORS、无任意 Bearer、禁止查询字符串。JSON 是严格 UTF-8，拒绝重复键、未知字段、超深嵌套、尾随内容和非1版本。Schema验证通过后，将schema_version、expires_in_seconds和max_uses规范化为整数；等价的1、1.0和1e0不能造成幂等摘要差异。所有新写操作也要求同源 Origin、X-CSRF-Token 和16..128字符 Idempotency-Key。GET仅有会话活跃记账；没有公共房间列表、搜索或旁观入口。

响应仍是 schema_version/request_id/data 或现有错误包络；错误码和HTTP状态原样复用。状态变化与加密幂等回执在同一显式 SQL 事务提交，回执逻辑期限最多5分钟。未知提交明确返回 OUTCOME_UNKNOWN，按原 Cookie/端点/正文/Key 查结果；不能盲目重建。每次重试重新查当前身份、角色、目标、邀请与参与关系，不能复活已撤销邀请、消耗过的入场令牌或退出的参与者。

## 2. 私人房间、角色与参与者

只有当前工作区 Owner/Administrator 账户可创建房间，创建者成为不可变房间所有者。工作区权限与房间角色分别校验；工作区管理者不自动成为已有房间管理员或参与者。房间绑定创建时选择的不可变 game_id；创建只保存配置，不执行包或开启 Session，真实包/内容/席位/模型检查由 B004 负责。房间 ID 和参与者 ID 由服务器产生。

房间所有者、显式管理员、主持角色和席位控制者是四个独立授权维度。管理员必须是当前工作区账户且由所有者显式授予；访客不能获得管理权。主持角色只能授予已入场参与者，可以是真人账户或本房间有效访客；主持角色本身不授予大厅管理或任何私密游戏视图。席位分配与席位控制绑定由后续启动/Session 批次提供，B003没有席位授予端点，邀请和审批也不选择席位。

房间读取只含名称、公开参与者标识/昵称/类型/角色及当前调用者房间角色；不含私密边界、席位状态、提示、Cookie、令牌或未授权工作区资料。非参与的房间管理者可办理大厅事务，不能据此读取游戏私密数据。无显式席位控制绑定时私密读取守卫必须拒绝。最多64名入场参与者及64条待处理申请，满额返回CONFLICT；游戏包可以在B004声明更小席位规模。

关闭、退出、移除和角色更改只处理未启动大厅；已启动后的 Session 操作在后续批次通过权威命令实现。所有者不能退出/被移除/被降级，只能关闭未启动大厅。房间关闭一次性撤销邀请、申请与相关大厅参与授权，不开启/终止 Session。launched状态由后续可信启动服务设置，本批不能自行设为launched。

## 3. 邀请与审批

创建邀请明确指定 approval_required、expires_in_seconds=60..604800 和 max_uses=1..64。链接令牌使用32字节密码随机数的无填充Base64URL（43字符）；房间代码使用80位随机数的16字符Crockford Base32，严格大写，不作模糊字符替换。数据库只保存在独立32字节只读邀请密钥下的HMAC；邀请秘密不得进入普通日志、诊断、导出或URL查询。分享链接把令牌放浏览器fragment，再通过JSON正文提交，反向代理不接收fragment。

链接和代码是同一邀请的两种入口，只授予申请入场资格。未过期、未撤销和剩余配额都从同一权威邀请行查询并锁定；并发审批和加入不能超额。已入場/批准的同一主体重试不重复消费。待处理申请不预占使用配额，但总数受64上限和速率限制；拒绝后不能靠换Key复活原申请。撤销邀请拒绝未使用资格和相关未兑换访客申请；已经加入的参与者须经单独退出/移除撤权。

account模式从有效账户Cookie确定账户，display_name取服务器账户记录，沿用已验收认证契约的1..128字符上限；账户参与者及账户申请响应也采用此上限。房间名称及guest昵称仍为1..80字符，guest请求、参与者和申请响应均保持80字符限制；访客认领后投影账户昵称采用账户128字符上限。guest模式只接受有效匿名Cookie且要求昵称。匿名申请与原匿名会话绑定，最多维持至该Cookie的5分钟期限，过期须重新申请；有效访客Cookie不能再申请另一房间。跨工作区账户仅获本房间参与关系，不获得工作区成员关系或资产权限。

免审批的账户加入立即在事务内消耗配额并创建参与者；需要审批时先记录pending。审批账户同样在一个事务内创建参与者并扣配额。访客获批后由原匿名会话申请一次性admission_token，期限为5分钟、原匿名会话及邀请期限的最小值；令牌只通往现有 /api/v1/auth/guest/exchange，不能用于其他API。可信入场验证器在现有认证同一SQL事务验证并消耗该令牌、创建单局guest/参与者、扣配额和旋转Cookie，失败全部回滚。验证器缺席或请求跨scope仍拒绝。

本批不改变已批准 guest claim 的字段或密码/会话规则。认领后用同一guest/participant标识及相同workspace/room/game关联投影账户身份，保留参与关系和已显式授予的非管理角色；不增加工作区成员、管理权或席位。遇到已有相同参与关系的冲突必须原子拒绝，不能产生双身份或半完成Cookie。

## 4. 端点与 Schema

路径中的标识符均须符合已批准Identifier格式并与当前权威scope一致；客户端不能提供角色、所有者或租户身份来取得权限。正文和响应仅使用同文件platform-room-api-v1.schema.json中的固定定义，不解析外部Schema引用。

| 方法 | 路径 | 请求定义 | 响应定义 | 当前授权 |
|---|---|---|---|---|
| POST | `/api/v1/workspaces/{workspace_id}/rooms` | CreateRoomRequest | RoomResponse | 当前账户及工作区房间创建权限；服务端生成房间 ID、不可变所有者。 |
| GET | `/api/v1/workspaces/{workspace_id}/rooms/{room_id}` | 无 | RoomResponse | 当前房间所有者、显式管理员或已入场参与者；只返回大厅资料。 |
| DELETE | `/api/v1/workspaces/{workspace_id}/rooms/{room_id}` | VersionOnlyRequest | AppliedResponse | 仅所有者；只关闭未启动大厅，不能通过房间事务终止 Session。 |
| POST | `/api/v1/workspaces/{workspace_id}/rooms/{room_id}/invitations` | CreateInvitationRequest | InvitationResponse | 当前所有者或显式房间管理员；秘密只在创建/仍获授权的幂等重试返回。 |
| DELETE | `/api/v1/workspaces/{workspace_id}/rooms/{room_id}/invitations/{invitation_id}` | VersionOnlyRequest | AppliedResponse | 当前所有者或显式房间管理员；撤销未使用入场资格及相关待处理申请。 |
| POST | `/api/v1/room-admissions` | AdmissionRequest | AdmissionResponse | 账户模式须有效账户 Cookie；访客模式须有效匿名 Cookie；已有访客须先认领，不能跨房间。 |
| GET | `/api/v1/room-admissions/{admission_id}` | 无 | AdmissionResponse | 仅申请原账户或原匿名会话；不接受客户端主体/租户断言。 |
| POST | `/api/v1/room-admissions/{admission_id}/guest-token` | VersionOnlyRequest | GuestTokenResponse | 原匿名会话、已批准且未过期/撤销的访客申请；一次性令牌最多5分钟。 |
| GET | `/api/v1/workspaces/{workspace_id}/rooms/{room_id}/admissions` | 无 | AdmissionQueueResponse | 当前所有者或显式房间管理员；至多64条待处理申请，不包含任何入场令牌。 |
| POST | `/api/v1/workspaces/{workspace_id}/rooms/{room_id}/admissions/{admission_id}/decision` | DecisionRequest | AdmissionResponse | 当前所有者或显式房间管理员；原子审批，邀请/配额/申请/主体仍有效。 |
| POST | `/api/v1/workspaces/{workspace_id}/rooms/{room_id}/leave` | VersionOnlyRequest | AppliedResponse | 当前参与者仅退出自身；所有者关闭大厅；已启动游戏的离开由后续 Session 命令处理。 |
| DELETE | `/api/v1/workspaces/{workspace_id}/rooms/{room_id}/participants/{participant_id}` | VersionOnlyRequest | AppliedResponse | 当前所有者或显式房间管理员；任何人不能移除所有者，管理员不能移除其他管理员；本批仅大厅。 |
| PUT | `/api/v1/workspaces/{workspace_id}/rooms/{room_id}/participants/{participant_id}/role` | SetRoomRoleRequest | AppliedResponse | 只有所有者可授予/撤销管理员；管理员仅限当前工作区账户；所有者/管理员可显式授予主持角色，不能改所有者身份或授予席位。 |

## 5. 存储、组合及验收

使用显式关系表和默认带workspace/room/game的SQL查询；不以JSONB替代身份/角色/配额关系。仅在本任务独占真实PostgreSQL验证，缺fixture为NOT_RUN且非零退出。邀请密钥与Cookie/回执密钥不同，只读文件加载；统一限流至每匿名网络/动作或验证身份每分钟10次、每服务每分钟60次，键和缓存有固定上限，不能记录原Cookie、IP、代码或令牌。无网络等待或模型调用进入权威事务。

沿用B001/B002源文件和已冻结契约。认证包新增room*.go可信组合扩展，共用当前会话、CSRF及加密回执检查；postgres包新增platform_room*.go同事务桥接，不导出原始数据库或凭据到HTTP层。httpapi新增room*.go，组合函数新增cmd/platformd/platform_room*.go；现有API路径行为不改。真实进程启动与Linux Compose仍是B004/B011工作。

验收须覆盖邀请码随机性/Hash/秘密脱敏、过期/撤销/限额、账户/访客审批、Cookie绑定、并发和回滚、未知提交跨重启恢复、撤权后旧回执拒绝、认领保留身份且无管理权、同名跨租户对象隔离、Owner/管理员/主持/席位负向矩阵，以及真实TLS的继承传输守卫。必需命令包含fresh单元/契约、ownedSQL、竞态、全仓check/test/vet和Linux CI，并进行独立验收与证据封存。M2最终13项需求仍由B012完整闭环证明，B003基础通过不能替代出口。
