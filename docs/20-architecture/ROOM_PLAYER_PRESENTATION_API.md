---
document_id: SPEC-PLATFORM-ROOM-PLAYER-PRESENTATION-API-V1
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 当前房间玩家游戏说明公共契约 v1

GET /api/v1/workspaces/{workspace}/rooms/{room}/presentation 返回指定房间当前选定游戏的只读说明。请求不接受正文或查询参数；工作区与房间 ID 使用现有通用 ID 语法。浏览器不能另行提交 configuration、game、包图或授权关系。

响应原样复用 `schemas/platform/platform-player-presentation-api-v1.schema.json` 的版本1统一信封与八个定义：ID、Digest、Label、Package、ModelSelection、Presentation、PresentationResponse、ErrorResponse。成功返回 PresentationResponse，失败使用现有固定安全错误；不添加定义、字段或错误枚举。房间作用域由请求路径和本次服务端授权读取确定，响应不新增 room_id。原 GET /api/v1/workspaces/{workspace}/games/{configuration}/presentation、工作区目录以及现有房间和玩家契约的行为保持。

## 当前身份与房间准入

每次读取都以当前同源 HTTPS Cookie 身份重新授权，沿用现有房间及准备信息读取的合法参与者关系，或该读取已允许的管理者关系。合法参与者包括已认领账号、跨工作区受邀的非成员账号及有效单局访客；加入资格本身不能代替已成立的房间参与关系。调用者不能通过本接口获得工作区成员资格、席位、管理权、模型使用权或长期 BYOK 权限。

移除工作区成员资格不会自动撤销仍有效的独立房间参与关系：只要参与者仍满足现有当前房间读取条件，本接口仍可读取该房间。若所有当前房间读取依据均失效，或账号禁用、Cookie 撤销、访客过期/撤销、租户或房间作用域不匹配，则拒绝。原配置说明接口仍要求其原有成员或有效单局访客依据；账号移除工作区成员资格后，即使保留独立房间参与关系，原配置接口也继续拒绝。管理关系失效且无其他有效参与依据时，新接口拒绝。

## 单次授权读取与可信来源

在同一当前授权读取事务中验证房间及准备信息，取得该房间当前选定的 configuration、game、权威配置摘要与已验证安装包图，并核验这些身份和摘要的一致性。不得先相信浏览器的配置标识、复用其他房间的授权结果，或拆成可混合不同权限/配置时点的独立授权读取。房间配置、准备状态或已安装图不匹配时拒绝，不回退到另一配置或缓存图。

packages 必须完整来自当前已验证工厂的实际解析图及既有许可/授权资料，包含原生 package_id、版本、标题、artifact/rights 摘要、许可和声明能力。package_id 保留 `publisher-namespace/stable-name` 原生语法，不采用通用 ID 别名。禁止硬编码演示包、浏览器包清单、未验证工厂或不完整清单；说明读取不实例化权威 Session。

## 本房间的模型说明

model_selections 仅来自本次已授权 workspace、room、game、configuration 对应的服务器模型记录。存储读取必须先按这些完整维度限定房间，再应用现有调用者/席位可见权限、当前配置与图/修订绑定、认证、凭据、预算及 ready 检查。可信 label 只注释已验证 selection_id；缺失、撤销、过期、不兼容或不可见模型不能默认就绪。

存储查询先限定本房间再取最多65条用于识别64条上限；不得先从同配置的多房间记录取65条再在内存过滤房间。其他房间的记录不能占用本房间的上限、使合法记录遗漏或造成错误超界。本房间候选记录超界时使用现有安全失败，不截断成可供确认的成功列表。两个房间即使选择同一配置或具有相同 selection_id，也分别计算各自的 seat_ids、capabilities 与 ready；禁止跨房间合并或借用模型资格。

## 请求、响应与副作用边界

沿用现有 HTTPS、同源 Cookie、拒绝 Authorization、未知或重复请求形式、no-store、no-referrer 及每次读取重新授权。Schema 闭合，ID、摘要和文本沿用已有格式；每个包、模型、能力及席位数组最多64项，标签最多80字符，Schema 最多64KiB、响应最多256KiB，处理5秒以内。非法请求、映射不一致、超界、取消或未知结果均使用现有固定安全错误，不泄露私密正文、底层 SQL 或原始认证响应。

响应不得包含端点、密钥、Cookie、令牌、工厂或执行权限对象、Actor 状态、其他席位私密边界、原始认证报告或跨租户包/模型信息。无席位管理者的房间说明读取不授予游戏私密数据读取权。

本接口不写房间、成员、席位、同意、模型、凭据或 Actor，不派发模型或任务，不改变数据库写入模式，不构成开局许可或自动恢复。玩家确认仍通过既有独立 consent；准备和启动仍按当前配置、包图、同意、模型与必要席位重新执行既有门禁。说明读取失败或摘要与当前房间不匹配时不能据此标记准备完成。
