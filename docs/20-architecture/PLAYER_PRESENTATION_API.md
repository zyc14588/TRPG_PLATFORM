---
document_id: SPEC-PLATFORM-PLAYER-PRESENTATION-API-V1
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 玩家游戏说明公共契约 v1

仅新增 GET /api/v1/workspaces/{workspace}/games/{configuration}/presentation；请求无正文或查询参数，返回 PresentationResponse 或现有安全枚举 ErrorResponse。独立 Schema 为 schemas/platform/platform-player-presentation-api-v1.schema.json，固定8个定义：ID、Digest、Label、Package、ModelSelection、Presentation、PresentationResponse、ErrorResponse。本契约已获 Owner 批准；ACTIVE 登记确认契约版本，业务资格仍须通过对应批次验收。

展示已安装、可信配置的完整包清单和可供本配置使用的 AI 选择摘要。响应包含工作区/配置/游戏身份、当前权威配置摘要和图摘要、完整 packages、model_selections；字段精确定义见附带 Schema。包展示名称、版本、artifact/rights 摘要、许可标识和声明能力，来自当前已验证工厂/实际解析图和既有许可/授权资料；服务器验证映射一致性。不得采用浏览器提交的包清单、硬编码演示清单、未验证工厂或伪造资格替代实际安装。图不完整或超界应失败，不能截断清单供用户同意。

包条目的 `package_id` 原样采用现有原生包身份 `publisher-namespace/stable-name`：小写命名空间、单一 `/` 和稳定短名，最长255字符；精确语法沿用原生包 manifest v1。仅此字段使用包身份语法，不复用工作区/配置/游戏/selection/seat 的通用 ID，也不以别名、去斜杠、编码或摘要代替真实包身份。其余字段、8个定义名称、边界和权限保持。

模型只展示当前配置、工作区和调用者授权范围内的服务器注册 selection_id、说明 label、兼容 seat_ids、已有 capabilities 和当前 ready。模型名称别名仅由可信配置提供，并与已存在模型记录验证绑定；兼容性及准备状态来自原认证/授权/预算链，不能由浏览器或名称推导。缺失或不可用的列表/模型不能默认就绪或自动授予模型管理权限；访客不能据此配置长期 BYOK。响应不构成开局许可，原准备/启动再次核验当前模型、包图、同意与所有必要人工席位。

沿用现有 HTTPS、同源 Cookie、无 Authorization、no-store、no-referrer、拒绝未知/重复请求、当前账户/工作区关系及单局访客范围。访客仅可读取其已入场房间当前选定配置；无席位管理员不能读取游戏私密数据。每次读取复验身份、禁用状态、当前关系和配置作用域。先退出所属工作区/房间或撤销 Cookie 后仍需拒绝缓存重用。不得出现端点、密钥、Cookie、认证令牌、工厂/执行权限对象、Actor状态、其他席位边界、模型原始认证报告/文本或跨租户模型/包信息。

请求和响应沿用版本1统一信封；Schema闭合，ID/摘要/标签受限，包及模型列表各最多64，能力及席位列表最多64；最大响应256KiB、Schema64KiB、处理5秒以内。请求非法、配置失效、权限不足、映射不一致、资源界限或未知结果使用现有固定安全错误，不打印私密正文、底层 SQL、认证资料或原始响应。

此接口不写房间、席位、同意、模型配置、凭据或 Actor，不派发模型/任务，不形成资格或自动恢复。B010 用配置/图摘要与当前大厅匹配后展示真实包列表并由每人独立提交既有 consent；不匹配或读取失败保持未准备。选择AI采用授权列表中的selection_id，提交后由原服务器校验。既有 PLAYER_API.md、platform-player-api-v1.schema.json 的15路由/39定义、权限、暂停及幂等行为保持完整。
