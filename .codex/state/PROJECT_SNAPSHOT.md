---
document_id: CODEX-PROJECT-SNAPSHOT
schema_version: 1
document_kind: state-summary
authority: state-summary
status: ACTIVE
source_commit: "6df9760d3b673891a9a9c1904ca36011757214fe"
---

# 项目摘要

## 当前 Linux M2：B014 玩家游戏说明上游批次已规划

正常 PLAN v47→v48 按 next14 分配 M2-B014（sequence14，PLANNED），next15。仅增加已批准只读游戏说明上游；十个已完成批次、B012、墓碑和 M1 保持。B010 仍为 IMPLEMENTING，全部字段及冻结摘要 bac1060f0aac5ca9b4142c94a6ba4683748c87fa027fc239980afe5c62b35ac6 原样保留，其业务执行停止等待 B014，不并行施工。

Owner 已批准 CHANGE-M2-B010-PLAYER-PRESENTATION-V1。独立公共文档和8定义 Schema 已由签名源 6df9760d3b673891a9a9c1904ca36011757214fe / tree 5326e802b0adb221775cc51bc3e87dd406dcea2a 采用并通过 fresh PLAN、codex check、just check；原606文件逐字节继承、旧 PLAYER_API 的15路由/39定义不变。公共契约登记与治理检查不计业务完成。

B014 只允许九项新文件/测试路径，单一 GET 由当前已验证包图和真实授权兼容模型资料产生，遵守完整列表、closed Schema、同源 HTTPS Cookie、当前租户/访客授权及既有准备/启动复验。显式 platformPlayersWithPresentation 组合入口必须由 B014 真实 HTTPS/独占 PostgreSQL/生产 Lua 集成实际调用；无人调用的新增函数、写死包清单或假 ready 不算通过。正式 daemon/Compose 接入仍由 B011 负责；本轮仅给未冻结 B011 增加 B014 依赖和接线验收，不扩大其允许路径，B012 经 B011 保留完整出口依赖。当前 main.go 仅有 M1/基线分派，B011 冻结前须在正常 PLAN 核定实际启动入口和精确路径，不得以 init/linkname 绕行，也不得把后端改动塞进 B010。

当前 Linux M2 完成10/14≈71.43%（仅按批次数量；分母因新增上游批次增加）。B014 尚未冻结或实现，业务、真实数据库/浏览器/模型验收、独立接收、Main 接收、清理封存与远端提交均未据此完成；其他平台业务 NOT_RUN。B014 后续须沿 PLANNED→FROZEN→IMPLEMENTING 合法推进，独立验收、准确来源证据、接收清理和普通远端推送完成后再恢复原 B010。

## 此前状态（保留原文）

## 当前 Linux M2：B010 玩家网页进入实现

正常 PLAN v46→v47 仅调整未开始 B010 或推进其合法状态；B001–B009/B013 完成记录、其余十二批、next14、墓碑和 M1 保持。B010 业务范围仅 apps/web-player/** 与 tests/e2e/player/**，不得修改后端、公共契约、规范、许可或治理控制面。依赖均已完整验收。

B013 精确签名源 4a35626e72ef3b9675137e85c8ac13b496d1ca4e / tree b182e2b0776e4eabe7d618d02bc3195a9a86fc1e 已业务及最终状态独立接收、本地主线仅快进接收、归档完整回读、十个专属测试数据库及私密凭证/缓存/可重建产物清理，并普通非强制推送远端 codex/m2-room-platform；远端准确源已复核。完整关闭回执 SHA256 7dda3c12c3c875a19f272964834706f5445e946b9eb3eed163033d2238ba5087，路径 m2-b013-completed-linux-4a35626e/FINAL_BATCH_RECEIPT.json。941 项必要具名测试及18项额外原生检查通过；最终状态仅三文件变化，602业务文件逐字节继承，明确不虚报状态提交重新执行业务测试。原 R1–R10 FAIL、缺夹具 NOT_RUN 与准备诊断保留且不计旧运行通过。

当前 Linux M2 完成10/13≈76.92%（按批次数量），剩余 B010/B011/B012；其他平台业务 NOT_RUN。B010 消费已批准 auth/room/player v1，新增阅读与 Schema 引用只是采用既有15路由/39定义，未改变公共语义。正常界面采用游戏、房间、参与者语言；高级模型配置按已有边界处理，不引入浏览器本地模型或资格捷径。真实浏览器验收采用已有 Linux Chrome、真实 PostgreSQL、生产 Lua runner 与同源 HTTPS，明确区分合成本地模型夹具和付费外部资格。

B010 尚未业务验收、Main接收、清理封存或远端提交。任何未执行测试不计通过。规范或公共接口不足、冻结范围外变更、未解决隐私/租户/门禁或必要检查失败时按既有门禁停止并提交具体提案。完成后仍复用已获 Owner 授权的一名只读独立验收代理；不新增实现代理。

## 此前状态（保留原文）

## 当前 Linux M2：B010 玩家网页冻结既有玩家契约与浏览器验收范围

正常 PLAN v45→v46 仅调整未开始 B010 或推进其合法状态；B001–B009/B013 完成记录、其余十二批、next14、墓碑和 M1 保持。B010 业务范围仅 apps/web-player/** 与 tests/e2e/player/**，不得修改后端、公共契约、规范、许可或治理控制面。依赖均已完整验收。

B013 精确签名源 4a35626e72ef3b9675137e85c8ac13b496d1ca4e / tree b182e2b0776e4eabe7d618d02bc3195a9a86fc1e 已业务及最终状态独立接收、本地主线仅快进接收、归档完整回读、十个专属测试数据库及私密凭证/缓存/可重建产物清理，并普通非强制推送远端 codex/m2-room-platform；远端准确源已复核。完整关闭回执 SHA256 7dda3c12c3c875a19f272964834706f5445e946b9eb3eed163033d2238ba5087，路径 m2-b013-completed-linux-4a35626e/FINAL_BATCH_RECEIPT.json。941 项必要具名测试及18项额外原生检查通过；最终状态仅三文件变化，602业务文件逐字节继承，明确不虚报状态提交重新执行业务测试。原 R1–R10 FAIL、缺夹具 NOT_RUN 与准备诊断保留且不计旧运行通过。

当前 Linux M2 完成10/13≈76.92%（按批次数量），剩余 B010/B011/B012；其他平台业务 NOT_RUN。B010 消费已批准 auth/room/player v1，新增阅读与 Schema 引用只是采用既有15路由/39定义，未改变公共语义。正常界面采用游戏、房间、参与者语言；高级模型配置按已有边界处理，不引入浏览器本地模型或资格捷径。真实浏览器验收采用已有 Linux Chrome、真实 PostgreSQL、生产 Lua runner 与同源 HTTPS，明确区分合成本地模型夹具和付费外部资格。

B010 尚未业务验收、Main接收、清理封存或远端提交。任何未执行测试不计通过。规范或公共接口不足、冻结范围外变更、未解决隐私/租户/门禁或必要检查失败时按既有门禁停止并提交具体提案。完成后仍复用已获 Owner 授权的一名只读独立验收代理；不新增实现代理。

## 此前状态（保留原文）

## 当前 Linux M2：B010 玩家网页规划契约引用与真实浏览器验收

正常 PLAN v44→v45 仅调整未开始 B010 或推进其合法状态；B001–B009/B013 完成记录、其余十二批、next14、墓碑和 M1 保持。B010 业务范围仅 apps/web-player/** 与 tests/e2e/player/**，不得修改后端、公共契约、规范、许可或治理控制面。依赖均已完整验收。

B013 精确签名源 4a35626e72ef3b9675137e85c8ac13b496d1ca4e / tree b182e2b0776e4eabe7d618d02bc3195a9a86fc1e 已业务及最终状态独立接收、本地主线仅快进接收、归档完整回读、十个专属测试数据库及私密凭证/缓存/可重建产物清理，并普通非强制推送远端 codex/m2-room-platform；远端准确源已复核。完整关闭回执 SHA256 7dda3c12c3c875a19f272964834706f5445e946b9eb3eed163033d2238ba5087，路径 m2-b013-completed-linux-4a35626e/FINAL_BATCH_RECEIPT.json。941 项必要具名测试及18项额外原生检查通过；最终状态仅三文件变化，602业务文件逐字节继承，明确不虚报状态提交重新执行业务测试。原 R1–R10 FAIL、缺夹具 NOT_RUN 与准备诊断保留且不计旧运行通过。

当前 Linux M2 完成10/13≈76.92%（按批次数量），剩余 B010/B011/B012；其他平台业务 NOT_RUN。B010 消费已批准 auth/room/player v1，新增阅读与 Schema 引用只是采用既有15路由/39定义，未改变公共语义。正常界面采用游戏、房间、参与者语言；高级模型配置按已有边界处理，不引入浏览器本地模型或资格捷径。真实浏览器验收采用已有 Linux Chrome、真实 PostgreSQL、生产 Lua runner 与同源 HTTPS，明确区分合成本地模型夹具和付费外部资格。

B010 尚未业务验收、Main接收、清理封存或远端提交。任何未执行测试不计通过。规范或公共接口不足、冻结范围外变更、未解决隐私/租户/门禁或必要检查失败时按既有门禁停止并提交具体提案。完成后仍复用已获 Owner 授权的一名只读独立验收代理；不新增实现代理。

## 此前状态（保留原文）

## 当前 M2：B013 Linux 玩家服务端 API 业务已独立验收，等待最终状态接收与远端提交

正常 PLAN v43→v44 仅将真实 M2-B013 VERIFYING→COMPLETED；十四字段冻结摘要 5d65d5fa4e239f20579ea05637336831c515fc3b93032a56ecabeecdac21f3e2、其余十二批、next14、tombstones 和 M1 状态保持。当前 Linux M2 已完成 10/13≈76.92%；其他平台业务 NOT_RUN。B010 仍须在 B013 本地主线接收、清理、归档和远端提交后启动。

准确业务签名源 4b08311f5a1fc791fa5a88691dbfcb443c28ef9d / tree 7545d2010a78cf32e03438973e3f920c944cf467 在全新、专属且物理归属核验的 PostgreSQL 夹具与生产 Lua runner 上完成实际 HTTPS/Actor/队列/计费/重启/私密视图/租约和暂停链路；B013 集成12/12、竞态12/12，相关旧 launch/platform_session/continuation/model_gateway 原断言集成及竞态均实际退出0、零失败/跳过。完整单元、just check、just test、go vet ./...、Linux just ci 及签名/正式原生 codex check 通过。当前精确命名计数、日志及资源哈希在 BUSINESS_PRODUCER_RECEIPT_R11.json、PRODUCER_SOURCE_MANIFEST_R11.json、REQUIRED_FAILURE_CLOSURE_R11.json；原失败 R1–R10 保留并由当前实际检查关闭，缺夹具 NOT_RUN/诊断不计业务 PASS。

本批仅采用 Owner 已批准15个同源 HTTPS 玩家路由与39个 Schema 定义，Cookie/CSRF/幂等及 workspace/room/当前席位授权保留。默认连接只恢复视图；任一已入场参与者可暂停，必要人工席位须各自明确确认才能恢复，主持不得代签；服务重启保持暂停。外部 AI/通用任务、回退、重试与 Actor mutation 共享持久控制及执行 fence，暂停确认前完成既有提交和取消/静止，确认后不产生新派发、回退、计费或状态改变。短租约与控制/准备修订在 mutation 前复验；分页保持游标进展及当前私密过滤，原导出种类和恢复点权限保留。

Owner 于2026-10-09明确批准 CHANGE-M2-B013-AI-CONTINUATION-RECOVERY 精确两个上游文件：恢复校验兼容已合法提交的 task/ai continuation，追加来源引用/包/Schema/闭合字段回归。原完成断言逐字保留，M1命令/事件/checkpoint字节、权限、SQL、依赖和许可未改；批准回执 SHA256 205940992d5ba2329bebdc2abf9be3671c713bc251e1ae21f0df80ec4baac091。原冻结字段未重写，仅以有审计的 Owner 限定补修接收。B013原业务32路径与另行批准的3文件补修分别归属，全部605源码 blob/mode/完整字节回读。Owner 随后明确批准 CHANGE-M2-B013-OPTIONAL-PLAYER-FIXTURE：仅修复 internal/projectctl/projectctl_test.go 的 absent 文档夹具提交，原路由调用、三用例和断言逐字保留；生产门禁未变。该单文件批准回执 SHA256 5ef2c3a01c00b0eb87e5b6431b846808740112309d8b0418357f0701aac13dcd。完整 just test 对该失败和归档父路径用例均实际通过，所有原失败保留；包串行与独有临时目录属于测试运行隔离，文件稳定性断言未降标。此前 GOV-M2-PLAYER-API-REGISTRY 源830a78b4已独立接收、清理封存并远端提交，维护许可未混入本批代码范围。

测试模型是合成本地 HTTP 服务；未调用付费外部提供者，也未将此计作真实模型资格。玩家浏览器完整界面、生产守护进程和 Compose、最终 M2 出口分别仍由 B010/B011/B012 承担。本批不改多平台范围。

只读独立验收已对准确 VERIFYING 源实际重跑所有必要检查并审查源码，Root全成员回读接收通过，未决必需项为空；回执在ROOT_INDEPENDENT_BUSINESS_RECEPTION.json。最终三状态接收、Main接收、专属资源清理封存与普通远端推送仍须完成后再推进B010。

## 此前状态（保留原文）

## 当前 M2：B013 Linux 玩家服务端 API 等待独立验收

正常 PLAN v42→v43 仅将真实 M2-B013 IMPLEMENTING→VERIFYING；十四字段冻结摘要 5d65d5fa4e239f20579ea05637336831c515fc3b93032a56ecabeecdac21f3e2、其余十二批、next14、tombstones 和 M1 状态保持。当前 Linux M2 已完成 9/13≈69.23%；其他平台业务 NOT_RUN。B010 仍须在 B013 本地主线接收、清理、归档和远端提交后启动。

准确业务签名源 4b08311f5a1fc791fa5a88691dbfcb443c28ef9d / tree 7545d2010a78cf32e03438973e3f920c944cf467 在全新、专属且物理归属核验的 PostgreSQL 夹具与生产 Lua runner 上完成实际 HTTPS/Actor/队列/计费/重启/私密视图/租约和暂停链路；B013 集成12/12、竞态12/12，相关旧 launch/platform_session/continuation/model_gateway 原断言集成及竞态均实际退出0、零失败/跳过。完整单元、just check、just test、go vet ./...、Linux just ci 及签名/正式原生 codex check 通过。当前精确命名计数、日志及资源哈希在 BUSINESS_PRODUCER_RECEIPT_R11.json、PRODUCER_SOURCE_MANIFEST_R11.json、REQUIRED_FAILURE_CLOSURE_R11.json；原失败 R1–R10 保留并由当前实际检查关闭，缺夹具 NOT_RUN/诊断不计业务 PASS。

本批仅采用 Owner 已批准15个同源 HTTPS 玩家路由与39个 Schema 定义，Cookie/CSRF/幂等及 workspace/room/当前席位授权保留。默认连接只恢复视图；任一已入场参与者可暂停，必要人工席位须各自明确确认才能恢复，主持不得代签；服务重启保持暂停。外部 AI/通用任务、回退、重试与 Actor mutation 共享持久控制及执行 fence，暂停确认前完成既有提交和取消/静止，确认后不产生新派发、回退、计费或状态改变。短租约与控制/准备修订在 mutation 前复验；分页保持游标进展及当前私密过滤，原导出种类和恢复点权限保留。

Owner 于2026-10-09明确批准 CHANGE-M2-B013-AI-CONTINUATION-RECOVERY 精确两个上游文件：恢复校验兼容已合法提交的 task/ai continuation，追加来源引用/包/Schema/闭合字段回归。原完成断言逐字保留，M1命令/事件/checkpoint字节、权限、SQL、依赖和许可未改；批准回执 SHA256 205940992d5ba2329bebdc2abf9be3671c713bc251e1ae21f0df80ec4baac091。原冻结字段未重写，仅以有审计的 Owner 限定补修接收。B013原业务32路径与另行批准的3文件补修分别归属，全部605源码 blob/mode/完整字节回读。Owner 随后明确批准 CHANGE-M2-B013-OPTIONAL-PLAYER-FIXTURE：仅修复 internal/projectctl/projectctl_test.go 的 absent 文档夹具提交，原路由调用、三用例和断言逐字保留；生产门禁未变。该单文件批准回执 SHA256 5ef2c3a01c00b0eb87e5b6431b846808740112309d8b0418357f0701aac13dcd。完整 just test 对该失败和归档父路径用例均实际通过，所有原失败保留；包串行与独有临时目录属于测试运行隔离，文件稳定性断言未降标。此前 GOV-M2-PLAYER-API-REGISTRY 源830a78b4已独立接收、清理封存并远端提交，维护许可未混入本批代码范围。

测试模型是合成本地 HTTP 服务；未调用付费外部提供者，也未将此计作真实模型资格。玩家浏览器完整界面、生产守护进程和 Compose、最终 M2 出口分别仍由 B010/B011/B012 承担。本批不改多平台范围。

独立验收、Main接收、本阶段专属资源清理、封存和远端推送尚未完成，B013不计完成。下一步复用既有 Owner 授权的一名只读独立验收代理，独占本批专属测试数据库，不修改生产源码。

## 此前状态（保留原文）

## 当前 M2：B013 Linux玩家API实现启动

正常 PLAN v41→v42 仅将 M2-B013 FROZEN→IMPLEMENTING，原生十四字段冻结摘要 5d65d5fa4e239f20579ea05637336831c515fc3b93032a56ecabeecdac21f3e2。永久分配sequence13、next14、九个已完成冻结契约及B010–B012的分配编号与计划内容均保持；B010依赖B013保证API先行。Linux M2完成9/13≈69.23%；B013业务测试、完整浏览器循环及其他平台均NOT_RUN。

Owner“同意”已批准 CHANGE-M2-PLAYER-API-V1；精确15路由/39定义及17业务路径的原始草案/授权SHA在冻结acceptance中绑定。只采纳DRAFT→ACTIVE元数据/注释，公共语义改变仍须新的Owner决定。原外部规划草案将分配编号误作执行顺序导致actual1，辅助缺失overlay实际1均保留；修正后的原生PLAN投影actual0，旧12分配编号与9完成契约不变，仓库控制门禁未改。

上游治理830a78b4已独立验收、Main接收、清理归档并核对远端；其登记权限不计玩家业务信用。B009完成与旧历史保持。本批连接原生Actor、房间准备/开局、服务端Session及持久暂停/连接租约，统一人类与AI续接控制；只使用专属真实物理PostgreSQL与本地合成provider进行集成/race/租户/访客/隐私/幂等/暂停/断线/重启/导出回归，全部必需检查和独立收件通过后再记完成并提交远端。生产启动/Compose保留B011，玩家界面保留B010。

## 此前状态（保留原文）

## 当前 M2：B013 玩家API契约冻结

正常 PLAN v40→v41 仅将 M2-B013 PLANNED→FROZEN，原生十四字段冻结摘要 5d65d5fa4e239f20579ea05637336831c515fc3b93032a56ecabeecdac21f3e2。永久分配sequence13、next14、九个已完成冻结契约及B010–B012的分配编号与计划内容均保持；B010依赖B013保证API先行。Linux M2完成9/13≈69.23%；B013业务测试、完整浏览器循环及其他平台均NOT_RUN。

Owner“同意”已批准 CHANGE-M2-PLAYER-API-V1；精确15路由/39定义及17业务路径的原始草案/授权SHA在冻结acceptance中绑定。只采纳DRAFT→ACTIVE元数据/注释，公共语义改变仍须新的Owner决定。原外部规划草案将分配编号误作执行顺序导致actual1，辅助缺失overlay实际1均保留；修正后的原生PLAN投影actual0，旧12分配编号与9完成契约不变，仓库控制门禁未改。

上游治理830a78b4已独立验收、Main接收、清理归档并核对远端；其登记权限不计玩家业务信用。B009完成与旧历史保持。本批连接原生Actor、房间准备/开局、服务端Session及持久暂停/连接租约，统一人类与AI续接控制；只使用专属真实物理PostgreSQL与本地合成provider进行集成/race/租户/访客/隐私/幂等/暂停/断线/重启/导出回归，全部必需检查和独立收件通过后再记完成并提交远端。生产启动/Compose保留B011，玩家界面保留B010。

## 此前状态（保留原文）

## 当前 M2：新增 B013 玩家 API 上游批次

Owner“同意”已批准 CHANGE-M2-PLAYER-API-V1，授权回执SHA256 3d7668786ab23ceadc6a50bfe5e85b22d076b3652d910de28dd687ef67cbc024。正常 PLAN v39→v40 新增未使用ID M2-B013 为永久分配sequence13；B010/B011/B012保持原分配编号10/11/12，next14，tombstones及九个已完成冻结契约原样保留。草案曾把分配编号当执行顺序，外部原生预检actual1拒绝sequence10；失败原件保留且未修改仓库。当前按原生单调分配规则追加13，通过B010对B013的依赖保证API先行，不改变公开API或业务目录。B010增加B013依赖和当前已存在auth/room契约引用；新增player规范/Schema在B013 IMPLEMENT采纳并验收后才登记到仍未开始的B010，绝不提前请求不存在的规范文件。

Linux M2统计分母由12调整为13，当前完成9/13≈69.23%。这是补齐浏览器与权威服务连接所需上游批次的范围细化；九个既有完成批次没有失效。B013业务实现、真实SQL/race和完整玩家浏览器循环均NOT_RUN；其他平台NOT_RUN。

独立 GOV-M2-PLAYER-API-REGISTRY 当前签名源830a78b4 已通过 Root 和独立治理436/436、必需Linux检查与五个前向签名；原样77登记探针、原017的3探针和18补充路径探针全部通过，001/002当前闭合，旧FAIL日志保持。577源码及574范围外文件完整字节绑定。独立217证据成员由Root完整收件；本地Main保持main分支快进接收同源、原生28节点与check通过。两份快照、三个专属缓存、两处可重建输出和12个临时助手已归档后清理；阶段完成归档1249成员完整回读。远端codex/m2-room-platform已核对到830a78b4，远端回执SHA256 466afb579cd80238fe4dad24f098eed91db5a8b754db0b1d4ba4ee1a029000ac。此治理完成不计玩家业务信用。

B009最终状态31178c0的独立接收、Main接收、资源清理、归档与远端推送均已完成，准确回执保存在m2-b009-linux-final及m2-b009-remote-reception-31178c0。此前“等待最终状态接收”的记录作为历史原文保留。

下一步正常PLAN冻结并激活B013，在已批准的17条业务路径内采纳精确15条HTTPS路由及39个Schema定义，连接现有房间、开局、原生Session和AI续接，并补齐持久暂停/连接租约。生产启动接线保留B011，玩家界面保留B010；当前不改业务源码。

## 此前状态（保留原文）

## 当前 M2：B009 Linux 模型网关 业务已独立验收，等待最终状态接收

正常 PLAN v38→v39 仅将 M2-B009 VERIFYING→COMPLETED；十四字段冻结摘要 3b1d11d51100dac5d3da52c714c33da89cd90ec9c9f2a78b5931d819c8194890 保持，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成 9/12=75%，其他平台业务NOT_RUN。

生产验证准确签名源bc3d93c91584e88d26d25495e7da7771c1d32bac在新建专属物理PostgreSQL夹具上实测unit61/61、SQL16/16、race77/77、相关回归73/73、独立B006旧SQL回归26/26、治理381/381，均零失败/跳过；just check、just test、go vet ./...及Linux just ci退出0。无夹具探针实际1、NOT_RUN、零业务用例/跳过，不计PASS。生产证据在m2-b009-client-platform/BUSINESS_PRODUCER_RECEIPT.json及REQUIRED_B009_FAILURE_CLOSURE_R5.json；原始生产与只读诊断失败原样保留，完整当前检查已重新执行。

B009服务端固定兼容协议适配器执行出口、凭据、超时、大小和资格限制；AI结构化建议先由权威Lua Actor事务提交，再生成叙述。任务、模型资格、席位视图及五级八维预算继承已有边界；有界格式修复和降级需预授权与能力兼容，不确定消耗保留预算，不重复付费调度，无可用降级时暂停席位。真实SQL验证持久叙述及确定结果模板，叙述失败不回滚事件。测试使用合成本地HTTP提供者与真实物理SQL，未付费调用外部提供者；完整玩家界面循环和生产监听仍由后续批次闭环。

既有CHANGE-M2-B009-AI-INTENT-SOURCE批准的两文件修复仅接纳对应已提交派发的AI意图。另经Owner三文件CHANGE-M2-MODEL-GATEWAY-SCOPE授权及正式GOV维护验收补齐M2冻结权限读取和SDK拒绝；维护独立governance381/381、反向探针103/103及全部Linux检查通过，711成员全回读，原001–005失败保持，Bootstrap018永久退役，临时阶段产物已清理。维护回执在gov-m2-model-gateway-scope/INDEPENDENT_BC3D93C_PASS_RECEPTION.json；本批不将维护权限混入业务目录。当前29个相对B008来源变化中，23为冻结B009允许业务路径，3为上述独立接收维护路径，3为正常PLAN状态。全部576源码和573非状态blob/mode/字节绑定；当前PLAN仅改变三状态，573非状态源码保持，并按准确生产来源记录检查继承。

业务候选已由Owner授权的只读独立验收代理实际重跑及源码审查，Root完整回读接收通过，未决必需项为空；证据在ROOT_INDEPENDENT_BUSINESS_RECEPTION.json。最终三状态文件接收、本地主线接收、专属资源清理、封存及普通远端推送尚未完成，需全部通过后按Owner“每批次完工提交远端”继续推进B010。

## 此前状态（保留原文）

## 当前 M2：B009 Linux 模型网关 候选等待独立验收

正常 PLAN v37→v38 仅将 M2-B009 IMPLEMENTING→VERIFYING；十四字段冻结摘要 3b1d11d51100dac5d3da52c714c33da89cd90ec9c9f2a78b5931d819c8194890 保持，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成 8/12≈66.67%，其他平台业务NOT_RUN。

生产验证准确签名源bc3d93c91584e88d26d25495e7da7771c1d32bac在新建专属物理PostgreSQL夹具上实测unit61/61、SQL16/16、race77/77、相关回归73/73、独立B006旧SQL回归26/26、治理381/381，均零失败/跳过；just check、just test、go vet ./...及Linux just ci退出0。无夹具探针实际1、NOT_RUN、零业务用例/跳过，不计PASS。生产证据在m2-b009-client-platform/BUSINESS_PRODUCER_RECEIPT.json及REQUIRED_B009_FAILURE_CLOSURE_R5.json；原始生产与只读诊断失败原样保留，完整当前检查已重新执行。

B009服务端固定兼容协议适配器执行出口、凭据、超时、大小和资格限制；AI结构化建议先由权威Lua Actor事务提交，再生成叙述。任务、模型资格、席位视图及五级八维预算继承已有边界；有界格式修复和降级需预授权与能力兼容，不确定消耗保留预算，不重复付费调度，无可用降级时暂停席位。真实SQL验证持久叙述及确定结果模板，叙述失败不回滚事件。测试使用合成本地HTTP提供者与真实物理SQL，未付费调用外部提供者；完整玩家界面循环和生产监听仍由后续批次闭环。

既有CHANGE-M2-B009-AI-INTENT-SOURCE批准的两文件修复仅接纳对应已提交派发的AI意图。另经Owner三文件CHANGE-M2-MODEL-GATEWAY-SCOPE授权及正式GOV维护验收补齐M2冻结权限读取和SDK拒绝；维护独立governance381/381、反向探针103/103及全部Linux检查通过，711成员全回读，原001–005失败保持，Bootstrap018永久退役，临时阶段产物已清理。维护回执在gov-m2-model-gateway-scope/INDEPENDENT_BC3D93C_PASS_RECEPTION.json；本批不将维护权限混入业务目录。当前29个相对B008来源变化中，23为冻结B009允许业务路径，3为上述独立接收维护路径，3为正常PLAN状态。全部576源码和573非状态blob/mode/字节绑定；当前PLAN仅改变三状态，573非状态源码保持，并按准确生产来源记录检查继承。

独立验收、本地主线接收、清理封存及远端推送尚未执行，B009不记完成。下一步复用Owner已授权的只读独立验收代理；Root完整接收通过后再转完成。

## 此前状态（保留原文）

## 当前 M2：B009 Linux模型网关实现启动

正常 PLAN v36→v37 仅将尚未开始的 M2-B009 FROZEN→IMPLEMENTING，原生十四字段摘要 3b1d11d51100dac5d3da52c714c33da89cd90ec9c9f2a78b5931d819c8194890。其余十一批、next13、tombstones及M1归档保持。Linux M2完成8/12≈66.67%；B009业务NOT_RUN，其他平台NOT_RUN，最终完整出口由B012闭环。

B008最终签名源5008b82已完成独立业务验收、最终状态接收、本地主线接收、清理及远端普通推送，准确远端哈希已核对。业务候选c1d68ed实测单元17、真实SQL20、race37、治理315全部通过；独立7个真实SQL/race探针通过。最终550非状态文件与业务候选按全部字节、blob、mode一致，当前业务明确NOT_RERUN。首次SQL20 run/2 pass/18 fail及所有NOT_RUN、辅助失败保留；正式修复后闭合，未改写历史验收。完成档案SHA256 f1c6bda2da99fb8d8e8bc1c99ffbd7bb818ed47bc5b0cf7a744cf5f3c7b3e7d8，1437成员完整回读。专属数据库、秘密文件、已释放副本和可重建缓存已清理，证据保留。

Owner直接“批准”已授权CHANGE-M2-B009-AI-INTENT-SOURCE。原host.ai.request写入ai和dispatch-ai，既有B006来源入口只接收task；本批冻结前仅增加platform_task_source.go与新platform_task_source_test.go两个明确批准的上游路径，补齐已有异步AI入口与严格类型/租户/来源配对。具体批准补丁SHA256 e2a1f5e6be8eace7f91b1a5cac01e4650922f3284d7a5fea272375071d9dd370，授权回执SHA256 28bc206b56fbd615e3bf7b0a1a9578522e25a8db913604cf13c6d8b5523b3d31。B006原冻结契约和完成证据保留，補修及回归单独绑定本批候选，不增加公开API/Schema/Host操作、许可或V1范围。

B009完成服务器端OpenAI-compatible、Ollama/llama.cpp兼容服务器的有限模型调用、端点网络及秘密边界、能力认证、持久任务、结构化动作先验证提交再叙事、有限格式修复和预授权兼容后备。沿用B006持久租约和Actor回调、B007模型绑定及凭据库、B008过滤上下文及真实多级预算。仅合成本地provider夹具参与本批网络验证，付费外部提供方认证及完整混合游戏由B012验收，production daemon接线由B011完成。本批新增race、相关单元和原B006真实SQL回归，不降低原验收标准。实现和业务测试均尚未执行。

## 此前状态（保留原文）

## 当前 M2：B009 模型网关契约冻结

正常 PLAN v35→v36 仅将尚未开始的 M2-B009 PLANNED→FROZEN，原生十四字段摘要 3b1d11d51100dac5d3da52c714c33da89cd90ec9c9f2a78b5931d819c8194890。其余十一批、next13、tombstones及M1归档保持。Linux M2完成8/12≈66.67%；B009业务NOT_RUN，其他平台NOT_RUN，最终完整出口由B012闭环。

B008最终签名源5008b82已完成独立业务验收、最终状态接收、本地主线接收、清理及远端普通推送，准确远端哈希已核对。业务候选c1d68ed实测单元17、真实SQL20、race37、治理315全部通过；独立7个真实SQL/race探针通过。最终550非状态文件与业务候选按全部字节、blob、mode一致，当前业务明确NOT_RERUN。首次SQL20 run/2 pass/18 fail及所有NOT_RUN、辅助失败保留；正式修复后闭合，未改写历史验收。完成档案SHA256 f1c6bda2da99fb8d8e8bc1c99ffbd7bb818ed47bc5b0cf7a744cf5f3c7b3e7d8，1437成员完整回读。专属数据库、秘密文件、已释放副本和可重建缓存已清理，证据保留。

Owner直接“批准”已授权CHANGE-M2-B009-AI-INTENT-SOURCE。原host.ai.request写入ai和dispatch-ai，既有B006来源入口只接收task；本批冻结前仅增加platform_task_source.go与新platform_task_source_test.go两个明确批准的上游路径，补齐已有异步AI入口与严格类型/租户/来源配对。具体批准补丁SHA256 e2a1f5e6be8eace7f91b1a5cac01e4650922f3284d7a5fea272375071d9dd370，授权回执SHA256 28bc206b56fbd615e3bf7b0a1a9578522e25a8db913604cf13c6d8b5523b3d31。B006原冻结契约和完成证据保留，補修及回归单独绑定本批候选，不增加公开API/Schema/Host操作、许可或V1范围。

B009完成服务器端OpenAI-compatible、Ollama/llama.cpp兼容服务器的有限模型调用、端点网络及秘密边界、能力认证、持久任务、结构化动作先验证提交再叙事、有限格式修复和预授权兼容后备。沿用B006持久租约和Actor回调、B007模型绑定及凭据库、B008过滤上下文及真实多级预算。仅合成本地provider夹具参与本批网络验证，付费外部提供方认证及完整混合游戏由B012验收，production daemon接线由B011完成。本批新增race、相关单元和原B006真实SQL回归，不降低原验收标准。实现和业务测试均尚未执行。

## 此前状态（保留原文）

## 当前 M2：B008 Linux AI 上下文隔离与资源预算 业务已独立验收，等待最终状态接收

正常 PLAN v34→v35 仅将 M2-B008 VERIFYING→COMPLETED；十四字段冻结摘要 b702faf607a4d61d518301a5f696de63c6476a787bc0563d495a667b21a61faa 保持，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成 8/12≈66.67%；其他平台业务NOT_RUN，最终出口由B012闭环。

B008在11个冻结范围内新增源码/测试文件，将当前真实账号、房间、AI席位、开局绑定、模型资格和受控能力交集在提示构造前校验；受保护宿主状态和事件先按现有席位视图过滤，记忆仅绑定本席位及仍可见事件来源。提示、任务及建议使用不透明私有句柄；模型结果仅为建议，不获得宿主游戏状态写入权限。workspace、room、session、seat、task五级预算对call、token、cost、latency、tool、subagent、context和local compute八项资源进行真实SQL事务预留及有限上界检查；重复调度不能重复调用，超限暂停相关席位，未知消耗保留预留，重启不能重置额度。

清洁签名业务候选c1d68edd82f8c59f93e589a2bd76bf5c58ccd9e3实测单元17/17、专用真实PostgreSQL20/20、race37/37、治理315/315，零失败/跳过；just check、just test、go vet ./...及Linux just ci实际退出0。首次准确7dca67694f8855dbf7d6c54c35fb6d415361b984 SQL实际20 run/2 pass/18 fail/0 skip原样保留：新增夹具更新准备信息后未登记对应新确认，现有授权正确拒绝。正式REPAIR仅修复本批isolation_test.go初始化，在同一事务明确登记revision2及其有效确认；原有全部Test函数和断言逐字节一致，未修改B007或既有授权检查。两次只读诊断实际1、零业务PASS信用；修复后所有必需检查重新运行，生产证据在仓库外m2-b008-ai-platform/BUSINESS_PRODUCER_RECEIPT.json及REQUIRED_SQL_FAILURE_R1_CLOSURE.json。

真实PostgreSQL夹具内使用受控合成资格、视图策略和类型化provider回调验证隔离及额度；外部provider资格和网络调用NOT_RUN，由B012闭环，混合玩家完整游戏循环及production daemon启动NOT_RUN，由B011/B012闭环。无物理夹具的探针明确非零退出、NOT_RUN且零业务用例/跳过。未新增公开HTTP/JSON Schema、Host方法、Lua后端、许可或依赖，539个此前非状态文件完整保留；最后三状态文件改变后550个非状态blob/mode必须准确一致，业务检查仅按准确来源继承NOT_RERUN。

业务候选已由Owner授权的只读独立验收代理实际重跑及源码审查，Root完整回读接收通过，未决必需项为空；证据在ROOT_INDEPENDENT_BUSINESS_RECEPTION.json。最终三状态文件接收、本地主线接收、专属资源清理、封存及普通远端推送尚未完成，需全部通过后按Owner“每批次完工提交远端”继续推进B009。

## 此前状态（保留原文）

## 当前 M2：B008 Linux AI 上下文隔离与资源预算 候选等待独立验收

正常 PLAN v33→v34 仅将 M2-B008 IMPLEMENTING→VERIFYING；十四字段冻结摘要 b702faf607a4d61d518301a5f696de63c6476a787bc0563d495a667b21a61faa 保持，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成 7/12≈58.33%；其他平台业务NOT_RUN，最终出口由B012闭环。

B008在11个冻结范围内新增源码/测试文件，将当前真实账号、房间、AI席位、开局绑定、模型资格和受控能力交集在提示构造前校验；受保护宿主状态和事件先按现有席位视图过滤，记忆仅绑定本席位及仍可见事件来源。提示、任务及建议使用不透明私有句柄；模型结果仅为建议，不获得宿主游戏状态写入权限。workspace、room、session、seat、task五级预算对call、token、cost、latency、tool、subagent、context和local compute八项资源进行真实SQL事务预留及有限上界检查；重复调度不能重复调用，超限暂停相关席位，未知消耗保留预留，重启不能重置额度。

清洁签名业务候选c1d68edd82f8c59f93e589a2bd76bf5c58ccd9e3实测单元17/17、专用真实PostgreSQL20/20、race37/37、治理315/315，零失败/跳过；just check、just test、go vet ./...及Linux just ci实际退出0。首次准确7dca67694f8855dbf7d6c54c35fb6d415361b984 SQL实际20 run/2 pass/18 fail/0 skip原样保留：新增夹具更新准备信息后未登记对应新确认，现有授权正确拒绝。正式REPAIR仅修复本批isolation_test.go初始化，在同一事务明确登记revision2及其有效确认；原有全部Test函数和断言逐字节一致，未修改B007或既有授权检查。两次只读诊断实际1、零业务PASS信用；修复后所有必需检查重新运行，生产证据在仓库外m2-b008-ai-platform/BUSINESS_PRODUCER_RECEIPT.json及REQUIRED_SQL_FAILURE_R1_CLOSURE.json。

真实PostgreSQL夹具内使用受控合成资格、视图策略和类型化provider回调验证隔离及额度；外部provider资格和网络调用NOT_RUN，由B012闭环，混合玩家完整游戏循环及production daemon启动NOT_RUN，由B011/B012闭环。无物理夹具的探针明确非零退出、NOT_RUN且零业务用例/跳过。未新增公开HTTP/JSON Schema、Host方法、Lua后端、许可或依赖，539个此前非状态文件完整保留；最后三状态文件改变后550个非状态blob/mode必须准确一致，业务检查仅按准确来源继承NOT_RERUN。

独立验收、本地主线接收、清理封存及远端推送尚未执行，B008不记完成。下一步复用Owner已授权的只读独立验收代理；Root完整接收通过后再转完成。

## 此前状态（保留原文）

## 当前 M2：B008 Linux AI席位隔离与预算实现启动

正常 PLAN v32→v33 仅冻结/启动尚未开始的 M2-B008，FROZEN→IMPLEMENTING；原生十四字段摘要 b702faf607a4d61d518301a5f696de63c6476a787bc0563d495a667b21a61faa。其余十一批、next13、tombstones及M1归档保持。Linux M2已完成7/12≈58.33%，B008业务NOT_RUN；其他平台业务NOT_RUN，最终出口由B012闭环。

B007准确最终签名源f81c016d25a27e03716a7328651880d1fa144f54通过独立业务与最终复核、Root和本地主线接收；542源码、181已接受业务证据及235最终复核证据完整回读。业务源8b1884e上单元75/75、真实SQL47/47、并发122/122、原始不变时区探针2/2、治理315/315及check/test/vet/Linux CI实际通过。最终仅三状态变化，539非状态文件按准确blob/mode/全部字节一致继承且当前明确NOT_RERUN，最终独立治理315/315、原生状态投影1/1及13项当前必需actual0。最初bf337b2的时区失败2run/1pass/1fail/0skip及后续NOT_RUN保持；仅经三文件正式范围内REPAIR与原断言不变复查闭合。专用容器、两份凭据及三个已释放副本已清理。完成档案SHA256 d6a1d737ecc5fac5b9c0087ebe67b150008f202bcbff9fe0a0b09fbff30f4a5f，1644成员全部回读；远端codex/m2-room-platform已普通推送并核对f81c016，回执SHA256 7827e5f7dade6d8553b6ad8c9216540e0742cb2452fd233e1f5438e08fde2d67。

B008在模型提示构造前建立实际Session/席位身份、可见事件/投影、私有记忆、模型配置与工具授权的隔离边界；禁止提供全隐藏状态后仅靠提示词控制。主持内部子Agent只有建议，无权提交权威事件。工作区/房间/Session/席位/任务多级预算预留覆盖调用数、Token、费用、延迟、工具、子Agent、上下文及本地计算；并发硬上限、到限暂停与外部消耗不确定时保守结算必须真实验证。复用现有权限事务、模型配置和Session/恢复接缝，不更改公开API/Schema/Host API。实际提供者网络认证、最终玩家循环及生产监听仍由后续批次负责，不提前声明其PASS。

冻结前依据CODEX-AUTONOMOUS-PLANNING与SPEC-CHANGE-CONTROL-NONTRIGGERS完成内部工程安排：追加已完成B006为Session继续/恢复数据接缝依赖，补充真实SQL并发race和既有Session回归检查。允许目录、单一目标、V1范围、公开契约、固定Lua后端、许可及出口门禁保持。当前只修改三状态路径，B008实现与业务测试均NOT_RUN。

## 此前状态（保留原文）

## 当前 M2：B008 AI席位隔离与预算契约冻结

正常 PLAN v31→v32 仅冻结/启动尚未开始的 M2-B008，PLANNED→FROZEN；原生十四字段摘要 b702faf607a4d61d518301a5f696de63c6476a787bc0563d495a667b21a61faa。其余十一批、next13、tombstones及M1归档保持。Linux M2已完成7/12≈58.33%，B008业务NOT_RUN；其他平台业务NOT_RUN，最终出口由B012闭环。

B007准确最终签名源f81c016d25a27e03716a7328651880d1fa144f54通过独立业务与最终复核、Root和本地主线接收；542源码、181已接受业务证据及235最终复核证据完整回读。业务源8b1884e上单元75/75、真实SQL47/47、并发122/122、原始不变时区探针2/2、治理315/315及check/test/vet/Linux CI实际通过。最终仅三状态变化，539非状态文件按准确blob/mode/全部字节一致继承且当前明确NOT_RERUN，最终独立治理315/315、原生状态投影1/1及13项当前必需actual0。最初bf337b2的时区失败2run/1pass/1fail/0skip及后续NOT_RUN保持；仅经三文件正式范围内REPAIR与原断言不变复查闭合。专用容器、两份凭据及三个已释放副本已清理。完成档案SHA256 d6a1d737ecc5fac5b9c0087ebe67b150008f202bcbff9fe0a0b09fbff30f4a5f，1644成员全部回读；远端codex/m2-room-platform已普通推送并核对f81c016，回执SHA256 7827e5f7dade6d8553b6ad8c9216540e0742cb2452fd233e1f5438e08fde2d67。

B008在模型提示构造前建立实际Session/席位身份、可见事件/投影、私有记忆、模型配置与工具授权的隔离边界；禁止提供全隐藏状态后仅靠提示词控制。主持内部子Agent只有建议，无权提交权威事件。工作区/房间/Session/席位/任务多级预算预留覆盖调用数、Token、费用、延迟、工具、子Agent、上下文及本地计算；并发硬上限、到限暂停与外部消耗不确定时保守结算必须真实验证。复用现有权限事务、模型配置和Session/恢复接缝，不更改公开API/Schema/Host API。实际提供者网络认证、最终玩家循环及生产监听仍由后续批次负责，不提前声明其PASS。

冻结前依据CODEX-AUTONOMOUS-PLANNING与SPEC-CHANGE-CONTROL-NONTRIGGERS完成内部工程安排：追加已完成B006为Session继续/恢复数据接缝依赖，补充真实SQL并发race和既有Session回归检查。允许目录、单一目标、V1范围、公开契约、固定Lua后端、许可及出口门禁保持。当前只修改三状态路径，B008实现与业务测试均NOT_RUN。

## 此前状态（保留原文）

## 当前 M2：B007 Linux 凭据与模型配置 业务已独立验收，等待最终状态接收

正常 PLAN v30→v31 仅将 M2-B007 VERIFYING→COMPLETED；十四字段冻结摘要 769eda90cd775639e349e8a50db1efa908f23835ec6fa4e4666d785f5c914434 保持，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成 7/12≈58.33%；其他平台业务NOT_RUN，最终出口由B012闭环。

B007在12个冻结范围内新增源码/测试文件实现只读文件AES-GCM主密钥或明确一游戏临时凭据，凭据的工作区、房间、游戏、席位、所有者、期限和版本全部认证绑定；长期授权沿用现有Owner/Admin权限，参与或管理房间不自动获得长期凭据权限。配置绑定服务器批准的endpoint、model、adapter、prompt template、tool mode、test version、MC能力等级和游戏资格；旧认证、过期/撤销凭据、跨范围及准备修订失效均拒绝。默认平台路由和显式获准fallback只引用受控登记，预算配置有有限上限；当前认证、房间、席位、资格与安全确认在既有launch.ModelChecker事务中重校验，无网络或游戏状态写入。

清洁签名业务候选ed3b98c208bb18c227c6469f9f6342650dc62282实测单元71/71、专用真实PostgreSQL43/43、race114/114、治理315/315，零失败/跳过；just check、just test、go vet ./...及Linux just ci实际退出0。首次单元1项与真实SQL7项失败（含父项）及just check原样保留，五文件正式范围内REPAIR与对应完整必需重跑已关闭。原生适配器登记未引入，服务器兼容端点统一使用OpenAI-compatible协议且未改M0门禁。无物理数据库夹具的探针明确非零退出、NOT_RUN且零业务用例/跳过；外置阅读辅助错误与测试源码格式化失败均保持零业务PASS计数。生产证据在仓库外m2-b007-model-platform/BUSINESS_PRODUCER_RECEIPT.json。

本批只用受控合成模型资格记录验证登记绑定，不将实际provider资格测试、AI调用、总量预算记为已运行；这些由B008/B012完成，实际production daemon启动由B011完成。提供受控platformd类型化工厂，未新增公开HTTP/JSON Schema、Host方法、Lua后端、许可或依赖，527个此前非状态文件完整保留。最后三状态文件改变后539个非状态blob/mode必须准确一致，业务检查仅按准确来源继承NOT_RERUN。

独立验收首次在准确bf337b215539ef26c016bfd92403eb5445f9a140确认临时凭据非UTC到期时间经PostgreSQL规范UTC后AAD不一致，实际探针1退出（2 run/1 pass/1 fail/0 skip）；完整FAIL、剩余NOT_RUN与192证据成员封存不变。正式REPAIR仅改vault及两个新增回归文件，保持期限、精度、权限与旧断言；准确签名8b1884e18b3fc0a180432c679dacb595488fdbdc实测单元75/75、SQL47/47、race122/122、治理315/315及原探针逐字节复用2/2，所有冻结检查实际0。当前生产证据为BUSINESS_PRODUCER_TIMEZONE_REPAIR_RECEIPT.json，已获新的准确源独立接收；首次FAIL不给业务PASS信用。

业务候选已由Owner授权的只读独立验收代理实际重跑及源码审查，Root完整回读接收通过，未决必需项为空；证据在ROOT_INDEPENDENT_BUSINESS_RECEPTION.json。最终三状态文件接收、本地主线接收、专属资源清理、封存及普通远端推送尚未完成，需全部通过后按Owner“每批次完工提交远端”继续推进B008。

## 此前状态（保留原文）

## 当前 M2：B007 Linux 凭据与模型配置 候选等待独立验收

正常 PLAN v29→v30 仅将 M2-B007 IMPLEMENTING→VERIFYING；十四字段冻结摘要 769eda90cd775639e349e8a50db1efa908f23835ec6fa4e4666d785f5c914434 保持，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成 6/12=50%；其他平台业务NOT_RUN，最终出口由B012闭环。

B007在12个冻结范围内新增源码/测试文件实现只读文件AES-GCM主密钥或明确一游戏临时凭据，凭据的工作区、房间、游戏、席位、所有者、期限和版本全部认证绑定；长期授权沿用现有Owner/Admin权限，参与或管理房间不自动获得长期凭据权限。配置绑定服务器批准的endpoint、model、adapter、prompt template、tool mode、test version、MC能力等级和游戏资格；旧认证、过期/撤销凭据、跨范围及准备修订失效均拒绝。默认平台路由和显式获准fallback只引用受控登记，预算配置有有限上限；当前认证、房间、席位、资格与安全确认在既有launch.ModelChecker事务中重校验，无网络或游戏状态写入。

清洁签名业务候选ed3b98c208bb18c227c6469f9f6342650dc62282实测单元71/71、专用真实PostgreSQL43/43、race114/114、治理315/315，零失败/跳过；just check、just test、go vet ./...及Linux just ci实际退出0。首次单元1项与真实SQL7项失败（含父项）及just check原样保留，五文件正式范围内REPAIR与对应完整必需重跑已关闭。原生适配器登记未引入，服务器兼容端点统一使用OpenAI-compatible协议且未改M0门禁。无物理数据库夹具的探针明确非零退出、NOT_RUN且零业务用例/跳过；外置阅读辅助错误与测试源码格式化失败均保持零业务PASS计数。生产证据在仓库外m2-b007-model-platform/BUSINESS_PRODUCER_RECEIPT.json。

本批只用受控合成模型资格记录验证登记绑定，不将实际provider资格测试、AI调用、总量预算记为已运行；这些由B008/B012完成，实际production daemon启动由B011完成。提供受控platformd类型化工厂，未新增公开HTTP/JSON Schema、Host方法、Lua后端、许可或依赖，527个此前非状态文件完整保留。最后三状态文件改变后539个非状态blob/mode必须准确一致，业务检查仅按准确来源继承NOT_RERUN。

独立验收、本地主线接收、清理封存及远端推送尚未执行，B007不记完成。下一步复用Owner已授权的只读独立验收代理；Root完整接收通过后再转完成。

## 此前状态（保留原文）

## 当前 M2：B007 Linux 凭据与模型配置实现启动

正常 PLAN v28→v29 仅调整并冻结/启动尚未开始的 M2-B007，FROZEN→IMPLEMENTING；原生十四字段摘要 769eda90cd775639e349e8a50db1efa908f23835ec6fa4e4666d785f5c914434。其余十一批、next13、tombstones及M1归档保持。Linux M2已完成6/12=50%，本批业务NOT_RUN；其他平台业务NOT_RUN，最终出口由B012闭环。

B006准确最终签名源6c38575cc765a51bdf7cea5d1006ef461e04cd1d通过独立业务与最终复核、Root和本地主线接收；530源码、266业务证据、222最终复核证据完整回读。业务候选deaccb0上的单元70/70、真实SQL26/26、并发96/96、治理315/315及私密导出探针3/3实际通过；最终三状态提交527个非状态blob/mode相同并明确按准确身份继承NOT_RERUN，最终治理315/315实际通过。专用容器、两份凭据及两份释放副本已清理。完成档案SHA256 5f1f4ffa8ac270acf4cbadd93bba0f1c8a8418ad3ada881c325bd7d35c8fefff，1441成员全量回读；远端codex/m2-room-platform已普通推送并核对6c38575，回执SHA256 3352049ecade1c59a0eda2eed918f88cb3ff77404b8c23caef123382463ca65d。首次失败、正式范围内REPAIR、NOT_RUN与外置助手修正证据保持。

B007实现服务器凭据隔离、只读文件主密钥加密或一Session临时凭据、工作区/席位模型配置及完整模型认证组合绑定；访客无长期凭据授权，普通日志、提示、导出不包含原始密钥。认证绑定model/endpoint/adapter/prompt template/tool mode/test version及能力等级和游戏要求，配置变更与撤销须当前授权重校验。复用现有RoomAuthority内部身份事务和launch.ModelChecker接缝，默认拒绝未授权endpoint/adapter。实际生产监听与启动由B011负责；模型预算与调用失败完整出口仍由B008/B012负责，本批不提前关闭REQ-AI-003总体出口。

冻结前依据CODEX-AUTONOMOUS-PLANNING与SPEC-CHANGE-CONTROL-NONTRIGGERS完成内部工程安排：加入cmd/platformd/platform_model*.go类型化组合范围，加入已完成B003/B004/B005为依赖，单元与race检查覆盖既有开局/会话接缝和本批真实SQL集成。目标、V1、公开HTTP/JSON Schema、Host API、固定Lua后端、许可及出口验收门禁保持；三状态路径之外的仓库源码本次未修改。

本批实现与业务测试尚未执行；下一步按新鲜正式路由继续。只有实际触发公共契约、架构、许可、固定后端或冻结目录门禁时准备具体CHANGE交Owner决定。

## 此前状态（保留原文）

## 当前 M2：B007 凭据与模型配置契约冻结

正常 PLAN v27→v28 仅调整并冻结/启动尚未开始的 M2-B007，PLANNED→FROZEN；原生十四字段摘要 769eda90cd775639e349e8a50db1efa908f23835ec6fa4e4666d785f5c914434。其余十一批、next13、tombstones及M1归档保持。Linux M2已完成6/12=50%，本批业务NOT_RUN；其他平台业务NOT_RUN，最终出口由B012闭环。

B006准确最终签名源6c38575cc765a51bdf7cea5d1006ef461e04cd1d通过独立业务与最终复核、Root和本地主线接收；530源码、266业务证据、222最终复核证据完整回读。业务候选deaccb0上的单元70/70、真实SQL26/26、并发96/96、治理315/315及私密导出探针3/3实际通过；最终三状态提交527个非状态blob/mode相同并明确按准确身份继承NOT_RERUN，最终治理315/315实际通过。专用容器、两份凭据及两份释放副本已清理。完成档案SHA256 5f1f4ffa8ac270acf4cbadd93bba0f1c8a8418ad3ada881c325bd7d35c8fefff，1441成员全量回读；远端codex/m2-room-platform已普通推送并核对6c38575，回执SHA256 3352049ecade1c59a0eda2eed918f88cb3ff77404b8c23caef123382463ca65d。首次失败、正式范围内REPAIR、NOT_RUN与外置助手修正证据保持。

B007实现服务器凭据隔离、只读文件主密钥加密或一Session临时凭据、工作区/席位模型配置及完整模型认证组合绑定；访客无长期凭据授权，普通日志、提示、导出不包含原始密钥。认证绑定model/endpoint/adapter/prompt template/tool mode/test version及能力等级和游戏要求，配置变更与撤销须当前授权重校验。复用现有RoomAuthority内部身份事务和launch.ModelChecker接缝，默认拒绝未授权endpoint/adapter。实际生产监听与启动由B011负责；模型预算与调用失败完整出口仍由B008/B012负责，本批不提前关闭REQ-AI-003总体出口。

冻结前依据CODEX-AUTONOMOUS-PLANNING与SPEC-CHANGE-CONTROL-NONTRIGGERS完成内部工程安排：加入cmd/platformd/platform_model*.go类型化组合范围，加入已完成B003/B004/B005为依赖，单元与race检查覆盖既有开局/会话接缝和本批真实SQL集成。目标、V1、公开HTTP/JSON Schema、Host API、固定Lua后端、许可及出口验收门禁保持；三状态路径之外的仓库源码本次未修改。

本批实现与业务测试尚未执行；下一步按新鲜正式路由继续。只有实际触发公共契约、架构、许可、固定后端或冻结目录门禁时准备具体CHANGE交Owner决定。

## 此前状态（保留原文）

## 当前 M2：B006 Linux 持久任务与 Continuation 业务已独立验收，等待最终状态接收

正常 PLAN v26→v27 仅将 M2-B006 VERIFYING→COMPLETED；十四字段冻结摘要 ecc2aaa2407c856c67782016d521acaec564f65d5dd3cd8eb3b9263e6e8ab329 保持，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成 6/12=50%；Windows/macOS业务NOT_RUN，最终13项证明由B012闭环。

B006在22个冻结范围内源码/测试文件中实现已提交Task/Continuation/Outbox的有界持久任务派发、worker授权与租约、Schema结果校验、结果保存、标准resume_continuation系统命令和现有单一Actor回传。来源以同一事务中的完整不可变历史为准；外部处理期间真实独立SQL NOWAIT探针确认权威行未被锁住。租户、Session、图、配置、当前令牌及版本均重新校验，普通玩家不能选择系统回调。多个Continuation保持原顺序，重复完成只返回匹配的原始回执，完成后的崩溃通过相同Actor与历史恢复；取消、过期、撤销、旧版本以及有界重试均有真实PostgreSQL证据。保护性句柄和稳定错误阻止普通日志/JSON暴露原始任务状态、凭据及私密席位值。

清洁签名业务候选19a1ff3f7a1f727cc57528ff3ded2326b756d140实测单元70/70、专用真实PostgreSQL26/26、race96/96、治理315/315，零失败/跳过；just check、just test、go vet ./...及Linux just ci实际退出0。首次数据库20项失败和just check导入边界失败原样保留，已通过正式REPAIR内两文件修复及对应完整必需重跑关闭。沙箱路由写权限失败、外置夹具目录错误及只读诊断失败均保存实际结果，未计业务PASS；无物理数据库夹具的探针明确非零退出、NOT_RUN且零业务用例/跳过。生产证据在仓库外m2-b006-continuation-platform/BUSINESS_PRODUCER_RECEIPT.json。

提供有界workerd内部类型化工厂和platformd现有Coordinator连接；实际生产守护进程启动仍由B011承担，未在本批提前记为已运行。采用获准合成任务处理器、真实已安装Lua和PostgreSQL验证，没有新增公开HTTP/JSON Schema、Host方法、Lua后端、许可证或依赖，也未修改旧批次测试断言。最终三状态文件变化后上述业务检查明确NOT_RERUN，只能按全部527个非状态blob/mode准确一致继承。

业务候选已由Owner授权的只读独立验收代理实际重跑及源码审查，Root完整回读接收通过，未决必需项为空；证据在ROOT_INDEPENDENT_BUSINESS_RECEPTION.json。最终三文件状态接收、本地主线接收、专属资源清理、封存及普通远端推送尚未完成，需全部通过后按Owner“每批次完工提交远端”继续推进B007。

## 此前状态（保留原文）

## 当前 M2：B006 Linux 持久任务与 Continuation 候选等待独立验收

正常 PLAN v25→v26 仅将 M2-B006 IMPLEMENTING→VERIFYING；十四字段冻结摘要 ecc2aaa2407c856c67782016d521acaec564f65d5dd3cd8eb3b9263e6e8ab329 保持，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成 5/12≈41.67%；Windows/macOS业务NOT_RUN，最终13项证明由B012闭环。

B006在22个冻结范围内源码/测试文件中实现已提交Task/Continuation/Outbox的有界持久任务派发、worker授权与租约、Schema结果校验、结果保存、标准resume_continuation系统命令和现有单一Actor回传。来源以同一事务中的完整不可变历史为准；外部处理期间真实独立SQL NOWAIT探针确认权威行未被锁住。租户、Session、图、配置、当前令牌及版本均重新校验，普通玩家不能选择系统回调。多个Continuation保持原顺序，重复完成只返回匹配的原始回执，完成后的崩溃通过相同Actor与历史恢复；取消、过期、撤销、旧版本以及有界重试均有真实PostgreSQL证据。保护性句柄和稳定错误阻止普通日志/JSON暴露原始任务状态、凭据及私密席位值。

清洁签名业务候选19a1ff3f7a1f727cc57528ff3ded2326b756d140实测单元70/70、专用真实PostgreSQL26/26、race96/96、治理315/315，零失败/跳过；just check、just test、go vet ./...及Linux just ci实际退出0。首次数据库20项失败和just check导入边界失败原样保留，已通过正式REPAIR内两文件修复及对应完整必需重跑关闭。沙箱路由写权限失败、外置夹具目录错误及只读诊断失败均保存实际结果，未计业务PASS；无物理数据库夹具的探针明确非零退出、NOT_RUN且零业务用例/跳过。生产证据在仓库外m2-b006-continuation-platform/BUSINESS_PRODUCER_RECEIPT.json。

提供有界workerd内部类型化工厂和platformd现有Coordinator连接；实际生产守护进程启动仍由B011承担，未在本批提前记为已运行。采用获准合成任务处理器、真实已安装Lua和PostgreSQL验证，没有新增公开HTTP/JSON Schema、Host方法、Lua后端、许可证或依赖，也未修改旧批次测试断言。最终三状态文件变化后上述业务检查明确NOT_RERUN，只能按全部527个非状态blob/mode准确一致继承。

独立验收、本地主线接收、清理封存及远端推送尚未执行，B006不记完成。下一步复用Owner已授权的只读独立验收代理，准确验收正式ACCEPT路由、签名源码、冻结边界和专用数据库，Root完整接收通过后再转完成。

## 此前状态（保留原文）

## 当前 M2：B006 Linux 外部任务与 Continuation 实现启动

正常 PLAN v24→v25 仅调整并冻结/启动尚未开始的 M2-B006，FROZEN→IMPLEMENTING；原生十四字段摘要 ecc2aaa2407c856c67782016d521acaec564f65d5dd3cd8eb3b9263e6e8ab329。其余十一批、next13、tombstones及M1归档保持。Linux M2已完成5/12≈41.67%，本批业务NOT_RUN；其他平台业务NOT_RUN，最终出口由B012闭环。

B005准确最终签名源9ea7bca9ef962e88aa7cc6cc5f4e3aadb185085f通过独立最终复核、Root和本地主线接收；511源码、277业务证据、225最终复核证据完整回读。业务候选5005114上的单元61/61、真实SQL18/18、并发79/79和额外导出探针3/3通过，最终三状态提交508非状态blob/mode相同并明确按准确身份继承NOT_RERUN；最终治理315/315实际通过。专用容器、两份凭据及两份释放副本已清理。完成档案SHA256 1b1718541160df2288b58d350ec06439af027a74204ecd9a8542c0c1478f56af，1527成员全量回读；远端codex/m2-room-platform已普通推送并核对9ea7bca，回执SHA256 f5d14a0ed6f2d72252ac8505d5c842b7f256d60f6745e266be39442f495fcaa4。此前Owner批准的两文件认证补修、原失败、NOT_RUN和外置助手修正证据保持。

B006复用已提交的Task、Continuation和Outbox记录；workerd在游戏事务外执行受控外部任务，完成结果以当前租户/Session/任务令牌/状态版本验证后的系统命令进入既有单写Actor，并调用既有标准resume_continuation。任务租约、结果保存、重复完成和提交前后崩溃提供有界重试及可重复恢复证据；旧协程栈不持久化，worker不写游戏权威状态，不在Lua或游戏事务等待外部网络。

冻结前依据CODEX-AUTONOMOUS-PLANNING与SPEC-CHANGE-CONTROL-NONTRIGGERS完成内部工程安排：增加command/envelope.go、command/native*.go、launch/session_bridge_runtime.go和launch/continuation*.go四个接缝范围，增加已完成B005为依赖，并将相关内部命令/开局/会话及两daemon组合纳入单元和并发验证。目标、V1、公开HTTP/JSON Schema、Host API、固定Lua后端、许可及出口验收门禁保持；本批不新增公开接口或改变标准Host方法。三状态路径之外的仓库源码本次未修改。

本批实现与业务测试尚未执行；下一步按新鲜正式路由继续。仅在实际触发公共契约、架构、许可、固定后端或冻结目录门禁时准备具体CHANGE交Owner决定。

## 此前状态（保留原文）

## 当前 M2：B006 外部任务与 Continuation 契约冻结

正常 PLAN v23→v24 仅调整并冻结/启动尚未开始的 M2-B006，PLANNED→FROZEN；原生十四字段摘要 ecc2aaa2407c856c67782016d521acaec564f65d5dd3cd8eb3b9263e6e8ab329。其余十一批、next13、tombstones及M1归档保持。Linux M2已完成5/12≈41.67%，本批业务NOT_RUN；其他平台业务NOT_RUN，最终出口由B012闭环。

B005准确最终签名源9ea7bca9ef962e88aa7cc6cc5f4e3aadb185085f通过独立最终复核、Root和本地主线接收；511源码、277业务证据、225最终复核证据完整回读。业务候选5005114上的单元61/61、真实SQL18/18、并发79/79和额外导出探针3/3通过，最终三状态提交508非状态blob/mode相同并明确按准确身份继承NOT_RERUN；最终治理315/315实际通过。专用容器、两份凭据及两份释放副本已清理。完成档案SHA256 1b1718541160df2288b58d350ec06439af027a74204ecd9a8542c0c1478f56af，1527成员全量回读；远端codex/m2-room-platform已普通推送并核对9ea7bca，回执SHA256 f5d14a0ed6f2d72252ac8505d5c842b7f256d60f6745e266be39442f495fcaa4。此前Owner批准的两文件认证补修、原失败、NOT_RUN和外置助手修正证据保持。

B006复用已提交的Task、Continuation和Outbox记录；workerd在游戏事务外执行受控外部任务，完成结果以当前租户/Session/任务令牌/状态版本验证后的系统命令进入既有单写Actor，并调用既有标准resume_continuation。任务租约、结果保存、重复完成和提交前后崩溃提供有界重试及可重复恢复证据；旧协程栈不持久化，worker不写游戏权威状态，不在Lua或游戏事务等待外部网络。

冻结前依据CODEX-AUTONOMOUS-PLANNING与SPEC-CHANGE-CONTROL-NONTRIGGERS完成内部工程安排：增加command/envelope.go、command/native*.go、launch/session_bridge_runtime.go和launch/continuation*.go四个接缝范围，增加已完成B005为依赖，并将相关内部命令/开局/会话及两daemon组合纳入单元和并发验证。目标、V1、公开HTTP/JSON Schema、Host API、固定Lua后端、许可及出口验收门禁保持；本批不新增公开接口或改变标准Host方法。三状态路径之外的仓库源码本次未修改。

本批实现与业务测试尚未执行；下一步按新鲜正式路由继续。仅在实际触发公共契约、架构、许可、固定后端或冻结目录门禁时准备具体CHANGE交Owner决定。

## 此前状态（保留原文）

## 当前 M2：B005 Linux 玩家会话业务独立验收通过

正常 PLAN v22→v23 仅将 M2-B005 VERIFYING→COMPLETED；十四字段冻结摘要 2e84bfc2bd8be65225903896c43c56010dfd14b8c91a64622dac9db538f7a077 保持，其余十一批、next13、tombstones及M1归档保持。M2批次状态完成5/12≈41.67%，B006及后续仍PLANNED；本次仅Linux，Windows/macOS业务NOT_RUN，最终13项证明由B012闭环。

B005将实际认证身份、当前参与者及人工席位连接到既有单写SessionActor、权威命令事务及私密实时消息。独立验证版本/幂等、当前身份/策略重检、私密视图/游标/待处理动作恢复、冷恢复和已提交命令不重执行。恢复点返回获准元数据，导出分页按当前申请人策略过滤，其他席位私密状态和原始Cookie/CSRF不进入普通输出；未入座管理员不获得观察权、内容哈希不赋予跨租户权限、断线不自动转AI。队列/邮箱保持有界；消息请求取消仅拒绝该次交付，仍有效的连接保持，真实权限失效仍拒绝并释放。

独立候选5005114a50d59ad71a758056fddad2236e7309f0及511个源码成员完整回读匹配，21个业务/测试文件在冻结目录内。Owner批准 CHANGE-M2-B005-LIVE-AUTH-INSPECTION 的 room.go/room_test.go 两叶子等于批准草案字节；内部权限复核复用原认证事务和寿命规则，用户Do限流/幂等不变。公开Doc/Schema、Host API、依赖和许可未改，后续生产UI、外部模型网关及最终混合玩家闭环未提前验收。

独立实测玩家会话单元61/61、专用真实PostgreSQL18/18、并发79/79和治理315/315均通过，零失败/跳过；准确ACCEPT路由、原生冻结转换、前向签名、just check/test、go vet ./...和Linux just ci实际0。Root已完整回读独立证据、源码及实际日志，未决finding和必需检查为空。原失败、NOT_RUN、取消修复的有效行为红对照及无效第一版外部探针保留，均无业务通过计数；缺少物理fixture证明非零退出。验收代理已释放副本与数据库租约，供Root接收清理。

独立业务接收回执为仓库外m2-b005-session-platform/ROOT_INDEPENDENT_BUSINESS_RECEPTION.json。此状态提交仍须准确最终签名源复核、本地主线接收、专用数据库/两份凭据/已释放副本清理及档案完整回读；这些后续步骤尚未执行。接收清理完成后按Owner要求普通推送已确认归属的远端分支，再继续B006正常PLAN。

## 此前状态（保留原文）

## 当前 M2：B005 Linux 玩家会话候选等待独立验收

正常 PLAN v21→v22 仅将 M2-B005 IMPLEMENTING→VERIFYING；十四字段冻结摘要 2e84bfc2bd8be65225903896c43c56010dfd14b8c91a64622dac9db538f7a077 保持，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成仍为4/12≈33.33%；本批未计完成，Windows/macOS业务NOT_RUN，最终13项证明由B012闭环。

B005通过21个冻结范围内源码/测试文件，连接真实平台Cookie、当前房间参与者与人工席位到同一权威SessionActor、命令事务和私密实时队列。命令保留版本、完整载荷及幂等检查；每次执行与交付重新检查服务端身份和包策略。恢复游标、私密视图与待处理动作，重连不重复已提交命令，不将断线人工席位自动交AI。恢复点仅返回当前获准的元数据；四种内部导出策略默认拒绝并按当前权限过滤，原始凭据与其他席位私密状态不进入普通输出；邮箱、订阅和导出分页均有界。使用既有Go/Lua、HostAPI、持久化和历史恢复；未新增公开HTTP/JSON Schema或生产UI接口，后续模型网关与最终混合玩家闭环未提前验收。

Owner批准的 CHANGE-M2-B005-LIVE-AUTH-INSPECTION 精确两文件补修保持原草案字节：internal/platform/auth/room.go 和 room_test.go。内部当前权限检查共享原有Cookie/会话寿命、参与者、CSRF和同一事务；用户入口Do限流及幂等回执保持。原批准签名源码2c54c7358e0dc441773756b76ca056f177f847df实测单元37/37、并发37/37；后续两文件blob完全相同，按准确身份继承并标NOT_RERUN。授权、具体补丁和验证保存于仓库外m2-b005-session-platform/change-live-auth-inspection。

清洁签名生产方候选fb558f746a228dca845b4b012fb54d60b80b4b75实测会话单元61/61、专用真实PostgreSQL18/18、并发79/79，均零失败/跳过；just check、just test、go vet ./...及Linux just ci实际退出0。未使用导入编译失败、测试误用大厅改角接口、沙箱拒绝本地监听以及消息请求取消误释放有效连接的全部原失败保留，并由相应必需检查重跑关闭。缺少物理数据库证明的探针明确NOT_RUN、非零退出且无业务通过计数。生产证据为仓库外m2-b005-session-platform/BUSINESS_PRODUCER_RECEIPT.json。

独立验收、本地主线接收、清理封存尚未执行，B005不记完成。下一步复用Owner已授权的单一只读独立验收代理，准确核对签名源码、正式ACCEPT路由、两文件批准范围及专用数据库；全部接收通过后方可转完成，并按Owner要求普通推送到已确认归属的远端分支。

## 此前状态（保留原文）

## 当前 M2：B005 Linux 玩家会话连接实现启动

正常 PLAN v20→v21 仅将 M2-B005 FROZEN→IMPLEMENTING；原生十四字段冻结摘要 2e84bfc2bd8be65225903896c43c56010dfd14b8c91a64622dac9db538f7a077，其余十一批、next13、tombstones及M1归档保持。Linux M2已完成4/12≈33.33%，本批业务尚未验收；Windows/macOS业务NOT_RUN，最终13项证明由B012闭环。

B004准确最终签名源be946f909a0ca6c7604bec5f75eaf0154edb7108与494源码成员已通过独立最终复核、本地主线接收及专用数据库、两份凭据、两份释放副本清理。独立实测检查点8/8、开局单元24/24、真实SQL30/30、开局并发54/54、边界探针6/6和治理315/315均零失败/跳过；最终状态提交仅三状态文件变化，491非状态blob/mode一致，业务实测证据按准确身份继承并标NOT_RERUN。完成接收档案SHA256 ae049ba7ca7143f51621faf5c6050c5a90be6fecefb498e381eb2b2c1e417357，1473成员全部完整回读；仓库外m2-b004-linux-final/COMPLETION_RECEIPT.json SHA256 66c8d52600461c44fa9ae458f209bae9a893ebf1523e26d35dd76e06eefd5eba。Owner批准的检查点两文件补修、全部原失败和NOT_RUN保持，B004已按Owner“每批次完工提交至远端”授权，以普通推送发布至github.com/zyc14588/TRPG_PLATFORM的codex/m2-room-platform，准确远端提交be946f909a0ca6c7604bec5f75eaf0154edb7108；远端接收回执仓库外m2-b004-remote-reception-be946f9/REMOTE_RECEPTION.json，SHA256 d8484e3bba3d46f7289e78b233eecfce197cee52c9872de883bdd9a3076b8a92。

B005保持既有目标、需求、机器契约、验收和停止条件；在尚未开始/未冻结时按正式PLAN补齐四个内部认证/会话连接接缝及对应单元和并发验证，防止采用伪玩家身份或创建第二Actor。精确追加范围为internal/session/command/envelope.go、native*.go、internal/platform/launch/actor.go和session_bridge*.go；未改公开HTTP/JSON Schema、Host API、V1、架构或许可，已完成四批契约保持。工程依据CODEX-AUTONOMOUS-PLANNING和SPEC-CHANGE-CONTROL-NONTRIGGERS，范围在冻结前形成并记录于本次唯一十四字段契约。B005连接平台认证到现有权威命令、私密实时消息、断线恢复、恢复点和按当前权限过滤的导出。保留版本与幂等检查，每次交付复核服务端身份与当前可见范围；断线恢复游标、私密视图与待处理动作，不重复已提交命令，不自动将人工席位交AI。当前申请者无权的数据、原始令牌和密钥不进入普通日志与导出；队列和邮箱保持有界。B002和B004依赖已完成，后续模型网关、UI、最终混合玩家闭环未提前验收。

本批源码与业务验证尚未执行，不计新增完成批次；下一步按对应新鲜正式路由继续。若需要变更公开API/Schema、Host API、架构或超出冻结目录，立即停止并准备具体CHANGE交Owner决定。

## 此前状态（保留原文）

## 当前 M2：B005 玩家会话连接契约冻结

正常 PLAN v19→v20 仅将 M2-B005 PLANNED→FROZEN；原生十四字段冻结摘要 2e84bfc2bd8be65225903896c43c56010dfd14b8c91a64622dac9db538f7a077，其余十一批、next13、tombstones及M1归档保持。Linux M2已完成4/12≈33.33%，本批业务尚未验收；Windows/macOS业务NOT_RUN，最终13项证明由B012闭环。

B004准确最终签名源be946f909a0ca6c7604bec5f75eaf0154edb7108与494源码成员已通过独立最终复核、本地主线接收及专用数据库、两份凭据、两份释放副本清理。独立实测检查点8/8、开局单元24/24、真实SQL30/30、开局并发54/54、边界探针6/6和治理315/315均零失败/跳过；最终状态提交仅三状态文件变化，491非状态blob/mode一致，业务实测证据按准确身份继承并标NOT_RERUN。完成接收档案SHA256 ae049ba7ca7143f51621faf5c6050c5a90be6fecefb498e381eb2b2c1e417357，1473成员全部完整回读；仓库外m2-b004-linux-final/COMPLETION_RECEIPT.json SHA256 66c8d52600461c44fa9ae458f209bae9a893ebf1523e26d35dd76e06eefd5eba。Owner批准的检查点两文件补修、全部原失败和NOT_RUN保持，B004已按Owner“每批次完工提交至远端”授权，以普通推送发布至github.com/zyc14588/TRPG_PLATFORM的codex/m2-room-platform，准确远端提交be946f909a0ca6c7604bec5f75eaf0154edb7108；远端接收回执仓库外m2-b004-remote-reception-be946f9/REMOTE_RECEPTION.json，SHA256 d8484e3bba3d46f7289e78b233eecfce197cee52c9872de883bdd9a3076b8a92。

B005保持既有目标、需求、机器契约、验收和停止条件；在尚未开始/未冻结时按正式PLAN补齐四个内部认证/会话连接接缝及对应单元和并发验证，防止采用伪玩家身份或创建第二Actor。精确追加范围为internal/session/command/envelope.go、native*.go、internal/platform/launch/actor.go和session_bridge*.go；未改公开HTTP/JSON Schema、Host API、V1、架构或许可，已完成四批契约保持。工程依据CODEX-AUTONOMOUS-PLANNING和SPEC-CHANGE-CONTROL-NONTRIGGERS，范围在冻结前形成并记录于本次唯一十四字段契约。B005连接平台认证到现有权威命令、私密实时消息、断线恢复、恢复点和按当前权限过滤的导出。保留版本与幂等检查，每次交付复核服务端身份与当前可见范围；断线恢复游标、私密视图与待处理动作，不重复已提交命令，不自动将人工席位交AI。当前申请者无权的数据、原始令牌和密钥不进入普通日志与导出；队列和邮箱保持有界。B002和B004依赖已完成，后续模型网关、UI、最终混合玩家闭环未提前验收。

本批源码与业务验证尚未执行，不计新增完成批次；下一步按对应新鲜正式路由继续。若需要变更公开API/Schema、Host API、架构或超出冻结目录，立即停止并准备具体CHANGE交Owner决定。

## 此前状态（保留原文）

## 当前 M2：B004 Linux 开局业务独立验收通过

正常 PLAN v18→v19 仅将 M2-B004 VERIFYING→COMPLETED；原生十四字段冻结摘要 b0169aa4fa66a5eadefa8022478f56d06e2a313c067550c725ee5324e64d7bcb 保持，其余十一批、next13、tombstones及M1归档保持。M2批次状态完成4/12≈33.33%，B005及后续仍PLANNED；本次仅Linux，Windows/macOS业务NOT_RUN，最终13项证明仍由B012闭环。

B004开局门禁、同事务房间至权威Session绑定、私有幂等结果、未知提交恢复及单Actor恢复通过独立验收：精确包授权/信任、当前主持权限、参与者内容同意、席位、准备、安全边界和模型证明均先检查，缺失条件拒绝且无部分Session；并发重试收敛同一创建。独立候选1bf5c2d8393043a08ff649c6698054df74f7f3e2及494个源码成员完整回读匹配。15个新增业务/测试文件在冻结目录内；Owner批准 CHANGE-M2-B004-CHECKPOINT-SEAL-ALIAS 的 checkpoint.go/checkpoint_test.go 两叶子例外等于批准草案字节；除正常三状态文件和该两叶子补修外旧源码保持。公开Doc/Schema、Host API、依赖和许可未改。

独立实测检查点race 8/8、开局单元24/24、专用真实PostgreSQL30/30、开局并发54/54和治理315/315全部实际通过，零失败/跳过；fresh ACCEPT/原生冻结校验、前向签名、独立边界探针、just check/test、go vet ./...和Linux just ci实际0。Root已回读全部独立证据和源码，未决finding及必需检查为空；验收代理已释放副本及数据库租约，资源保留交Root接收清理。原全部失败、NOT_RUN及辅助审计修正证据保留。明确缺fixture非零退出且不计业务通过。

独立业务接收回执：仓库外m2-b004-launch-platform/ROOT_INDEPENDENT_BUSINESS_RECEPTION.json；生产方、Owner授权与独立验收证据均保留。此状态提交随后仍需准确最终签名源复核、本地主线接收、专用数据库/两份凭据/已释放副本清理和档案完整回读；这些后续步骤尚未执行，不提前声称已通过。后续席位传输、生产UI、外部模型网关和最终混合玩家闭环未提前验收；接收清理完成后继续B005正常PLAN。

## 此前状态（保留原文）

## 当前 M2：B004 Linux 开局候选等待独立验收

正常 PLAN v17→v18 仅将 M2-B004 IMPLEMENTING→VERIFYING；原生十四字段冻结摘要 b0169aa4fa66a5eadefa8022478f56d06e2a313c067550c725ee5324e64d7bcb 保持，其余十一批、next13、tombstones及M1归档保持。B001/B002/B003已完成，当前完成仍3/12=25%；本次仅Linux，Windows/macOS业务NOT_RUN，最终13项证明仍由B012闭环。

B004增加15个允许范围内源码/测试文件：复用原有认证、房间、精确包校验与SessionActor，在同一房间事务中检查当前主持权限、包授权/信任、内容同意、必需席位、准备及安全边界，绑定独立Session并保存私有幂等结果；任一条件缺失均拒绝且不留下部分Session。并发启动保持一个权威创建与一个Actor恢复。未提前验收后续席位传输、外部模型网关、生产UI或最终混合玩家闭环；模型证明缺失仍拒绝启动，未新增公开HTTP/JSON/Host API或依赖。

Owner批准 CHANGE-M2-B004-CHECKPOINT-SEAL-ALIAS 的两叶子路径范围例外已实施：仅 internal/luaruntime/checkpoint/checkpoint.go 和 checkpoint_test.go 改为从全新对象解码封存字节，避免调用方共享map被写入；规范化字节、哈希、公开接口及校验/限额保持，M1历史归档不改。新引用隔离测试先在旧实现实际失败；补修后检查点8/8和并发封存通过。授权、具体补丁及原竞态证据均保存在仓库外m2-b004-launch-platform/change-checkpoint-seal-alias。

清洁签名生产方候选4d26990f9430acc0c3ddc9ed343d7ece85e483cf实际验证：开局单元24/24、专用真实PostgreSQL30/30、并发54/54，均零失败/跳过；just check、just test、go vet ./...及Linux just ci实际退出0。ACC-M2-B004-001至005及全部原失败保留；缺少专用数据库的探针明确NOT_RUN、非零退出且无业务通过计数。具体生产证据：仓库外m2-b004-launch-platform/BUSINESS_PRODUCER_RECEIPT_4D26990.json。

独立验收、本地主线接收、清理封存尚未执行，B004不记完成。下一步复用Owner已授权的单一只读独立验收代理，准确核对签名源码、合法ACCEPT路由、两文件批准范围及专用数据库；全部接收通过后方可正常转换为完成。

## 此前状态（保留原文）

## 当前 M2：B004 Linux 开局服务实现启动

正常 PLAN v16→v17 仅将 M2-B004 FROZEN→IMPLEMENTING；原生十四字段冻结摘要 b0169aa4fa66a5eadefa8022478f56d06e2a313c067550c725ee5324e64d7bcb，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成3/12=25%；Windows/macOS业务NOT_RUN，最终13项证明由B012闭环。

B003最终签名候选2fc2d21651c871fc2f14a19cec5dad56e980cfc1及479个源码成员已完成精确独立复核与本地主线接收，独立实测91单元/31真实SQL/122并发/4边界探针及315治理检查通过，零失败/跳过；最终状态提交315治理检查新实测通过，业务通过476个非状态blob/mode一致性继承原实测而未声称重跑。专用数据库、两份凭据和两份已释放副本已精确清理，1382成员Linux完成接收档案完整回读通过。档案SHA256 82c1749f2700deb85eceb16453d89e93b7a986ecb4bf856a02d1109fd0a95064，仓库外m2-b003-linux-final/COMPLETION_RECEIPT.json。未执行M2远端操作，原失败及NOT_RUN记录保持。

B004保持既有批次目标、范围、要求、验收和测试：在内部开局服务及允许的存储/组合接缝复用现有认证、房间、包验证与单一有界SessionActor；先验证精确包依赖/信任、参与者内容同意、席位、权限、必需准备、模型能力与安全边界，任一缺失均拒绝且不得留下部分Session。房间至Session绑定以事务和幂等方式完成。生产入口不新增公开JSON或HTTP契约；后续席位、模型网关、UI和最终混合玩家闭环未提前验收，相关缺失仍拒绝启动。

B004业务验证及独立验收尚未执行，不计新增完成批次。下一步仅依对应新鲜正式路由；若出现真实公共契约/架构/范围或验收门禁变更，先保存具体CHANGE草案并等待Owner决定。

## 此前状态（保留原文）

## 当前 M2：B004 开局条件契约冻结

正常 PLAN v15→v16 仅将 M2-B004 PLANNED→FROZEN；原生十四字段冻结摘要 b0169aa4fa66a5eadefa8022478f56d06e2a313c067550c725ee5324e64d7bcb，其余十一批、next13、tombstones及M1归档保持。当前Linux M2完成3/12=25%；Windows/macOS业务NOT_RUN，最终13项证明由B012闭环。

B003最终签名候选2fc2d21651c871fc2f14a19cec5dad56e980cfc1及479个源码成员已完成精确独立复核与本地主线接收，独立实测91单元/31真实SQL/122并发/4边界探针及315治理检查通过，零失败/跳过；最终状态提交315治理检查新实测通过，业务通过476个非状态blob/mode一致性继承原实测而未声称重跑。专用数据库、两份凭据和两份已释放副本已精确清理，1382成员Linux完成接收档案完整回读通过。档案SHA256 82c1749f2700deb85eceb16453d89e93b7a986ecb4bf856a02d1109fd0a95064，仓库外m2-b003-linux-final/COMPLETION_RECEIPT.json。未执行M2远端操作，原失败及NOT_RUN记录保持。

B004保持既有批次目标、范围、要求、验收和测试：在内部开局服务及允许的存储/组合接缝复用现有认证、房间、包验证与单一有界SessionActor；先验证精确包依赖/信任、参与者内容同意、席位、权限、必需准备、模型能力与安全边界，任一缺失均拒绝且不得留下部分Session。房间至Session绑定以事务和幂等方式完成。生产入口不新增公开JSON或HTTP契约；后续席位、模型网关、UI和最终混合玩家闭环未提前验收，相关缺失仍拒绝启动。

B004业务验证及独立验收尚未执行，不计新增完成批次。下一步仅依对应新鲜正式路由；若出现真实公共契约/架构/范围或验收门禁变更，先保存具体CHANGE草案并等待Owner决定。

## 此前状态（保留原文）

## 当前 M2：B003 Linux 房间业务独立验收通过

正常 PLAN v14→v15 仅将 M2-B003 VERIFYING→COMPLETED；原生十四字段冻结摘要 ac0127370b92f8d23433bec1dd28c7b9f97b5ef13b00f2874a27a2cd7071c854 保持，其余十一批、next13、tombstones及M1归档保持。M2批次状态完成3/12=25%，B004及后续批次仍PLANNED；本次仅Linux，Windows/macOS业务NOT_RUN，最终13项证明仍由B012闭环。

已批准 CHANGE-M2-ROOM-API-V1 与 CHANGE-M2-ROOM-ACCOUNT-NICKNAME-V1 的私人房间、角色分权、邀请/审批、同事务可信游客入场、幂等恢复及128/80昵称兼容已独立验收。独立候选c6b1ec18bd70ab56534bcee993b8e1b703a38d7a及479个源码成员全部回读匹配，454个旧文件blob/mode保持，20个业务新增文件处于允许范围。既有B001/B002业务源和认证契约保持；当前公共Doc/Schema与批准的R1提交9a56cd7一致。

独立实测单元/接口91/91、专用真实PostgreSQL31/31、并发122/122、projectctl315/315及独立边界探针4/4均通过，零失败/跳过；fresh ACCEPT阅读/检查、七个新增签名的既有信任验签、just check、just test、go vet ./...和Linux just ci实际0。缺fixture负向探针明确NOT_RUN且非零。独立覆盖未知提交后的管理撤权/成员恢复不复权及Unicode128/80边界；Root已回读全部独立证据成员，未决必需finding及未完成必需检查为空。原环境、测试准备和辅助审计失误的失败记录及原断言保持。

独立业务接收回执为仓库外m2-b003-room-platform/ROOT_INDEPENDENT_BUSINESS_RECEPTION.json；生产方与独立验收源码、命令、hash及释放记录均保留。此正常状态提交随后仍需准确签名源复核、本地主线接收、专用数据库/凭据/已释放副本清理与完整封存，不提前声称这些后续步骤已执行。席位与私密视图授权、真实启动、模型及最终玩家闭环仍由后续批次完成；接收清理完成后继续B004正式PLAN。

## 此前状态（保留原文）

## 当前 M2：B003 Linux 房间候选等待独立验收

正常 PLAN v13→v14 仅将 M2-B003 IMPLEMENTING→VERIFYING；原生十四字段冻结摘要 ac0127370b92f8d23433bec1dd28c7b9f97b5ef13b00f2874a27a2cd7071c854 保持，其余十一批、next13、tombstones及M1归档保持。B001/B002已完成，当前完成仍2/12≈16.67%；本次仅Linux，Windows/macOS业务NOT_RUN，最终13项证明仍由B012闭环。

已批准 CHANGE-M2-ROOM-API-V1 与 CHANGE-M2-ROOM-ACCOUNT-NICKNAME-V1 已落实：私人房间、房间角色、邀请/审批、同事务可信游客入场、幂等结果恢复；账户昵称128、游客名与房间名80。B003增加20个允许范围内文件，原B001/B002业务源保持；公共Doc/Schema与已批准R1提交9a56cd7一致。管理、主持及后续席位控制分别授权；邀请只授予入场资格。真实启动、席位控制和最终模型检查仍由后续批次负责。

清洁签名候选9e11ac473c34198901ac2fdfcc771386f7500ef9生产方验证实际通过：单元/接口91/91、专用真实PostgreSQL31/31、并发122/122，均零跳过；just check、just test、go vet ./...及Linux just ci实际退出0。缺少专用数据库的负向探针明确NOT_RUN并非零退出。ACC-M2-B003-001至004的修复、最初环境与测试准备失败及原断言均保留；具体证据见仓库外m2-b003-room-platform/BUSINESS_PRODUCER_RECEIPT_9E11AC4.json。

独立验收尚未运行，不记录B003完成。下一步复用Owner已授权的单一只读独立验收代理，在准确签名源码、合法ACCEPT路由和独占专用数据库上复核；验收、签名源接收、主线接收及清理封存均通过后方可完成。

## 此前状态（保留原文）

## 当前 M2：B003 Linux 房间实现启动

正常 PLAN v12→v13 仅将 M2-B003 FROZEN→IMPLEMENTING；原生十四字段冻结摘要 ac0127370b92f8d23433bec1dd28c7b9f97b5ef13b00f2874a27a2cd7071c854 保持，其余十一批、next13、tombstones及M1归档保持。B001/B002已完成，当前完成仍2/12≈16.67%；本次仅Linux，Windows/macOS业务NOT_RUN，最终13项证明仍由B012闭环。

按已批准 CHANGE-M2-ROOM-API-V1，在九类允许的新文件内实现私人房间、邀请/审批和可信同事务入场；既有B001/B002源及公开认证契约保持。房间管理、主持和后续席位控制分别授权，邀请只授予入场申请资格。真实启动、席位与模型检查仍由后续批次负责。

治理登记da25997已完成独立接收、本地主线接收、临时副本清理与753成员封存。B003业务尚未验收通过；原失败、NOT_RUN及辅助程序更正记录保留。下一步按当前正式路由执行对应阶段，只有真实检查和独立验收通过才记录业务完成。

## 此前状态（保留原文）

## 当前 M2：B003 房间契约冻结

正常 PLAN v11→v12 仅将 M2-B003 PLANNED→FROZEN；原生十四字段冻结摘要 ac0127370b92f8d23433bec1dd28c7b9f97b5ef13b00f2874a27a2cd7071c854 保持，其余十一批、next13、tombstones及M1归档保持。B001/B002已完成，当前完成仍2/12≈16.67%；本次仅Linux，Windows/macOS业务NOT_RUN，最终13项证明仍由B012闭环。

按已批准 CHANGE-M2-ROOM-API-V1，在九类允许的新文件内实现私人房间、邀请/审批和可信同事务入场；既有B001/B002源及公开认证契约保持。房间管理、主持和后续席位控制分别授权，邀请只授予入场申请资格。真实启动、席位与模型检查仍由后续批次负责。

治理登记da25997已完成独立接收、本地主线接收、临时副本清理与753成员封存。B003业务尚未验收通过；原失败、NOT_RUN及辅助程序更正记录保留。下一步按当前正式路由执行对应阶段，只有真实检查和独立验收通过才记录业务完成。

## 此前状态（保留原文）

## 当前 M2：B003 房间契约已批准，准备冻结

Owner 已批准 CHANGE-M2-ROOM-API-V1。正常 PLAN v10→v11 仅采纳未启动 B003 的十四字段契约，仍为 PLANNED；其余十一批、已完成 B001/B002 的冻结摘要、next13、tombstones、M1归档和全部业务源保持。M2仍ACTIVE，完成2/12≈16.67%；B003房间业务和Windows/macOS业务NOT_RUN，最终13项证明仍由B012闭环。

固定房间文档和Schema按批准草案采用，仅调整ACTIVE/草案标题与批准状态注解，并去除重复frontmatter身份的空标题锚点；第1节起语义正文逐字节相同，Schema全部公共字段/定义/端点结构相同。邀请只授予申请入场资格，房间管理、主持及后续席位控制分别授权，不启动Session或授予私密视图。

GOV-M2-ROOM-API-REGISTRY da25997 已独立治理PASS：315/315/0/0，所有要求实际exit0，93证据成员由父尺寸/hash读回，未解决项为空；本地主线五门禁实际0，已释放临时副本清理及753成员封存完成。Bootstrap017已在代码编辑前永久退役，原FAIL/NOT_RUN和阅读/审计辅助失败资格均保留。B002准确7ede9be的业务/完成状态独立接收、主线接收、任务SQL与秘密文件清理和1271成员最终封存已完成。

下一步正常PLAN冻结并激活B003，再通过新鲜IMPLEMENT路由仅在批准的九类新文件范围实施Linux私人房间/邀请/审批、可信同事务访客兑换及真实SQL验证。未提前声明B003业务或M2出口通过。

## 此前状态（保留原文）

## 当前 M2：B002 Linux 认证基础独立验收通过

正常 PLAN v9→v10 仅将 M2-B002 VERIFYING→COMPLETED；批准的14字段冻结摘要 f25c45d24b03d97b33ffa73cfd6fa1ae7ab682b57fce587329a7773b8e174d51 由原生 Go 校验并保持，其余11批、next13、tombstones、M1归档和全部业务源字节保持。M2仍ACTIVE，B001/B002已完成2/12≈16.67%；其余十批PLANNED，最终13项需求仍待B012完整闭环证明，Windows/macOS业务NOT_RUN。

Owner批准 CHANGE-M2-AUTH-API-V1 后的账户、HTTPS/Cookie/CSRF、单局访客认领、SQL原子提交与加密幂等回执基础在5683689独立业务PASS；与3b13b27业务字节相同。独立实测50单元/契约、31真实独占SQL、81竞态，0 fail/skip；全仓check/test/Linux CI/vet/license实际exit0，另4项隐私/撤权/取消/字段绑定探针和1项原生状态校验PASS，普通日志秘密命中0。独立回执SHA256 b6e13c740653aa341f37b2cde72855949d0beb5aabfdf148c2febeb5f893e126，344成员已由父逐尺寸/hash读回，未解决必需项及后来必需检查数组为空。

原001/003在准确独立候选闭合；002仍限定d47115f原授权登记夹具五行补修的独立治理PASS。原源FAIL/NOT_RUN、独立只读缓存环境FAIL、外置JSON包装断言及父读回助手错误均保留资格，之后同源实际通过，不重写历史。验证器缺席时访客兑换拒绝；本批组合接缝不代表B004/B011真实服务启动、房间/席位或M2最终出口完成。

当前完成投影等待独立状态接收、本地主线快进、仅本任务数据库/两秘密文件/已释放快照清理与最终封存；没有提前声称这些步骤PASS。下一步B003私人房间/邀请/审批公共契约须具体CHANGE批准后才可实现；草案只保存仓库外，不改变已完成B001/B002契约或源文件。

## 此前状态（保留原文）

## 当前 M2：B002 候选通过父验证，等待独立验收

正常 PLAN v8→v9 仅将 M2-B002 IMPLEMENTING→VERIFYING；批准的14字段契约由原生 Go 摘要校验并保持，其余11批、next13、tombstones及M1归档保持。B001已完成并清理/封存，当前完成仍1/12≈8.33%，没有认证业务 PASS。

按 Owner 已批准 CHANGE-M2-AUTH-API-V1 实现版本化 Linux HTTPS 接口、局部账户/安全会话、单局访客认领、显式 SQL 原子提交及重试撤权检查；访客兑换在可信房间入场验证器缺席时拒绝。限当前冻结范围，不放宽公共字段或密码参数。最终13项需求仍待B012。

父业务验证实际在3b13b27通过50单元/契约、31独占SQL及81竞态命名事件，0 fail/skip；check/test/Linux CI/vet全部同来源实际exit0，日志秘密命中0。先前d47115f原授权登记测试五行补修已独立治理PASS、94成员尺寸/hash读回；原两次FAIL及后续NOT_RUN保留，003仅补正文读取时限和等价版本幂等规范化，不改公开字段。下一步独立B002业务/状态验收，不提前宣称完成；cmd/platformd本批只供认证组合接缝，实际服务启动仍待B004/B011。

## 此前状态（保留原文）

## 当前 M2：B002 Linux 认证实现启动

正常 PLAN v7→v8 仅将 M2-B002 FROZEN→IMPLEMENTING；批准的14字段契约由原生 Go 摘要校验并保持，其余11批、next13、tombstones及M1归档保持。B001已完成并清理/封存，当前完成仍1/12≈8.33%，没有认证业务 PASS。

按 Owner 已批准 CHANGE-M2-AUTH-API-V1 实现版本化 Linux HTTPS 接口、局部账户/安全会话、单局访客认领、显式 SQL 原子提交及重试撤权检查；访客兑换在可信房间入场验证器缺席时拒绝。限当前冻结范围，不放宽公共字段或密码参数。最终13项需求仍待B012。

## 此前状态（保留原文）

## 当前 M2：B002 认证契约冻结

正常 PLAN v6→v7 仅将 M2-B002 PLANNED→FROZEN；批准的14字段契约由原生 Go 摘要校验并保持，其余11批、next13、tombstones及M1归档保持。B001已完成并清理/封存，当前完成仍1/12≈8.33%，没有认证业务 PASS。

按 Owner 已批准 CHANGE-M2-AUTH-API-V1 实现版本化 Linux HTTPS 接口、局部账户/安全会话、单局访客认领、显式 SQL 原子提交及重试撤权检查；访客兑换在可信房间入场验证器缺席时拒绝。限当前冻结范围，不放宽公共字段或密码参数。最终13项需求仍待B012。

## 此前状态（保留原文）

## 当前 M2：B002 公开认证契约已批准，准备冻结

Owner 对具体 CHANGE-M2-AUTH-API-V1 回复“同意”；原草案保留，正式文档与 Schema 按批准内容登记，Schema 仅改为 ACTIVE 并移除标题草案字样。正常 PLAN v5→v6 仅调整未启动 B002 范围/阅读/验收/验证；B001 冻结及其余十批契约逐字节保持。B002 仍 PLANNED，尚无认证实现或业务 PASS。

B001 完成投影 d5668d6 已经独立接收、本地主线接收、清理本任务 SQL 与密钥文件并封存 722 成员证据；当前 GOV-M2-PLATFORM-API-REGISTRY 0bd1010 独立 PASS、无未解决必需发现、本地主线接收和 198 成员封存完成，限定治理登记，未当作认证实现证明。M2 仍 ACTIVE，完成 1/12≈8.33%，13 个最终需求出口仍由 B012 证明。Windows/macOS 业务 NOT_RUN。

## 此前状态（保留原文）

## 当前 M2：B001 独立业务验收通过

正常 PLAN v4→v5 仅将 M2-B001 VERIFYING→COMPLETED；原14字段与冻结418233a32e381a337749524f792a2dc9b210145f452366a2d4fcf549686d1328、其余11批、next13及tombstones保持。M2仍ACTIVE，完成批次1/12≈8.33%；全M2出口未满足，两个REQ只取得基础部分证据。

精确候选cde9d7213ea9cc3705d56ec3b92c97ce321f27e5/tree e20de853279d80dc94df6ffd238c76ec7f9b5065独立只读ACCEPT PASS，无未解决必需发现；实现来源0619aa6d60a9d7f57b8dc5dbf9b11d005a87d64e和437个非state blob/mode保持。实际独立76unit、27ownedSQL、103race及补充5命名事件均PASS、0 fail/skip；5事件为一个父项、三叶项及另一项，不称5个独立叶用例。父check/test/LinuxCI/vet和其他必需验证实际exit0，普通日志秘密命中0。

仓库外independent-business-cde9d72的ACCEPTANCE_RECEIPT摘要db1a21b36bd81a97e6fad2326c00454cecd8d5d2c702e3466bacd71362ec4cb7、131成员清单摘要5aabbfdc20c84c1982354f8a15b9c3ca24b7234222408c7ec79320d1391d56ba均由Root逐项尺寸/hash核对。所有历史FAIL/NOT_RUN及验收助手错误资格保留；SQL租约已实际释放、其他连接0。本COMPLETED投影仍待独立state-only接收、本地主线接收、只清理本任务服务和证据封存；不得提前宣称这些动作完成。

下一批B002仍PLANNED，公开认证/API/Cookie/错误格式没有实施或获批。具体CHANGE-M2-AUTH-API-V1草案及26定义Schema在仓库外m2-auth-api-v1-proposal，DRAFT_NOT_APPROVED；当前不释放B002。Linux范围和已有M1完成资格保持，Windows/macOS业务NOT_RUN，全V1未完成。

## 此前状态（保留原文）

## 当前 M2：B001 候选通过父验证，等待独立验收

正常 PLAN v3→v4 仅将 M2-B001 IMPLEMENTING→VERIFYING；原14字段、冻结418233a32e381a337749524f792a2dc9b210145f452366a2d4fcf549686d1328、其余11批、next13及tombstones保持。完成仍0/12，不提前宣称独立PASS。

干净签名候选0619aa6d60a9d7f57b8dc5dbf9b11d005a87d64e实际通过76单元、27独占PostgreSQL、103竞态用例，named fail/skip均0；check/test/Linux CI/vet全部实际exit0，日志秘密命中0。六个业务路径均在冻结允许范围，完成M1归档逐字节保持。父证据在仓库外m2-b001-platform-core/PARENT_VALIDATION_0619AA6.json；只构成REQ-DATA-002/REQ-PLAYER-003的基础部分证据。

ACC-M2-B001-001默认沙箱回环连接NOT_RUN和002复测夹具重复标识FAIL均保留；001仅恢复已授权本任务数据库执行权限，002仅单文件加入测试进程随机标识，不清库或削弱断言。准确修复来源已实际复测通过，原失败不改写。签名核对所需旧临时允许签名文件仅按已接受的同一公钥/主体恢复，无新增trust。下一步严格独立只读ACCEPT、状态接收与仅本任务服务清理/证据封存。B002公开认证/API契约仍须具体CHANGE批准。

## 此前状态（保留原文）

## 当前 M2：启动 B001 内部账户与租户基础

正常 PLAN v2→v3 仅将 M2-B001 FROZEN→IMPLEMENTING；原14字段和冻结摘要 418233a32e381a337749524f792a2dc9b210145f452366a2d4fcf549686d1328、其余11批、next13与tombstones保持。依赖为空、WIP=1，Owner继续M2及Linux范围的授权有效。

正式IMPLEMENT须先通过当前批次路由；仅内部账户/工作区/成员/单局访客关系和权限、显式SQL及本批验证。没有业务候选或业务PASS；完成仍0/12。B002新公共认证/API契约需具体CHANGE批准；本激活不替代该决定。已完成M1和所有历史证据资格保持。

## 此前状态（保留原文）

## 当前 M2：B001 内部基础契约冻结

正常 PLAN v1→v2 仅将 M2-B001 PLANNED→FROZEN，原14字段不变，冻结摘要 418233a32e381a337749524f792a2dc9b210145f452366a2d4fcf549686d1328；其余11批仍PLANNED、next13、tombstones=[]，active=0。前置为空，Owner的继续M2授权有效。

B001限定内部账户、工作区、成员和单局访客关系/权限服务及显式SQL；不创建登录协议、Cookie、公共API、房间或AI。缺失公开契约的B002仍需具体CHANGE批准，不能借B001提前实施。下一步单独登记IMPLEMENTING并生成正式批次路由。

## 此前状态（保留原文）

## 当前 M2：平台侧原生计划 v1

正常 PLAN 已分配 M2-B001—M2-B012，共12个 PLANNED 批次、next13、tombstones=[]、active=0，全部未冻结。13个M2 REQUIRED需求完整覆盖；顺序为账户/租户基础、认证与公共协议、私人房间、启动门禁、同步与导出、持久Continuation、模型/凭据、AI隔离/预算、模型网关、玩家界面、Linux Compose、完整认证。Rules仓库M2-B001的历史资格独立保留，不当作本平台M2-B001的完成。

当前可以先准备无新公共协议的B001内部关系模型与权限基础。B002所需公开API/认证/错误格式在现有schemas中尚不存在，必须提交具体CHANGE草案并获批后实施；本计划只登记该前置，不替代批准。未来发现M0集成门禁仍阻挡正式M2模型协议时须以真实来源提交限定治理补修，不能规避检查。

当前M2完成0/12，业务尚未开始；M1已完成记录、远端PR #24状态及原历史原文继续保留。所有批次默认顺序执行，无并行授权。

## 此前状态（保留原文）

## 当前 M2：正式规划基线

Owner 已批准 CHANGE-M2-PLANNING-ENTRY。控制面补修业务来源 7589256fc9f85fcd91e35d4593363ff5af9efe1b 的259项控制面回归全部PASS、0 fail/skip，check/test/Linux CI/vet实际exit0；待验收来源87687ae59d0a57a919f89aa3a5520dda6b52561b经正式ACCEPT入口及证据核对PASS。Bootstrap-015已永久退休。

本次正常PLAN仅将已完成M1推进到紧邻M2 NOT_GENERATED基线，尚无M2批次。已完成M1 v78、14/14、next15全部字节保存于 .codex/state/completed/M1/MILESTONE_PLAN.yaml，来源500d07dac96460f36b6ae2953ae22d4133dea41a。M1的最终本机接收、任务服务清理和归档闭环已实际完成，最终归档SHA256 fbb9a29641311be88d568fea9bd038422de8ee50ee165870cff12bc0c03830f4；原文历史证据与资格保留。

Linux M1已推送远端 codex/m1-linux-complete，PR #24 OPEN；Linux/macOS基线通过，Windows基线因固定Lua依赖编译失败，main尚未合并。M1 Windows/macOS业务验收仍NOT_RUN；本次M2推进不解决该远端合并决策。

下一步按既有13个M2 REQUIRED需求形成平台批次。账户/工作区/私人房间/模型网关/玩家界面缺少正式实现；SessionActor、事件、投影、重连和权限过滤基础可以在新批次内接入。新增公共协议草案须经CHANGE批准后实施，不宣称已获协议批准。M2业务进度尚未开始，全V1未完成。

## 此前状态（保留原文）

## 当前 Linux M1：M1-B009 独立验收通过

正常 PLAN v77→v78 仅将 M1-B009 VERIFYING→COMPLETED；冻结合同 edf35377a4fc1a9bbb252e177aef3d7b99bc4e52659540fb5ac18a7d5f5f48d1、全部14字段、其余批次和next15保持。最终全部批次完成，M1正常ACTIVE→COMPLETE。

精确干净签名业务候选 df6ad1de1d7491db2f05c56f10bf90e72fbff72e/tree99eccf6a659d47dd1d497f4a5ce2dabb8e0a797c 已取得独立只读ACCEPT PASS，无未解决必需发现。父实际用例：source-and-fixed-replay-json=3 PASS; m1-exit-gate-16-json=20 PASS；所有named fail/skip0，canonical门禁全部exit0。独立回执SHA256 c0f3a66a45f4721498e3a6a02074a8afdc75e8738ff5500ca1b6eee2317c61c8 与证据清单SHA256 95551a45eaeb0618e5a6395f85da66ce6cd4af74d4449aa69f745eac00b13f36（226成员）已由父逐个核对尺寸与hash。原始独立结果及补充测试细节以封存回执为准。

所有历史失败、环境限制、草稿与证据助手错误保持原文及原日志，资格不变；状态投影只记录已有业务PASS，不冒充业务复测。本COMPLETED状态还须独立state-only验收、本地主分支接收、仅本任务临时服务清理和最终归档；以上未实际完成前不提前声称。Windows/macOS NOT_RUN，全V1未完成。

原生完成批次 14/14≈100.00%；Linux M1退出门禁的业务验收完成，等待上述状态接收和清理归档闭环。无Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：B009 业务候选完成，等待独立验收

正常 PLAN v76→v77 仅将 M1-B009 IMPLEMENTING→VERIFYING，原冻结14字段、其余批次、next15及冻结摘要 edf35377a4fc1a9bbb252e177aef3d7b99bc4e52659540fb5ac18a7d5f5f48d1 保持。M1仍 ACTIVE，完成批次13/14≈92.86%。

精确干净签名候选 df6ad1de1d7491db2f05c56f10bf90e72fbff72e/tree99eccf6a659d47dd1d497f4a5ce2dabb8e0a797c 父门禁23项均exit0；外层M1验收20 named PASS、0 fail/skip，机器16个主TEST ID全部PASS、27条新执行子命令合计3530 named PASS。固定场景三恢复边界一致，59个实际Runner ESRCH；八个规定30s Fuzz及提交语料通过；Compose9阶段及真实失败/SIGINT/SIGTERM三清理回归通过；canonical全部通过；普通日志私密/凭据命中0。498成员清单逐一核对，摘要 c43d4b87781a35c40e174a669b7809d838847499f159d1919ad84c640c828ef8。这里只记录实际父验证，不声称独立业务PASS。

B009业务相对激活358d8b7的55路径都在原允许目录，没有核心实现、公共契约或规范变更。历史VCS构建假设、测试镜像文件权限、Fuzz空集合内部表示误判等实际FAIL保持；最后失败的最小输入77b1d3c050b93b31已经提交回归语料，新候选采用保留全部规范化线缆事实的比较。审计助手的hash前缀错误已修正，原失败记录保留，不重跑已通过业务门禁。草稿只具草稿资格。

下一步独立只读ACCEPT必须针对精确候选及当前状态，实际SQL仅使用本任务临时容器 d7a6ac6f1d491a03335477ee277945e2a094b76787c23b0ec38ed9301fd90594 与六个约定数据库；父将交接零连接租约。独立通过、正常最终COMPLETED/M1 COMPLETE状态验收、可信本地主分支接收、只清理本任务临时资源及最终归档全部实际完成前不得宣告Linux M1完成。Windows/macOS NOT_RUN，全V1未完成，无Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：启动最后的认证批次 M1-B009

正常 PLAN v75→76 仅将 M1-B009 FROZEN→IMPLEMENTING；原14字段与冻结契约 edf35377a4fc1a9bbb252e177aef3d7b99bc4e52659540fb5ac18a7d5f5f48d1、其他13批和next15保持，M1 ACTIVE。前置 B007/B008 均已完成独立业务及状态验收、本地主线接收、仅任务临时服务清理和逐项读回归档。

B008干净签名业务45acd0daf26001527facd095abda104b9cd14b45获得独立PASS；001/002/003仅对该来源CLOSED，旧失败原文保持。COMPLETED状态d7de066ddd1053282d8b47dc73822b817333488f已获独立state-only PASS并以10个可信签名提交快进本地main，全部接收后检查exit0；仅本任务f9da测试数据库已连接数0后停止/删除并确认缺席。最终归档SHA256 ff79cc5548aac42bd56add21c993abe79eefffa1af11e22331c1d54638b4e8e5，1645成员实际读回一致。

B009按既有冻结契约完成版本化source-only五角色最小样例、确定性命令及重放、8个30秒Fuzz目标与稳定回归corpus、隔离Compose完整生命周期和16个M1出口TEST ID的精确候选机器证据。仅在允许的games/fixture-minimal、tests/m1、tests/fixture-minimal、tests/fuzz、tests/smoke及两部署文件内施工；核心缺陷返回所属批次REPAIR，实际需要Owner决定时停下。本状态不是B009业务通过，独立状态验收及正式IMPLEMENT入口仍须完成。

原生进度13/14≈92.86%；仅Linux，Windows/macOS NOT_RUN，全V1未完成。

## 此前状态（保留原文）

## 当前 Linux M1：M1-B008 独立验收通过

正常 PLAN v74→v75 仅将 M1-B008 VERIFYING→COMPLETED；冻结合同 de3b3774294e2f056c04362e996ef40b4c39251c6a63a996fa2cdc2b774c63c6、全部14字段、其余批次和next15保持。M1保持ACTIVE。

精确干净签名业务候选 45acd0daf26001527facd095abda104b9cd14b45/tree4dc2e57a699e4e9833ae93d2fd334b25a49c2922 已取得独立只读ACCEPT PASS，无未解决必需发现。父实际用例：original-and-tracked-diagnostic-probes=52 PASS; related-unit-json=1459 PASS; related-race-json=1408 PASS; hostapi-postgres-regression=48 PASS; session-postgres-regression=24 PASS; replay-postgres-regression=34 PASS; migration-postgres-regression=45 PASS; security-matrix-json=21 PASS；所有named fail/skip0，canonical门禁全部exit0。独立回执SHA256 b819db2e74692398c928ab68f6808a83531a70e52aec0d6da6e42f7f11ad8076 与证据清单SHA256 bf3d15ca2417f28da9cb2cf3872b89d2b6a55291a77916bee5afec2368e49986（124成员）已由父逐个核对尺寸与hash。原始独立结果及补充测试细节以封存回执为准。

所有历史失败、环境限制、草稿与证据助手错误保持原文及原日志，资格不变；状态投影只记录已有业务PASS，不冒充业务复测。本COMPLETED状态还须独立state-only验收、本地主分支接收、仅本任务临时服务清理和最终归档；以上未实际完成前不提前声称。Windows/macOS NOT_RUN，全V1未完成。

原生完成批次 13/14≈92.86%；继续既定剩余Linux M1批次。无Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：安全补修进入独立验收

正常PLAN仅将M1-B008 IMPLEMENTING→VERIFYING，plan_version73→74；冻结合同de3b3774294e2f056c04362e996ef40b4c39251c6a63a996fa2cdc2b774c63c6及14字段、其余13批与next15保持，M1 ACTIVE。B009仅FROZEN，不能在B008完成前施工。

精确干净签名业务候选2ded7f258cebec73b70e09c2c76b2c459b2b0a54/treecc46b227694d2fac1ecf96dff9a70e38eeb905a5父必需门禁全部实际通过：1459 related、1408 race、48 Host、24 Session、34 replay、45 migration、21 security顶层与11分类实际775子例，named fail/skip0；check/build/test/ci/vet/license/native/signature/privacy各exit0。712成员父清单SHA5ad1ee64bd774bf686ab57874d64330f6902b9b1c3128f6a3fbb3f7a2c75725b已逐个尺寸hash核验。独立只读精确候选ACCEPT正在进行，未宣告其PASS或发现正式闭合。

保留原ACC-M1-B008-001合成私密诊断FAIL19与Owner明确批准，ed65独立FAIL73/002OPEN，以及8154实际Session诊断负例003/完整测试两case失败及partialFAIL78。新的检查不替换旧FAIL。checkpoint.Value、Token与API返回的*Session普通fmt均使用常量有界脱敏；真正授权与状态JSON字节保持。治理测试夹具单文件dd3补修已独立PASS_GOV_ONLY51，原权限门禁/契约/plan不变，只关闭当时完整测试假定失效，不验收B008业务。所有草稿、读取/字段/编译等助手错误原文原日志保留且资格不变。

父实际观察698 Runner与10 daemon全部signal0=ESRCH，四fixture库其他连接0后已释放独占验收租约；仅本任务f9da隔离服务保持运行配置不变，授权独立验收使用。服务清理/本地主分支接收/最终归档须业务和状态分别独立验收后完成，不提前声称。Linux范围持续，Windows/macOS NOT_RUN，V1总体未完成。原生14批12COMPLETED，12/14≈85.71%，无Owner决策等待。

## 此前状态（保留原文）

### Linux M1 最终认证原契约冻结（尚未施工）

M1-B009 从 PLANNED 进入 FROZEN；原14字段逐项不变，固定摘要为 `edf35377a4fc1a9bbb252e177aef3d7b99bc4e52659540fb5ac18a7d5f5f48d1`。仅登记已授权的既定 fixture-minimal、16项M1出口、八个指定30秒Fuzz/提交语料及真实Compose生命周期范围；未实施业务代码，未扩展核心/公共契约/架构/许可或多平台。

M1-B008 仍 IMPLEMENTING，业务候选 `ed65d8d8f09da9f185749a15074e74699ef784a2` 的父实际安全/回归/规范门禁已通过，独立只读 ACCEPT 尚待结论。此前 ACC-M1-B008-001 私密合成标记诊断负例及用户明确批准补修均完整保留，新候选结果不替换旧 FAIL。B009 不能在 B007/B008 依赖均已完成前激活或声明出口完成。

M1 仍 ACTIVE；14批中12批 COMPLETED，Linux 批次计数进度12/14=85.71%。Windows/macOS NOT_RUN，V1 总体完成未声明。此次为正常 PLAN 状态投影，只允许3个状态路径、plan_version +1、其余13批和旧正文tail逐字节保持；原生前后验证及独立状态审计均须保留实际证据。

## 此前状态（保留原文）

## 当前 Linux M1：迁移闭环完成，启动安全验收

正常PLAN仅将M1-B008 FROZEN→IMPLEMENTING，冻结de3b3774294e2f056c04362e996ef40b4c39251c6a63a996fa2cdc2b774c63c6、全部14字段、其他批次与next15保持，既定上游B003/B004/B005/B006/B007均COMPLETED。B008尚无业务PASS，按原合同实际验证身份/席位、隐藏视图、租户、沙箱、秘密、能力、审计、资源、导入、迁移和跨Session边界；不引入账户/Room等产品或公共契约变化。

B007业务0ac26398ead753666f88091e367c6b41ef97f991独立PASS、4db VERIFYING与99e完成状态分别独立state-onlyPASS，父逐个核验102/45/49成员。父与独立1413相关/1413race/45迁移/48Host/24Session/34replay全namedPASS0fail/skip，另5实际独立补充PASS。正式99e72e71df30d5c723156405744e0ec8867d450f已本地主分支实际接收，5个前向commit签名及freshmain门禁均0；main回执b8478168abbbc33ebe0caf48f79f91a2d896e527550f48fa2259c7be5c55e140。三个fixture库0连接及全部观察进程ESRCH后，精确60b临时容器已stop/remove并验证不存在，cleanup回执5c19df5dd5a9e29238cd4f8d08a5f5f9432e589cd74256407fdf4b2799121ed4。868成员B007归档实际逐个读回，SHA2566a874477b55b594ef106053b1bc7d7e07a5c58563dad93597d23a489d836456e。

所有上游失败、原PENDING提案及后来Owner批准、草稿/夹具/读取/编译/计数/释放助手错误均保留且资格不变。Linux范围的Owner批准继续有效，Windows/macOS NOT_RUN；无远端、许可、依赖、trust或公共授权变化。B009仍PLANNED，既有edf35377合同保持。

原生14批次12COMPLETED，12/14≈85.71%，M1 ACTIVE。安全批次及最终认证须实际完成并独立验收后才宣告LinuxM1退出；全V1未完成，无Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：M1-B007 独立验收通过

正常 PLAN v70→v71 仅将 M1-B007 VERIFYING→COMPLETED；冻结合同 3fd98c06a16d5d511f5fda8b5e1d58a7f864bae940f7d87896348574ea227629、全部14字段、其余批次和next15保持。M1保持ACTIVE。

精确干净签名业务候选 0ac26398ead753666f88091e367c6b41ef97f991/tree5d53ebd5bdc1fefb1e4e3344c6ac977cd231de22 已取得独立只读ACCEPT PASS，无未解决必需发现。父实际用例：related=1413 PASS; race=1413 PASS; migration-postgres=45 PASS; hostapi-postgres-regression=48 PASS; session-postgres-regression=24 PASS; replay-postgres-regression=34 PASS；所有named fail/skip0，canonical门禁全部exit0。独立回执SHA256 bd97a543fa6e0033e520eb5746a3d99031b9298c8982cd9078a54e6b07d991f2 与证据清单SHA256 569843d77033e85e1248f416c8636e25e94627d7cc1f27ce737a20a50e804f96（102成员）已由父逐个核对尺寸与hash。原始独立结果及补充测试细节以封存回执为准。

所有历史失败、环境限制、草稿与证据助手错误保持原文及原日志，资格不变；状态投影只记录已有业务PASS，不冒充业务复测。本COMPLETED状态还须独立state-only验收、本地主分支接收、仅本任务临时服务清理和最终归档；以上未实际完成前不提前声称。Windows/macOS NOT_RUN，全V1未完成。

原生完成批次 12/14≈85.71%；继续既定剩余Linux M1批次。无Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：冻结既有安全验收范围

正常PLAN仅将既有M1-B008 PLANNED→FROZEN，原14字段合同保持，固定hash de3b3774294e2f056c04362e996ef40b4c39251c6a63a996fa2cdc2b774c63c6；B007仍VERIFYING，独立业务验收进行中。此动作只固定已计划的安全验收范围，不启动B008施工，不改变B007候选、其余批次、next15或任何公共/授权/许可契约。

安全批次按既有TEST-SEC-001确定性验证身份/席位、隐藏视图、租户、Lua沙箱、秘密、能力、审计、资源预算、导入、迁移和跨Session边界，并对package/Lua/HostAPI/Session执行竞态检查。Linux限定继续有效，Windows/macOS NOT_RUN。启动施工前须B007独立业务PASS、正常完成状态接收及临时服务清理完成；不会用状态冻结替代业务证明。

当前14批次11COMPLETED，11/14≈78.57%，M1 ACTIVE。B007已取得父精确候选全部门禁PASS，独立验收仍待；所有原失败/环境/助手计数错误证据保留。B008仅冻结；B009仍PLANNED，原edf35377合同不变。全V1未完成，无Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：安全迁移进入独立验收

正常 PLAN 仅将 M1-B007 IMPLEMENTING→VERIFYING，冻结合同3fd98c06a16d5d511f5fda8b5e1d58a7f864bae940f7d87896348574ea227629、全部14字段、其余13批次与next15保持。精确干净签名业务候选0ac26398ead753666f88091e367c6b41ef97f991/tree5d53ebd5bdc1fefb1e4e3344c6ac977cd231de22父必需门禁全部通过；独立只读ACCEPT正在进行，不提前声称业务独立PASS。

父实际1413相关、1413race、45真实迁移、48HostAPI、24Session、34replay named用例全部PASS、named fail/skip0；canonical check/test/ci/build/vet/license/signature/native门禁实际exit0。父215成员证据清单SHA256 1c44515977dd7d2825828ddc0364d8dd83a67330d7c46d037b05f1950b4154c8已逐个核对尺寸与hash。实际来源为本任务临时Linux PostgreSQL、生产Lua Runner与source-built platformd。Windows/macOS NOT_RUN。

完整五角色固定锁、升级前有界恢复点、持有SQL锁的隔离目标VM预演、安全边界/关键Continuation/ACL失败拒绝、九阶段SQL故障回滚与记录点回退均实际验证。升级与回退不改变原事件ID/payload/cursor、原命令回执或raw恢复点事实；daemon停止条件与明确固定版本重启通过真实进程和玩家视图验证。

全部历史FAIL原文和原日志保留：批准前目录/完整图Host失败、草稿读取方式错误、短夹具credential前置失败、未使用import编译失败与封存助手计数错误。旧真实58为Session24+replay34合计，当前单套件计数已按实际日志核对，未修改实现或测试断言以放宽验收。上游治理修复与B014已独立通过、main接收并清理；Owner批准Linux范围与两个前置变更继续有效。

原生14批次11个COMPLETED，11/14≈78.57%；M1 ACTIVE。B007还须独立业务验收、正常COMPLETED状态投影验收、本地主分支接收、仅本任务临时数据库清理和最终归档。之后继续B008与B009直至Linux M1退出；全V1未完成，无Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：前置修复完成，恢复迁移批次

正常 PLAN v67→v68 仅将 M1-B007 BLOCKED→IMPLEMENTING，原冻结合同3fd98c06a16d5d511f5fda8b5e1d58a7f864bae940f7d87896348574ea227629与全部14字段保持；其他13批、next15及历史正文保持。原B007在cbb9a36的12 tracked+17 untracked草稿已精确保存，将在当前已验收上游基础上按批准范围恢复，不将旧草稿或原失败视为业务PASS。

Owner已明确批准CHANGE-M1-LINUX-MIGRATION-PREREQUISITES两项上游补齐。治理修复8a33226独立GOVERNANCE_ONLY PASS（239回归、239race、42对抗）；完整图Host修复129620e独立business PASS（237相关、237race、48真实named，0fail/skip），B014完成状态a077b76独立state-only PASS且本地主分支实际接收到a077b764315b57c78edc49223cef96245a979f1e。精确f875测试数据库0连接后已stop/remove并验证不存在，cleanup回执1f1b65fca31633a707a15044c4d883f205c357a81efc4292b39d7542b6b4dafe；B014732成员归档实际创建并逐成员读回，SHA25682490971a63c1162fe67e25c6ef1a017cc93bcd58e8ab7cd68fee6373cde1c4a。

B007继续验证完整精确锁、安全边界、隔离预演、旧事件重放、原子迁移及记录恢复点回退，补齐真实未声明边界、已结束Session、恢复故障和Linux守护进程组合验证。原目录门禁FAIL、Host完整图FAIL、旧buildFAIL与历史PENDING提案原文保留。B008/B009尚未开始；B009既有14字段合同保持，未来迁移fuzz路径例外仍仅受批准的edf35377精确合同约束。

当前14批次11个COMPLETED，按批次数11/14≈78.57%；M1 ACTIVE，Windows/macOS NOT_RUN，全V1未完成。B007尚无业务候选或独立业务验收通过，不提前声称迁移或M1完成；无Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：完整依赖图认证修复独立通过

正常 PLAN v66→v67 只将 M1-B014 VERIFYING→COMPLETED，冻结合同 edfd469b5e257551887d06b884d83db27974e64d8635e261c9e8a05fae51dd92、全部14字段、其余13批次和 next15 保持。业务精确签名候选 129620eb94b55be7f4a4f126b2645f5fd92b62d5/tree471dddf51eac8e04f42a71a9f72f0fd6b36a4d04 已取得独立只读 ACCEPT PASS，无 unresolved required finding。父与独立均实际通过 237 相关、237 race、48 真实 PostgreSQL/生产 Runner named用例，named fail/skip 为0；canonical check/test/ci/build/vet/license/signature/native门禁实际0。

独立回执 SHA256 d93a7b8b2db05e22aa74046e95994b2eb6ec77ca342a77bfdb1754ec6ceefe44，证据清单 SHA256 d46c837b399744040abac7e364678cd40a5c97003e4bbc0273e71f6393b3bcb7（225成员）已由父逐个核对长度与hash。B014三次启动与021 VERIFYING状态投影另有 state-only 资格，不冒充业务复测。补充 Schema 测试早期夹具前置失败原日志保留为 INVALID_BOUNDARY_PROOF；修正有效Schema后原4项Host断言实际4/4通过。原B007目录门禁FAIL、Host完整图FAIL、历史buildFAIL和批准前PENDING提案原文保留。

VM提供已认证完整5角色包hash副本，Host校验完整不可变图；ModuleBindings仍独占脚本来源与能力。无Lua被动包没有脚本、token或隐式授权；缺失、多余、替换hash、跨包借用能力和默认零授权均拒绝。治理路径门禁8a33226已独立治理PASS；B007原草稿与冻结3fd98c保持，当前仍BLOCKED，完成本批次状态独立验收、本地主分支接收与精确临时服务清理后正常恢复。

原生14批次11个COMPLETED，按批次数11/14≈78.57%；M1 ACTIVE，Windows/macOS NOT_RUN，全V1未完成。此状态仅记录已取得的B014业务独立PASS；COMPLETED状态投影验收、本地主分支接收及f875c84b临时数据库清理仍待实际完成，不提前声称。无Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：完整依赖图认证修复待独立验收

正常PLAN v65→v66只将M1-B014 IMPLEMENTING→VERIFYING，冻结合同edfd469b5e257551887d06b884d83db27974e64d8635e261c9e8a05fae51dd92与全部14字段、其他13批次、next15及历史正文保持。精确签名业务候选129620eb94b55be7f4a4f126b2645f5fd92b62d5/tree471dddf51eac8e04f42a71a9f72f0fd6b36a4d04仅修改批准的5个文件：VM完整包hash绑定副本与Host全图认证及相应测试。父237相关、237race、48真实PostgreSQL/生产Runner named用例0fail/skip；clean check/test/ci/build/vet/license/signature/codex actual0，180成员清单bcd492321c33d2681da1f8cb61c269280ef496082a9372c89ef42f3b4ca7d6f1已封存。独立business ACCEPT正在验证，不将父PASS视为独立通过或批次完成。

治理路径修复8a33226af5916cd4a18259450eca123415912075已实际独立PASS（仅治理），239回归+239race+42对抗0fail/skip，无required finding；64成员清单ab3f773ba9cb74cd04ab0b10939001def51ef474e8193922dddd43ffb8c7fa8b与回执10aa150b759e42cf27f8380c02223f36b111c02c8ff1015ff2482583015571ce已由父逐字节核对。原目录FAIL、Host完整图原FAIL及测试辅助buildFAIL保持原资格。

B014五类签名安装图的无Lua content/assets/ui包正常通过隔离验证、已安装对象认证和真实命令提交；ModuleBindings继续独占脚本来源/能力。缺失/多余/替换包、库借用root能力、被动包越权、零默认授权均拒绝，map副本无法改变authority。既有公共格式/API、三层授权交集、依赖许可与平台义务保持。B007继续BLOCKED且原3fd98c...冻结与全部草稿仓库外保存，B014独立通过与完成后恢复B007，随后B008/B009直至Linux M1。

原生14批次10个COMPLETED，按批次数10/14≈71.43%；M1 ACTIVE，Windows/macOS NOT_RUN，全V1未完成。B014精确f875c84b1d070e0eaae4d7fb2e8c3374b84833835dd949eb7a20642323bfa4f1 PostgreSQL服务仍只供独立验收，不声称已清理/main接收。无新Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：完整依赖图认证修复施工前置

正常PLAN v64→v65只将M1-B014 FROZEN→IMPLEMENTING，冻结合同edfd469b5e257551887d06b884d83db27974e64d8635e261c9e8a05fae51dd92及全部14字段不变。冻结源b6b052bfa8baa25b2f7d7819b23683c9d1be56d3、36成员清单5c618e83540c51e3f4ddc04985806c0482a2cf0ffadb9554b4a949e78a1333d0与原生1run/pass已封存；此前追加签名242及其原生状态验证保持。这里只提供施工前置，无B014业务PASS。

Owner已批准CHANGE-M1-LINUX-MIGRATION-PREREQUISITES，两项真实原始FAIL不改写。有限治理路径修复8a33226af5916cd4a18259450eca123415912075已形成239个原生回归及239个race通过，独立验收待实际回执。B014仅认证VM已验证完整不可变包hash绑定的副本，Host验证全部五类包；ModuleBindings继续独占实际脚本来源及能力授权。被动包不获脚本或默认权限，现有三层交集、零授权、公开格式/API、事件、许可与依赖保持。须fresh IMPLEMENT、真实签名安装/PG/生产Runner和负向、原子、并发矩阵，并对精确签名候选独立ACCEPT。

B007保持BLOCKED，原冻结hash3fd98c06a16d5d511f5fda8b5e1d58a7f864bae940f7d87896348574ea227629、其他批次和next15保持；全部迁移草稿完整仓库外保存，上游两项完成后精确恢复并继续B007/B008/B009直到Linux M1。原生14批次10个COMPLETED，按批次数10/14≈71.43%；M1 ACTIVE，Windows/macOS NOT_RUN，全V1未完成，无新Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：完整依赖图认证修复合同冻结

正常PLAN v63→v64只将M1-B014 PLANNED→FROZEN，14字段冻结合同hash为edfd469b5e257551887d06b884d83db27974e64d8635e261c9e8a05fae51dd92。Owner批准的CHANGE-M1-LINUX-MIGRATION-PREREQUISITES及原始两个FAIL保持；B007 BLOCKED、其冻结合同3fd98c06a16d5d511f5fda8b5e1d58a7f864bae940f7d87896348574ea227629、其他13批次合同、next15和历史记录不变。新增批次及阻塞状态来自签名242df8fbfe9b54f8f33f11061e73f5c064d66ed8，其原生PLAN转换实际1run/pass与34成员清单124e7f57caa165622c9e9fb4501174f0d17a3b6e7f915b2bedca47de65f8536d已保存；仅状态前置，无业务PASS。

B014仅补齐已有VM完整不可变包hash副本与Host全图认证，实际能力调用仍按真实ModuleBindings授权。五种包角色的被动包不获得假脚本/默认能力，三层交集、零默认授权、公共格式/API、事件、许可、依赖和平台义务保持。实施须正常启动并fresh IMPLEMENT，真实PostgreSQL/生产Runner五类包及原授权/原子/并发回归后对精确签名候选独立ACCEPT；之后恢复B007，继续B008/B009直至Linux M1完成。

原生14批次10个COMPLETED，按批次数10/14≈71.43%；B014尚未实施，治理维护独立验收仍待实际回执，B007草稿完整仓库外保存。M1 ACTIVE，Windows/macOS NOT_RUN，全V1未完成，无新Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：已批准完整依赖图上游修复

Owner已直接回复“批准”CHANGE-M1-LINUX-MIGRATION-PREREQUISITES（提案SHA256 c67e4f0ef460786dc73537cdb37419a77277108c7b0c1382a7723981082d50c6）。本次正常PLAN v62→v63将B007 IMPLEMENTING→BLOCKED，并只追加未启动的M1-B014，next14→15；原13批次全部14字段合同、已有冻结hash及历史记录保持。B007的12个tracked与17个新文件草稿已按字节/校验和保存到仓库外，当前源不含草稿。两个真实原始FAIL均保留：旧M0迁移目录门禁冲突与无脚本的三类包在Host隔离验证被CONFIGURATION_REJECTED拒绝。

有限治理维护GOV-M1-MIGRATION-PATH-SCOPE-GATE已正常登记并形成签名候选8a33226af5916cd4a18259450eca123415912075，239个实际native regression与239个race均无fail/skip；独立ACCEPT待实际回执，不宣称已通过。B014单一目标是认证VM已验证完整不可变包hash图，ModuleBindings继续独占实际脚本来源与能力授权。不得为被动包加假脚本或授予默认能力，不改变公共格式/API、三层授权交集、零默认授权、事件语义、许可、依赖或其他平台。

原生14批次已有10个COMPLETED，按批次数10/14≈71.43%；这是新增批准前置工作的分母变化，不是工作退步。B014仍PLANNED无业务PASS，须正常冻结/启动、真实五类包PostgreSQL与生产Runner测试、签名精确候选和独立ACCEPT；两项上游修复完成后继续原合同B007，再完成B008/B009直至Linux M1。main仍523e29ced387f581fedc51fc476dea809ac3f025干净；M1 ACTIVE，Windows/macOS NOT_RUN，全V1未完成，无新Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：B007安全迁移施工前置

正常PLAN v61→v62只将B007 FROZEN→IMPLEMENTING；合同hash 3fd98c06a16d5d511f5fda8b5e1d58a7f864bae940f7d87896348574ea227629及全部14字段保持，next14、其他批次及此前正文不变。冻结源97bd30377acf41bc7ffa731b428347fa1c2f12d8的37成员清单5a602be647a7ef316dc057b64e23c03e551bb9982f3a7ab58653017bb3174160及实际native1run/pass已封存。此激活只提供施工前置，尚无B007业务PASS；实施必须使用精确签名源的fresh IMPLEMENT路由，完成后另行独立ACCEPT。

B007只实现完整包/依赖锁、安全边界、最小恢复点、真实PostgreSQL隔离迁移预演、原事件兼容恢复、原子切换及此次恢复点回滚。保持事件不可变、三层授权交集、默认零授权、稳定错误、有限资源和runtime DDL/rawSQL禁令。不触及多平台、产品路线、许可、公开能力或备份管理产品。

原生13批次已有10个COMPLETED，按批次数10/13≈76.92%；当前唯一施工批次B007，B008/B009仍PLANNED。依赖B003/B006均已独立验收并本地接收，main当前523e29ced387f581fedc51fc476dea809ac3f025干净；B006精确服务已清理且965成员归档SHA256 b01fbffef1f0d30b131942deb03bf60d846b803ce837ed08d677f41dc0b6ce72。原业务001/002仅476、状态003仅523闭合，所有历史FAIL/环境/助手/LOST资格保持。

M1 ACTIVE，只完成Linux内容；Windows/macOS NOT_RUN，全V1未完成，无新Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：B007安全迁移合同冻结

正常PLAN v60→v61只将B007 PLANNED→FROZEN，14字段合同hash为3fd98c06a16d5d511f5fda8b5e1d58a7f864bae940f7d87896348574ea227629。此前9a6760e431124b1e3e2a47126822ee1325b39598正常细化未开始批次内部范围，39成员清单dc87f4360ee0588d9555e59ca8275fce7f4c0a84ab5b00beda46c0935d160947和实际native1run/pass记录保持。冻结不改变目标、需求、验收、测试、停止条件、依赖、其他批次、next14或历史正文；B007尚无业务实现证明，须正常激活后取得fresh IMPLEMENT路线。

安全迁移沿用现有包声明与平台验证的边界、授权交集和完整依赖锁。升级前保存最小恢复点，真实PostgreSQL隔离副本预演命名空间/关系数据、状态、checkpoint与锁，验证原事件可重放及不变量；正式迁移原子切换，失败维持旧版本可用，回滚只恢复此次升级前恢复点。原事件ID/负载/版本/语义不改，无运行中范围解析、自动补丁、DDL/rawSQL能力、通用降级或正式备份产品。

原生13批次已有10个COMPLETED，按批次数10/13≈76.92%，B007/B008/B009尚未完成。B006业务476独立PASS、状态523独立PASS，main已实际接收523，精确97dc容器已清理；965成员最终归档SHA256 b01fbffef1f0d30b131942deb03bf60d846b803ce837ed08d677f41dc0b6ce72。原001/002仅476、状态003仅523闭合；原始FAIL/环境/助手/LOST资格保持，不将状态冻结当成业务测试。

M1 ACTIVE，Linux限定；Windows/macOS NOT_RUN，全V1未完成，无新Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：B007 安全迁移批次范围准备

本轮正常 PLAN 只细化尚未开始、未冻结的 M1-B007 内部施工范围，增加 internal/storage/package/**、internal/projection/**、internal/session/recovery/**、internal/package/install/session*.go。它们用于现有包安装工厂的历史完整依赖锁校验、不可变迁移效果与多版本重放、派生数据恢复；不增加新的公开能力或变更架构、授权规则、Schema/Host API、许可与出口门禁。objective、requirements、non_goals、机器契约、验收、测试、停止条件、依赖及其他冻结字段保持原文，B007仍PLANNED且无冻结hash，下一步仅经正常PLAN冻结及激活。

原生计划共有13个批次，10个COMPLETED，当前按批次数10/13≈76.92%，B007/B008/B009未完成。B006业务来源47661588aba3ee9375854c2ac884046754e1ff5e独立PASS，完成计数纠正提交523e29ced387f581fedc51fc476dea809ac3f025；状态独立37成员清单ab09f991d59e7dd27c1def999b5dc21b4d60beb633d82eb0c007bf417f222ccb为PASS_FOR_STATE_PROJECTION_ONLY，003仅523闭合。本地main已实际快进至523，接收回执SHA256 e19614e3150d170f57d9952da56b5786c063304b4b860c04e6f3c9f32ed780ac；精确97dc测试容器已移除，清理回执SHA256 f1167d9f2aa63be1aebef42a63e02c15db412a55a174c243c5f498f49c02fbf4。965成员归档M1_B006_LINUX_COMPLETION_RECEPTION_523E29C.tar.gz已实际创建并读回逐成员验证，SHA256 b01fbffef1f0d30b131942deb03bf60d846b803ce837ed08d677f41dc0b6ce72。既有原2b FAIL001、354 FAIL002、d24状态FAIL003及各修复、环境/助手失败和LOST证据保持，历史数据不改写。

M1仍ACTIVE，只推进Linux；Windows/macOS NOT_RUN，全V1未完成。继续B007安全边界、最小恢复点、真实PostgreSQL隔离预演、原子完整锁迁移及对应迁移前恢复点回滚；然后完成B008安全负向用例与B009Linux整体出口，无新增Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：B006 已完成，进度摘要计数修正

原生计划 v59 共13个批次，B001–B006及B010–B013共10个COMPLETED；B007、B008、B009仍PLANNED，tombstones为空。当前按批次数完成10/13，约76.92%；这不是工作量估算。M1仍ACTIVE，Windows/macOS NOT_RUN，全V1未完成。

独立状态复核指出 d24bf3ac7bdb9ff34e80794d861674289920ba0b 两份新增摘要误写9/12、75%（ACC-M1-B006-003）。本次只纠正派生摘要，不改变原生MILESTONE_PLAN.yaml、v58→v59的B006完成转移、冻结合同、next14、其他批次或业务源。错误摘要在此前状态区保持原文并明确归属于历史，不能作为当前计数。

B006业务来源仍为签名提交47661588aba3ee9375854c2ac884046754e1ff5e，tree 41b03e850eeffcad08b4e38347ec9957b9f2826a。独立业务回执SHA256 f7a318690a74fa288c0e47b66bfa15f1038fa2d484cef59225aacaaa4571551f、79成员清单SHA256 f7efe48ae8bdbbe78dda65f43e4bcd7404872e70ebc771fe1345a612848b1549为PASS；父及独立实际362 related、136 race、58 PostgreSQL用例均无named fail/skip，required门禁实际通过，原001/002仅对476闭合。该业务证据不因本次数值摘要修正而重标来源；d24状态FAIL及此前业务FAIL、助手失败、环境失败和LOST资格全部保留。

本次状态修正的独立验收、本机main接收、精确已拥有97dc容器清理及完整归档仍由各自后续实际回执确认，本文不声称这些动作已发生。继续B007安全迁移、B008安全检查和B009整体Linux出口，无新Owner决策等待。

## 此前状态（保留原文；含已纠正的历史计数）

## 当前 Linux M1：事件重放 B006 COMPLETED

正常 PLAN v58→v59 只将 B006 VERIFYING→COMPLETED；冻结合同 `873d928f833d8c19a84c3c5b16151b780ba7bc95ac2838ac3c6f852e4f549e5e`、其他批次、next14 与此前正文保持。业务验收来源是精确签名修复提交 `47661588aba3ee9375854c2ac884046754e1ff5e`（tree `41b03e850eeffcad08b4e38347ec9957b9f2826a`），不是本次三文件状态投影。父与独立 actual362 related、136 race、58 real（B006 34+B005 24），named 0 fail/skip；required check/test/vet/license/ci/build/signature actual0。父33成员清单 SHA256 `cd2834fde7d9afc71019cc75c34b62817daa88e1de6db798f9fc1a493d32231e`。独立 `/home/zyc14588/.codex/visualizations/2026/10/05/01a10c5c-1d16-7a91-8c2f-1909e2af4f43/m1-b006-linux-progress/independent-repair-4766158/ACCEPTANCE_RECEIPT.json` SHA256 `f7a318690a74fa288c0e47b66bfa15f1038fa2d484cef59225aacaaa4571551f`、79成员 manifest SHA256 `f7efe48ae8bdbbe78dda65f43e4bcd7404872e70ebc771fe1345a612848b1549`：PASS，无未闭合 required finding；原001/002仅对476 CLOSED。

已提交事件、创建种子、原始请求/收据及完整 Go 效果保持不可变。重放验证原事件/Schema/确定性输入，派生状态和检查点仅作经完整不可变前缀核验的加速器。生产 Host 在原 Session 行锁内、任何 SQL 效果前验证256记录、4MiB证据/8MiB读取总量和完整128行/128关系条目/256KiB Row编码上限；累计事实来自不可变效果，与恢复共用 FoldFacts，派生数据缺失也不能绕过。重复命令优先返回原收据；净数量合法的删除再新增可提交，超限拒绝后实际休眠和零派生恢复仍可用。

独立原容量探针1、两合法命名空间累计第129行探针1、恢复/restore回调 caught-write19 named（16目标叶子）及准确拒绝补充1均实际PASS；712个记录runner及4个实际daemon PID已ESRCH，b005/b006其他连接各0。关系quantity在476增加实际签名fixture PostgreSQL/VM证明，使用两把不同的合成publisher/certification key及Policy.SigningBytes，不是生产认证服务；此前2b/354的quantity单元测试资格原文保留。legacy不完整效果仍明确不完整，不认证为可恢复历史。

历史失败不改写：原2b独立FAIL001、1479分层scope检查FAIL、354独立FAIL002（001当时仅对354 CLOSED）均保持；旧256MiB服务disk-full实际34run25pass9fail与首个替换容器min_wal_size初始化失败、dirty开发失败和修复证据全部保留。归档权限比对助手的实际失败单独保留并限定为tar导出权限/exec flags资格，不冒充业务失败或改变Git源blob/mode核验。本次摘要生成字段误读与缺失body导致的partial一文件PLAN改动记录保持，三文件完整投影须另行实际核验后才提交。重启前B006 /tmp首次proof仍明确LOST，旧B004原始FAIL/修复、B013无效harness及上游已完成接收记录继续保留。状态独立验收、本机main接收和精确已拥有97dc容器清理由各自实际回执记录，不把状态投影当作新业务测试或接收证明。

M1 ACTIVE，有效完成9/12（75%，按批次数）；继续B007安全迁移、B008安全检查和B009整体Linux出口。Windows/macOS NOT_RUN，全V1未完成，无新Owner决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：事件重放 B006 VERIFYING

正常 PLAN v57→v58 只将 M1-B006 IMPLEMENTING→VERIFYING；冻结合同 `873d928f833d8c19a84c3c5b16151b780ba7bc95ac2838ac3c6f852e4f549e5e`、其他批次、next14 和此前正文保持。精确签名业务候选 `2b295e1494f1afd25ef0392684116e82dc3c5706`（tree `c4ce230b066a177e9ab1cae74040679695068f1d`）的父必验 PASS：362 related、136 race、51 real（B006 27+B005 24），named 0 fail/skip；check/test/vet/license/ci/build actual0。父 29 成员证据清单 `business-2b295e1/EVIDENCE_MANIFEST.json` SHA256 `5b9d79c4c39d2d5f0b71bd3c05ab5239d22e847a1f945cd15ba6dba079bc34bc`，位于持久证据目录 `m1-b006-linux-progress`。独立业务验收尚待，不能把此状态投影或父测试当作独立 PASS。

真实 PostgreSQL 完整效果、确定性输入及事件 Schema 元数据加入原命令事务；重建只读不可变创建种子和事件，删除派生状态后与所有快照边界的状态、游标、许可座位视图及生产 VM 可见文档事实相同。当前完整重建事实匹配的检查点可用，旧版或不兼容/自算哈希伪造缓存忽略；恢复 SQL 失败/实际 VM 检查点恢复失败可重试且不改写事件、请求、任务、Continuation、Outbox、版本或游标。实际 platformd 在 SQL 已提交而广播仍被隔离屏障阻塞时取消请求、SIGTERM 回收，再从零派生状态重启并去重，外部意图未重复。命名 quantity 的重建只作单元测试资格，真实 fixture 验证 declared documents。M1 暂不扩成迁移或正式备份产品。

重启前 B006 `/tmp` 首次 proof 明确 LOST；新的三步 state-only 复验 PASS 109 成员清单 SHA256 `3357fe3dae3dc52db267076ede48985eabd2e9cb32cf30b41b8cac6bae749a2a`，限定补充清单 SHA256 `9aeb893a892a10f341b4ad8d661491342e727acbb1b9732b33fe663c40b1c76d`。新开发五条实际失败日志保持 dirty precommit 资格，不覆盖或充当干净候选验收。B005 main 接收、B004 原始 FAIL/修复及 B013 无效补充 harness 历史限定全部保留。

M1 ACTIVE，有效完成8/12（66.7%，按批次数）；B006 独立验收/完成/main 接收尚待，随后继续 B007–B009 至 Linux M1 出口完成。Windows/macOS NOT_RUN，全 V1 未完成。无新 Owner 决策等待。

## 此前状态（保留原文）

## 当前 Linux M1：事件重放 B006 IMPLEMENTING

正常 PLAN activate v56→v57；仅未开始 B006 草案细化后冻结、激活。不可变创建种子和已提交事件/完整Go效果与确定性输入作为恢复依据，当前状态/数据投影/检查点只作派生加速器；重放不重复调用外部模型，不改写旧事件、任务、Outbox或幂等结果。范围补充内部数据契约、Host效果记录、已验证安装图的session构造和现有最小platformd组合，保持现有规范、公共包格式、依赖与许可。所有已冻结/完成合同、其他批次与next14保持不变，摘要 `873d928f833d8c19a84c3c5b16151b780ba7bc95ac2838ac3c6f852e4f549e5e`。B006业务NOT_RUN；重放旧证据不完整时失败关闭，禁止拿当前状态充当创建种子。

B005 实际本机main接收PASS，回执 `/tmp/trpg-m1-b005-linux-20261006/main-reception-8ed2f00/RECEPTION_RECEIPT.json` SHA256 `2b1252915300f2f28811f9fd619fa86ff9f9e91e938c7af4650a3dadd2c132e6`；精确cb393aa独立业务与各状态投影来源保持分列。M1 ACTIVE，有效完成8/12（66.7%，按数量）；B006–B009与整体Linux出口尚待。Windows/macOS NOT_RUN。B004原始FAIL/修复和B013补充harness的无效证明记录保持。无新的Owner决策等待。

## 历史 B005 完成接收及此前状态（保留）

## 当前 Linux M1：事件重放 B006 FROZEN

正常 PLAN freeze v55→v56；仅未开始 B006 草案细化后冻结、激活。不可变创建种子和已提交事件/完整Go效果与确定性输入作为恢复依据，当前状态/数据投影/检查点只作派生加速器；重放不重复调用外部模型，不改写旧事件、任务、Outbox或幂等结果。范围补充内部数据契约、Host效果记录、已验证安装图的session构造和现有最小platformd组合，保持现有规范、公共包格式、依赖与许可。所有已冻结/完成合同、其他批次与next14保持不变，摘要 `873d928f833d8c19a84c3c5b16151b780ba7bc95ac2838ac3c6f852e4f549e5e`。B006业务NOT_RUN；重放旧证据不完整时失败关闭，禁止拿当前状态充当创建种子。

B005 实际本机main接收PASS，回执 `/tmp/trpg-m1-b005-linux-20261006/main-reception-8ed2f00/RECEPTION_RECEIPT.json` SHA256 `2b1252915300f2f28811f9fd619fa86ff9f9e91e938c7af4650a3dadd2c132e6`；精确cb393aa独立业务与各状态投影来源保持分列。M1 ACTIVE，有效完成8/12（66.7%，按数量）；B006–B009与整体Linux出口尚待。Windows/macOS NOT_RUN。B004原始FAIL/修复和B013补充harness的无效证明记录保持。无新的Owner决策等待。

## 历史 B005 完成接收及此前状态（保留）

## 当前 Linux M1：事件重放 B006 PLANNED

正常 PLAN revise v54→v55；仅未开始 B006 草案细化后冻结、激活。不可变创建种子和已提交事件/完整Go效果与确定性输入作为恢复依据，当前状态/数据投影/检查点只作派生加速器；重放不重复调用外部模型，不改写旧事件、任务、Outbox或幂等结果。范围补充内部数据契约、Host效果记录、已验证安装图的session构造和现有最小platformd组合，保持现有规范、公共包格式、依赖与许可。所有已冻结/完成合同、其他批次与next14保持不变，摘要 `未冻结草案`。B006业务NOT_RUN；重放旧证据不完整时失败关闭，禁止拿当前状态充当创建种子。

B005 实际本机main接收PASS，回执 `/tmp/trpg-m1-b005-linux-20261006/main-reception-8ed2f00/RECEPTION_RECEIPT.json` SHA256 `2b1252915300f2f28811f9fd619fa86ff9f9e91e938c7af4650a3dadd2c132e6`；精确cb393aa独立业务与各状态投影来源保持分列。M1 ACTIVE，有效完成8/12（66.7%，按数量）；B006–B009与整体Linux出口尚待。Windows/macOS NOT_RUN。B004原始FAIL/修复和B013补充harness的无效证明记录保持。无新的Owner决策等待。

## 历史 B005 完成接收及此前状态（保留）

## 当前 Linux M1：B005 完成登记，等待最终状态验收及接收

正常 PLAN v53→v54，仅 B005 VERIFYING→COMPLETED；冻结14字段摘要 `df022fd1324b5e9cb052cac3017cf0ac0b698c90040116aac31628d754aa1c47`、其他批次、依赖和next14保持。独立精确业务 `cb393aaaf051fe6b720f3e2761145888b9fc0905` / tree `887519c72f295f4412c1223efceb42ae4b2e1e11` PASS，回执 `/tmp/trpg-m1-b005-linux-20261006/independent-business-cb393aa/ACCEPTANCE_RECEIPT.json` SHA256 `3d10eee5beb697eb4913f260785df9295320f13eb54763aa2f763d587bf01b94`；共享73成员清单 `4d08a1daa26ddc4b8273cf4b630ed8a997f04c6728f6749e443d67bf85d691df`逐size/hash核验，无未解决必修项。独立 actual299 related、79 race、24真实PG run/pass，0fail/skip，追加8个真实journal边界均PASS；范围/截断、非法游标/limit、图和租户替换、genesis不变及checkpoint不改权威已独立验证。单写者/背压/故障隔离、完整Envelope/幂等、九SQL真实回滚/无广播、只读效果拒绝、两Seat过滤、实际platformd WebSocket/重启/SIGTERM及worker回收通过。纯Go双替身、official-trust named guard保持unit资格，不冒充服务或安装签名证明。父dirty e1全部失败/预提交记录留存，不重标cb。

独立9ec VERIFYING仅state-only PASS，回执 `/tmp/trpg-m1-b005-linux-20261006/independent-business-cb393aa/STATE_PROJECTION_9EC4603.json` SHA256 `394a9a654e5d273b5914dbfc741971bdd9c37a76e080312f7c59122a7ac7cfb9`，0状态问题；329非state blob/mode、冻结/其他批次/next14与旧摘要正文保留。三计划投影48成员证据保持。此完成候选尚待最终state-only验收；本机main仍cf559ed，B005接收PENDING。独立验收者已释放自有PG32773，release SHA256 `83c74bf6ad471b8147726884b72f7afcaf89fe21a9d6829770841418d53451d5`，实际其他连接0；实际接收后才按精确ID/标签/镜像清理并归档。

M1 ACTIVE，有效完成8/12（66.7%，按批次数量）；B006–B009与整体Linux出口尚待。仅Linux，Windows/macOS NOT_RUN。B013已实际main接收与归档、B004原始FAIL/修复以及B013无效补充harness均保留。无新Owner决策、公共格式、依赖或许可变更。重放/迁移认证留B006/B007，不作为B005完成内容。

## 历史 B005 VERIFYING 及此前状态（保留）

## 当前 Linux M1：B005 精确业务检查通过，等待独立 ACCEPT

正常 PLAN v52→v53，仅 B005 IMPLEMENTING→VERIFYING；14字段冻结摘要 `df022fd1324b5e9cb052cac3017cf0ac0b698c90040116aac31628d754aa1c47`、其他批次、依赖与next14不变。精确清洁业务 `cb393aaaf051fe6b720f3e2761145888b9fc0905` / tree `887519c72f295f4412c1223efceb42ae4b2e1e11`，23允许路径。Actor 有界单写者、鉴权完整 Envelope 与重复结果恢复、真实SQL原子通知/提交后过滤广播、只读投影/检查点、不可变创建证据及有界租户/图游标读取已实现。实际 platformd 内部 fixture 提供两种 Seat 的最小 WebSocket，验证重启、撤权与信号回收；未扩展账户/Room/AI 产品。重放认证与迁移仍待 B006/B007。

精确 actual299 related、79 race、24真实PG run/pass，0fail/skip；check/test/vet/license/ci及实际 runner/platformd 构建 exit0。31成员清单 `/tmp/trpg-m1-b005-linux-20261006/business/EVIDENCE_MANIFEST.json` SHA256 `4d528faaf4854a8cb48ef57cddad8effee037bfc859422e63c085d68141c1afa` 已逐项核验；9真实SQL故障点全回滚且无广播，8只读写入攻击拒绝，命令七效果及通知同事务、重复不执行/不广播、真实休眠/重新激活/结束均核验。纯Go Actor double 与显式 official-trust 的 named guard 仅unit，不能冒充安装签名或PG证明。全部 dirty e1 开发失败、原先编译错误与错误 outbox 预期保留。

三计划投影已独立 PASS：revise49d4b85、freezeffb6565、activatee1f9cd2，48成员清单 `658e0503b96e83d17cf5fafa718f887d2cf4fb4317fcc471792d6786227896a7`。B005 独立业务/VERIFYING/完成投影、实际 main接收及服务清理仍PENDING，不能标为完成。自有PG32773 ID `e6a6046068dd020b545864247ec045bd202616333b0000996a54ff49382fb30a` 仅保留给独立验收；父线程已结束业务测试，验收者释放后才进行接收与清理。

M1 ACTIVE，已完成有效7/12按批次数量计；B005–B009及Linux总出口尚待。仅Linux，Windows/macOS NOT_RUN。main目前cf559ed，B013已实际接收及归档；B004原始FAIL/修复、B013无效补充harness记录和历史证据保持。无新增Owner决策、公共格式、依赖或许可变更。

## 历史 B005 计划投影及此前状态（保留）

## 当前 Linux M1：SessionActor B005 IMPLEMENTING

正常 PLAN activate v51→v52；B005 未开始草案完善后冻结并激活。沿用有界单写Actor、完整命令信封/幂等、七效果同事务、先提交后广播与席位重连规范。草案新增 trusted storage/package、storage/postgres 范围以实现固定有界游标/创建证据读取，投影/检查点只读不提交，不增加公共协议、依赖、产品或许可；所有已冻结/完成合同、其他批次与 next14 不变。冻结摘要 `df022fd1324b5e9cb052cac3017cf0ac0b698c90040116aac31628d754aa1c47`。业务 NOT_RUN；下一步按新 IMPLEMENT 路由施工。

B013 实际本机 main 接收已 PASS，回执 `/tmp/trpg-m1-b013-linux-20261006/main-reception-cf559ed/RECEPTION_RECEIPT.json` SHA256 `d0ce4491d2b3659d7e245196f2121c6b0fea02200b7ade339cf044e3a09bcb13`；精确814独立业务验收与状态门禁来源保持分列。M1 ACTIVE，有效完成7/12（58.3%，按数量）；B005–B009 与整体 Linux 出口尚待，Windows/macOS NOT_RUN。无新的Owner决策等待。

## 历史 B013 完成登记与此前状态（保留）

## 当前 Linux M1：SessionActor B005 FROZEN

正常 PLAN freeze v50→v51；B005 未开始草案完善后冻结并激活。沿用有界单写Actor、完整命令信封/幂等、七效果同事务、先提交后广播与席位重连规范。草案新增 trusted storage/package、storage/postgres 范围以实现固定有界游标/创建证据读取，投影/检查点只读不提交，不增加公共协议、依赖、产品或许可；所有已冻结/完成合同、其他批次与 next14 不变。冻结摘要 `df022fd1324b5e9cb052cac3017cf0ac0b698c90040116aac31628d754aa1c47`。业务 NOT_RUN；下一步按新 IMPLEMENT 路由施工。

B013 实际本机 main 接收已 PASS，回执 `/tmp/trpg-m1-b013-linux-20261006/main-reception-cf559ed/RECEPTION_RECEIPT.json` SHA256 `d0ce4491d2b3659d7e245196f2121c6b0fea02200b7ade339cf044e3a09bcb13`；精确814独立业务验收与状态门禁来源保持分列。M1 ACTIVE，有效完成7/12（58.3%，按数量）；B005–B009 与整体 Linux 出口尚待，Windows/macOS NOT_RUN。无新的Owner决策等待。

## 历史 B013 完成登记与此前状态（保留）

## 当前 Linux M1：SessionActor B005 PLANNED

正常 PLAN revise v49→v50；B005 未开始草案完善后冻结并激活。沿用有界单写Actor、完整命令信封/幂等、七效果同事务、先提交后广播与席位重连规范。草案新增 trusted storage/package、storage/postgres 范围以实现固定有界游标/创建证据读取，投影/检查点只读不提交，不增加公共协议、依赖、产品或许可；所有已冻结/完成合同、其他批次与 next14 不变。冻结摘要 `未冻结草案`。业务 NOT_RUN；下一步按新 IMPLEMENT 路由施工。

B013 实际本机 main 接收已 PASS，回执 `/tmp/trpg-m1-b013-linux-20261006/main-reception-cf559ed/RECEPTION_RECEIPT.json` SHA256 `d0ce4491d2b3659d7e245196f2121c6b0fea02200b7ade339cf044e3a09bcb13`；精确814独立业务验收与状态门禁来源保持分列。M1 ACTIVE，有效完成7/12（58.3%，按数量）；B005–B009 与整体 Linux 出口尚待，Windows/macOS NOT_RUN。无新的Owner决策等待。

## 历史 B013 完成登记与此前状态（保留）

## 当前 Linux M1：B013 完成登记，等待最终状态验收及接收

正常 PLAN v48→v49 仅 B013 VERIFYING→COMPLETED；14字段冻结摘要 `2b2c92ec6524dacdcd5a6cdd7f697f102c425b31fcfd1b308f77f89103ba41a3`、其他批次和 next14 保持。独立精确业务 `814fdf079ddfa2f102ba4dbaf7e7232579df6e8c` / tree `7c2c493d788a1e99aa87c3bead7002e92e30dc5a` PASS，回执 `/tmp/trpg-m1-b013-linux-20261006/independent-business-814fdf0/ACCEPTANCE_RECEIPT.json` SHA256 `1308abc9107a7846f9f63f984f4cafda6098b9457b2370fdec5096f61827a37e`；61成员清单 `f3d797c99f3de260446e42084ef16bdf1eecf367d04824a2105a3333842bf867` 已逐字节核验，unresolved_required_findings=[]。独立278 related、142 race、20真实PG actual run/pass，0 named fail/skip；签名绑定、隔离固定回调、运行哈希/限额、精确安装图ACL/对象、同workspace锁原子登记、Resume新Token及必需审计/实际回收均经独立验收。独立路径构建 runner 哈希 `sha256:d1edf6f7ae43629a4195582ad66816d92c7766e726a22a6d1ead494661f42f83`，父精确门禁 runner 为 `sha256:8e84375e92c6ac178b5755b02b2fac552e40e47fe05b3c946005ac89352a53eb`，分别记录，不宣称二进制同一。三次附加跨回调累计限额夹具未到预定边界，保持 INVALID_BOUNDARY_PROOF，不作 source FAIL/PASS；该累计边界仅静态审查和已有资源测试支持，未宣称附加黑盒成功。补充签名绑定负例的有效结果及父dirty31开发失败均保留真实来源。

独立032 VERIFYING投影仅state-only PASS，回执 `/tmp/trpg-m1-b013-linux-20261006/independent-verifying-032f614/ACCEPTANCE_RECEIPT.json` SHA256 `1c4ab1c1edf732fa720cdc4819034c282db9db755befe491b3fcf53d7fa02479`，14成员清单 `b44d365e47e26fd40076ab0cde706359ebee08467ede34f64a029eea2c0f7fe3`已核验，0状态问题；已独立分配/冻结/激活56成员计划证据保持。状态门禁不重标814业务来源。本完成候选仍待最终state-only验收，本机main仍ff66，B013接收PENDING。自有PG32772仅在独立方释放及实际main接收后，按精确ID/labels/image清理并持久归档；B004 d348原FAIL/51修复PASS及实际接收清理保持。

M1 ACTIVE，有效完成7/12（58.3%，按数量）；后续B005–B009和整体Linux出口尚待。仅Linux，Windows/macOS NOT_RUN；没有新的Owner决策、公共格式、依赖或许可变更。

## 历史 B013 VERIFYING 与此前状态（保留）

## 当前 Linux M1：B013 精确业务门禁通过，等待独立验收

正常 PLAN v47→v48 仅 B013 IMPLEMENTING→VERIFYING；14字段冻结摘要 `2b2c92ec6524dacdcd5a6cdd7f697f102c425b31fcfd1b308f77f89103ba41a3`、其他批次、depends_on 与next14不变。精确清洁业务 `814fdf079ddfa2f102ba4dbaf7e7232579df6e8c` / tree `7c2c493d788a1e99aa87c3bead7002e92e30dc5a`，16允许路径。显式三层授权与实际签名绑定 runner哈希/限额、schema/seed/固定回调，nil配置零授权及旧签名/NoHost保持；隔离认证不接触运行库。每个已安装图节点实际ACL/对象/精确lock核验，真实VM/service先配置，然后单PG事务在安装workspace锁下登记Session、精确artifact evidence及全部data_targets；恢复使用权威状态与新Token，必需审计失败关闭。

精确 actual278 related、142 race、20真实PG run/pass，0fail/skip；check/test/vet/license/ci及实际runner/platformd构建全部exit0。31成员清单 `/tmp/trpg-m1-b013-linux-20261006/exact-gates-814fdf0/EVIDENCE_MANIFEST.json` SHA256 `201579b58307b64c9c8793de62312cca72576855fff95cb32eef5bb9a19e67af` 已逐项核验；实际PG覆盖安装→持久化重载→生命周期/七效果→恢复、旧Token/重复创建、权限/图/策略替换、四SQL真实中止、VM/必需审计失败、对象丢失及真实workspace锁安装竞争拒绝。纯Go/memory负例明确仅unit，不冒充服务证明。所有dirty31开发失败/预提交来源保留，状态门禁不重标814业务来源。

独立业务和VERIFYING投影、完成登记及本机main接收仍PENDING，B013不完成。自有PG32772精确ID `210b2ad5419526dc03cc63bbd216d955f031da34ca6f3dc4be16e06208a98395` 保留给独立验收，释放后接收与清理。已独立三计划投影56成员清单 `201d7d10e9bd2ccec2bf0b9e642b3e7b82e71c1531fa0787ec5fa80305e23d97` 保持；B004实际main ff66及d348原FAIL/51修复PASS lineage不变。

M1 ACTIVE，已完成有效6/12按数量计；B013、B005–B009及Linux总出口尚待。仅Linux，Windows/macOS NOT_RUN。无新增Owner决策、公共格式、依赖或许可变更。

## 历史 B013 分配/冻结/激活及此前状态（保留）

## 当前 Linux M1：安装图与 Host 组合 B013 IMPLEMENTING

正常 PLAN activate v46→v47；B013 序号13，依赖已完成 B003/B004/B012，沿用既有规范补齐内部组合，不增加公共 Manifest/extension 语义、隐式授权、依赖或许可。本批14字段冻结摘要 `2b2c92ec6524dacdcd5a6cdd7f697f102c425b31fcfd1b308f77f89103ba41a3`。只在初始分配时给未开始的 B005 草案补 M1-B013 依赖；所有已冻结批次/已完成合同保持不变。业务尚 NOT_RUN；接下来按有效 IMPLEMENT 路由完成精确安装图 ACL/对象验证、显式三层授权、隔离固定回调认证与同安装 workspace 锁的 Session/data_targets 原子登记。

B004 修复业务51ce9de独立PASS及原d348 FAIL lineage保留；最终ff66a2d独立状态通过并已实际本机 main 接收，回执 `/tmp/trpg-m1-b004-linux-20261006/main-reception-ff66a2d/RECEPTION_RECEIPT.json` SHA256 `b8ab7950586e906ce03c76c81af44104f161b0304f8b5a3270adf54fe5168265`。B004 原输出预算问题仅对51关闭，状态门禁不重标业务来源。M1 ACTIVE，有效已完成6/12（50.0%，按批次数量；分母增加来自内部组合批次，非业务退步）；B013、B005–B009及整体Linux出口仍待。Windows/macOS NOT_RUN；无新的Owner决策等待。

## 历史 B004 完成登记及此前状态（保留）

## 当前 Linux M1：安装图与 Host 组合 B013 FROZEN

正常 PLAN freeze v45→v46；B013 序号13，依赖已完成 B003/B004/B012，沿用既有规范补齐内部组合，不增加公共 Manifest/extension 语义、隐式授权、依赖或许可。本批14字段冻结摘要 `2b2c92ec6524dacdcd5a6cdd7f697f102c425b31fcfd1b308f77f89103ba41a3`。只在初始分配时给未开始的 B005 草案补 M1-B013 依赖；所有已冻结批次/已完成合同保持不变。业务尚 NOT_RUN；接下来按有效 IMPLEMENT 路由完成精确安装图 ACL/对象验证、显式三层授权、隔离固定回调认证与同安装 workspace 锁的 Session/data_targets 原子登记。

B004 修复业务51ce9de独立PASS及原d348 FAIL lineage保留；最终ff66a2d独立状态通过并已实际本机 main 接收，回执 `/tmp/trpg-m1-b004-linux-20261006/main-reception-ff66a2d/RECEPTION_RECEIPT.json` SHA256 `b8ab7950586e906ce03c76c81af44104f161b0304f8b5a3270adf54fe5168265`。B004 原输出预算问题仅对51关闭，状态门禁不重标业务来源。M1 ACTIVE，有效已完成6/12（50.0%，按批次数量；分母增加来自内部组合批次，非业务退步）；B013、B005–B009及整体Linux出口仍待。Windows/macOS NOT_RUN；无新的Owner决策等待。

## 历史 B004 完成登记及此前状态（保留）

## 当前 Linux M1：安装图与 Host 组合 B013 PLANNED

正常 PLAN allocate v44→v45；B013 序号13，依赖已完成 B003/B004/B012，沿用既有规范补齐内部组合，不增加公共 Manifest/extension 语义、隐式授权、依赖或许可。本批14字段冻结摘要 `未冻结草案`。只在初始分配时给未开始的 B005 草案补 M1-B013 依赖；所有已冻结批次/已完成合同保持不变。业务尚 NOT_RUN；接下来按有效 IMPLEMENT 路由完成精确安装图 ACL/对象验证、显式三层授权、隔离固定回调认证与同安装 workspace 锁的 Session/data_targets 原子登记。

B004 修复业务51ce9de独立PASS及原d348 FAIL lineage保留；最终ff66a2d独立状态通过并已实际本机 main 接收，回执 `/tmp/trpg-m1-b004-linux-20261006/main-reception-ff66a2d/RECEPTION_RECEIPT.json` SHA256 `b8ab7950586e906ce03c76c81af44104f161b0304f8b5a3270adf54fe5168265`。B004 原输出预算问题仅对51关闭，状态门禁不重标业务来源。M1 ACTIVE，有效已完成6/12（50.0%，按批次数量；分母增加来自内部组合批次，非业务退步）；B013、B005–B009及整体Linux出口仍待。Windows/macOS NOT_RUN；无新的Owner决策等待。

## 历史 B004 完成登记及此前状态（保留）

## 当前 Linux M1：B004 独立复验通过，完成登记候选

正常 PLAN v43→v44 仅 B004 VERIFYING→COMPLETED；冻结摘要 `4b7d73a8d566d3097f3afbb45f8271e00d7aead7192992bba59974a717c08e31`、全部14字段、其他批次与next_batch_sequence=13不变。新精确业务 `51ce9de1a9171f661e759c77f7e33a7ddee611a5` / tree `9854edb1c8e4c5fc92953a32eb2e9055533c936d` 独立PASS，回执 `/tmp/trpg-m1-b004-linux-20261006/independent-repair-business-51ce9de/ACCEPTANCE_RECEIPT.json` SHA256 `0809a90fd46a990faa545ca5c06593f3fb21822d018654c60eb6453ff1d4c4df`，46文件清单 SHA256 `d016d10bcefc04ba227313f22e4a180310959542fecf7b9763595d27dbcfdc93` 已逐项核验，unresolved_required_findings=[]。独立201 related、58 race、32真实PG实际run/pass，0fail/skip；父完整check/test/vet/license/ci/build actual0绑定精确清洁51。

ACC-M1-B004-001仅对新51精确候选关闭：原两required断言bytes未改，仅新clone路径适配；raw2000与raw1010+result19在1024上限下均version0/commits0/BUDGET_EXCEEDED，rollback及污染VM回收断言PASS。新增低/default/打印+日志+结果组合/pcall超限及低额内正例、异常IPC计量、四真实PG回滚证明通过。旧d348 FAIL回执 `0a3e3530600ee377fff9bed30798a62d512bf63600d5e4d31e0b36685c94c86b`、46清单 `55a71a6d21dd37c82824f36095b087f27b87e308f301d76e551bb8dca0c9a2c6` 与所有历史预提交/错误元数据原件保留，不改成PASS。

正常VERIFYING投影bce（并保留859修复激活）独立state-only PASS回执 `/tmp/trpg-m1-b004-linux-20261006/independent-repair-verifying-bce7d40/ACCEPTANCE_RECEIPT.json` SHA256 `1ec6c3b5f0f4fb43cf86eaab87520efe4482f317cd2ffb6cef33c870f54b7352`，44文件清单 `ee45e3c354641fc13f1c6a664b2a71c31f00e32c2c32600638b57004128a8ec9` 已核验，0state问题；state门禁不重标业务测试为bce或本完成候选。本完成登记候选仍待独立最终state投影和本机main接收；接收后按精确owned容器ID清理32771并归档。main在准备时仍c67b019，不把PENDING冒充已接收。

M1 ACTIVE，有效已完成6/11（54.5%，按数量计），总出口尚未满足。B003安装图与Host认证/Session data-target组合已整理为独立未分配B013草案，接收后正常PLAN补齐，继而B005–B009；不改已冻结B003/B004。仅Linux，Windows/macOS NOT_RUN；无新增Owner决策或公开契约变化。

## 历史 B004 再VERIFYING及原始FAIL lineage（保留）

## 当前 Linux M1：B004 修复精确门禁通过，再次等待独立验收

正常 PLAN v42→v43 仅 B004 IMPLEMENTING→VERIFYING；冻结摘要 `4b7d73a8d566d3097f3afbb45f8271e00d7aead7192992bba59974a717c08e31`、全部14字段、其他批次及next_batch_sequence=13保持不变。新精确业务 `51ce9de1a9171f661e759c77f7e33a7ddee611a5` / tree `9854edb1c8e4c5fc92953a32eb2e9055533c936d` 修复原始print/warn、Host日志与结果的合计预算，仍保持64KiB最大上限、默认零授权与脱敏；仅7允许修复路径，无公共合同/依赖/许可变化。

精确清洁相关201 run/pass、Host race58 run/pass、真实PG32 run/pass，全部0fail/skip；check/test/vet/license/ci/build actual0。新25文件清单 `/tmp/trpg-m1-b004-linux-20261006/repair-exact-gates-51ce9de/EVIDENCE_MANIFEST.json` SHA256 `12487823bbb330b7779587d26b9fcddf54256ab0c430e0c069f26473efd4b814` 已逐项核验。新增低/default/组合/caught日志超限七类效果回滚及子进程回收，真实PG四负例读回所有九表计数0、version1/state1。预提交dirty859之194/32保持原来源。

独立 d348 FAIL 回执 SHA256 `0a3e3530600ee377fff9bed30798a62d512bf63600d5e4d31e0b36685c94c86b`、46成员清单 `55a71a6d21dd37c82824f36095b087f27b87e308f301d76e551bb8dca0c9a2c6` 保留，ACC-M1-B004-001尚待独立以原unchanged负例重新验收后关闭，不能以父PASS抹去。B004不COMPLETED、不接收main。自有fixture32771保留给复验，之后按精确ID清理。安装图组合/B005–B009与总出口仍待，M1 ACTIVE，已完成有效5/11按数量计，跨平台NOT_RUN；无新增Owner决策。

## 历史 B004 独立FAIL、修复激活及此前状态（保留）

## 当前 Linux M1：B004 独立负例 FAIL，进入契约内修复

正常 PLAN v41→v42 仅 B004 VERIFYING→IMPLEMENTING，冻结摘要 `4b7d73a8d566d3097f3afbb45f8271e00d7aead7192992bba59974a717c08e31` 与全部14字段、其他批次、next_batch_sequence=13 均不变。独立精确业务 `d348cd58fefbc5039c4070e80fe067f6fb180b54` / tree `c4a94701fb89f71e8d4149d23512f8cab75410a3` 出现稳定必修 `ACC-M1-B004-001`：Host mode print 只输出摘要，命令输出上限错误按摘要长度计费，低于 profile 默认64KiB 的命令限额可绕过。独立外置 overlay command `/tmp/trpg-m1-b004-linux-20261006/independent-business-d348cd5/independent-output-budget.json` actual exit1，raw log SHA256 `e4c2db4f622a473ba1001744ddc6d30964ddd6efd818ddd66f6435a443acc05a`；1024上限/raw2000/result1和raw1010/result19均实际version2/commits1/errnil，两叶断言FAIL。Host log同类readback未作断言，不能计为预算PASS。

此前 exact clean相关187、race49、真实PG27与完整门禁PASS保持原来源；它们没有覆盖新增边界，不能替代独立总判定或抹掉FAIL。B004不完成、不接收main。父侧接下来生成有效 REPAIR route，仅在B004允许路径补原始打印、Host日志与最终结果的统一预算及组合越限/回滚/污染VM回收负例；默认64KiB硬上限与脱敏保持，禁止通过扩大上限修复。验收代理已释放本轮自有fixture32771，父仍保留其精确ID用于同批修复验证，之后清理。不新增Owner决策、公共格式或依赖变化；其余M1仍待完成，仅Linux，跨平台NOT_RUN。

## 历史 B004 VERIFYING 及原始门禁证据（保留）

## 当前 Linux M1：B004 精确门禁通过，等待独立验收

正常 PLAN v40→v41 仅 B004 IMPLEMENTING→VERIFYING；14 个冻结字段、摘要 `4b7d73a8d566d3097f3afbb45f8271e00d7aead7192992bba59974a717c08e31`、depends_on、其余批次和 next_batch_sequence=13 均不变。业务候选 `d348cd58fefbc5039c4070e80fe067f6fb180b54` / tree `c4a94701fb89f71e8d4149d23512f8cab75410a3`，21 允许路径内实现版本化 Host 入口、实际模块来源/Token 授权、七类效果 MutationWorkspace 与静态 PostgreSQL 单事务；没有修改公共契约、依赖、许可或禁区。

精确清洁 Linux 相关 JSON187 run/pass、Host race49 run/pass、真实 PostgreSQL27 run/pass，全部0 fail/skip；check/test/vet/license/ci/build actual0。证据清单 `/tmp/trpg-m1-b004-linux-20261006/exact-gates-d348cd5/EVIDENCE_MANIFEST.json` SHA256 `1f95ea4e99edac8530bdd6a268f64df500a5f9f8c854158d01bef19677369556`，原始命令/log及逐项 inventory 已核验来源和哈希。数据库九类实际 SQL 错误、七类回滚和主动回调取消/runner 丢失均有读回与进程回收证据。ENVIRONMENT.json 纠正原 GATES 元数据 profile 标签误写，原件未替换。所有预提交 dirty来源、未执行 recorder失误和先前 NOT_PASS 原文保留。

独立 ACCEPT 与状态投影/本机 main 接收仍 PENDING；不把业务测试重标为本状态 SHA。自有临时 PostgreSQL 保留给独立验收，之后按精确 ID 清理。B003 已安装包与 Host 会话启动组合尚待后续正常 PLAN 小批次补齐，B004 不跨越其冻结范围；SessionActor/重放/迁移/总出口仍待 B005–B009。M1 ACTIVE，有效已完成5/11（45.5%，按数量计）；仅 Linux，Windows/macOS NOT_RUN。无新增 Owner 决策。

## 历史 B004 施工激活及此前状态（原文保留）

## 当前 Linux M1：B012 已完成并本机接收，B004 恢复施工

正常 PLAN v39→v40 仅 B004 BLOCKED→IMPLEMENTING；冻结摘要 `4b7d73a8d566d3097f3afbb45f8271e00d7aead7192992bba59974a717c08e31`、14 个冻结字段、原 depends_on、其他批次及 next_batch_sequence=13 全部不变。Owner 直接批准的 CHANGE-M1-HOST-CAPABILITY-REGISTRY 已完成：B012 独立业务验收绑定 `3a039cdb7f5637add74daae33a1c03bde3c5ad5b`，最终状态 `c67b019805df04358134b0a77d16a914001ad322` 经独立状态投影后已实际 fast-forward 接收至本机 main（清洁）；实际接收回执 `/tmp/trpg-m1-b012-linux-20261006/main-reception-c67b019/RECEPTION_RECEIPT.json` SHA256 `af92ca0a0a226aebb8c2d50b1f37e88ca0e3e2f28404ebf7df8cd011fcf361fb`，三份独立回执均按其精确来源保存。原始 Owner 授权 SHA256 `d0fc7adf81855c00c21d23d6c631e7a004c90ac1f64f7268a2e32c7388838214` 保持不变。

B004 的能力登记前置门禁现已满足，下一步生成有效 IMPLEMENT route 并在既有允许路径内实现版本化 Host API、实际模块来源和 Execution Token 绑定、MutationWorkspace 七类效果单事务提交及失败回滚、命名空间/命名数据库操作、资源预算和强制审计。此状态提交不含 B004 业务实现，不宣称其测试或验收 PASS。B001/B002/B003 前置均 COMPLETED；所有历史 FAIL、socket skip NOT_PASS、Windows/macOS NOT_RUN 与服务清理证据按原来源保留，不重标业务测试为治理 SHA。本轮继续仅 Linux；M1 ACTIVE，已完成有效批次5/11（45.5%，按数量计），总出口尚未满足。

## 历史 B012 完成候选及此前状态（原文保留）

## 当前 Linux M1：B012 独立业务验收 PASS，完成登记候选

正常 PLAN v38→v39 仅 B012 VERIFYING→COMPLETED，WIP=0；冻结摘要 `2540a4cac12d95789e3650d41567fc0704d0c8641ac91920c34d80df1697fdcd`、其余批次及下一序号13不变。B004 仍 BLOCKED，必须 B012 独立验收和本机接收完成后才解除阻塞。

Owner 批准的能力登记补充独立 PASS：精确业务 `3a039cdb7f5637add74daae33a1c03bde3c5ad5b` / tree `4e12bb843674880eadfd5e5d742237c8e8a38544`，回执 `/tmp/trpg-m1-b012-linux-20261006/independent-b012-3a039cd/ACCEPTANCE_RECEIPT.json` SHA256 `9a1ff305c01dfb35af83c0d7f6ec60a2212a200aea4ca9dbd556941d2a343b55`，39文件清单 SHA256 `9e5c4e92762b7c48562cf906e73d28aa21e0eff629e9229cbbe5ff54592417f7` 已逐条核验，unresolved_required_findings=[]。独立实际核心878 run/pass和外置mixed/Unicode51 run/pass，均0 fail/skip；父完整1159 run/pass和check/test/vet/license/ci actual0绑定3a清洁业务源。只有四允许路径变化、其余289 blobs/modes一致，既有七类名称补全与 Schema 边界保持默认零权限、三层交集和未登记名拒绝；不执行 Host 操作，无依赖/许可/主版本/公开字段或身份规则变化。

原 sandbox socket skip首次记录 NOT_PASS 与预提交 dirty600 保留，不重标；B003 cf业务/ab9本机接收、旧 FAIL、跨平台NOT_RUN、服务清理原哈希不改。本 v39 完成登记候选仍待独立状态投影和本机 main 接收，不能把3a业务测试重标本状态SHA；B012 main在本候选准备时仍PENDING。M1 ACTIVE、有效产品批次5/11（45.5%）按数量计，M1 总出口尚未满足，本轮仅Linux。接收后将正常 PLAN 解除 B004 的本项前置门禁，沿既有冻结合同继续 Host API 与原子事务，不新增公开语义。

## 历史 B012 VERIFYING 及此前状态（原文保留）

## 当前 Linux M1：B012 精确完整门禁通过，进入 VERIFYING

正常 PLAN v37→v38 仅 B012 IMPLEMENTING→VERIFYING，所有冻结字段/digest、旧 batches/序号不变；B004 仍 BLOCKED 等待 B012 独立验收与本机接收。获批七类闭合能力登记及 Schema 一致性实现仅四路径，业务精确候选 `3a039cdb7f5637add74daae33a1c03bde3c5ad5b` / tree `4e12bb843674880eadfd5e5d742237c8e8a38544`；其余 289 blobs/modes 及依赖、Manifest 字段/身份规则不变，默认 grants 与交集执行代码不改，无 Host 操作实现。

精确清洁 Linux attempt2 的 affected JSON 1159 run/pass、0 fail/skip，check/test/vet/license/ci 各 actual exit0；固定索引 `/tmp/trpg-m1-b012-linux-20261006/full-linux-3a039cd-attempt2/EVIDENCE_MANIFEST.json` SHA256 `6f7f182d1c380c8dfc2108fa0b467375527a172d5b65573af90bbd9930fa32d0`（13文件）。原 attempt1 的 sandbox special_socket skip/计数驱动exit1全部保留，不改成 PASS；attempt2 在自身临时 socket 环境实际执行通过，无新PG服务。预提交600 dirty-c07 cases 与 clean3a完整执行来源分开。

独立 ACCEPT 仍 PENDING；当前状态提交并非业务重跑或 B012 COMPLETED。B003 已本机接收 ab9bcd8，旧 FAIL 与跨平台 NOT_RUN 及清理不变。本轮仅 Linux；M1 ACTIVE、有效产品批次4/11（36.4%）按数量计，M1 出口未满足。

## 历史 B012 激活及此前状态（原文保留）

## 当前 Linux M1：Owner 已批准能力登记补充，B012 IMPLEMENTING

Owner 已直接批准 CHANGE-M1-HOST-CAPABILITY-REGISTRY（提案 SHA256 `5d486dd0e47d2d0deebcde066f45816a2197f2ec084598729c86a9bafa35a5e7`）；原文授权 `/home/zyc14588/.codex/visualizations/2026/10/05/01a10c5c-1d16-7a91-8c2f-1909e2af4f43/m1-host-capability-decision/OWNER_AUTHORIZATION.json` SHA256 `d0fc7adf81855c00c21d23d6c631e7a004c90ac1f64f7268a2e32c7388838214`。本次正常 PLAN activate，v36→v37，B012 IMPLEMENTING；只允许能力登记、Manifest 和 package Schema 一致性小批次，既有七类名补充不授予任何默认权限、不执行 Host 操作、不更换依赖、许可、公开字段或 API 主版本。B004 BLOCKED，冻结合同和前置列表保持，解除阻塞需 B012 精确验收与接收。

B003 Linux 业务独立 PASS 精确来源 cf364dc 保留，完成状态 fbe5532 与阻塞状态 ab9bcd8 分别独立状态投影通过；本机 main 已实际接收 `ab9bcd8c818cb9f3f79f6ac031caa516a4705ca3` / tree `4f5bd74cb86691b528ee0cd0cc58ed8f6659aa42`，固定接收回执 `/tmp/trpg-m1-b003-20261006/main-reception-ab9bcd8/RECEPTION_RECEIPT.json` SHA256 `5d250f3db0279dcfda2edd1fca06997402f911d5107f89d7fc152c0d36ecb70f`。旧 FAIL、Windows/macOS NOT_RUN 及服务清理记录不变，不重标旧测试来源。

M1 ACTIVE；增加真实序号 12 的小批次后有效产品批次为 4/11（36.4%）按数量计，B010 superseded 不重复计数，分母变化来自已批准补充工作。当前 B012 业务 NOT_RUN、此状态未作为业务 PASS；M1 总出口未满足。本轮仅 Linux，新的公共语义或范围决定仍需停止提交 CHANGE。

## 历史能力登记阻塞/此前状态（原文保留）

## 当前 Linux M1：Owner 已批准能力登记补充，B012 FROZEN

Owner 已直接批准 CHANGE-M1-HOST-CAPABILITY-REGISTRY（提案 SHA256 `5d486dd0e47d2d0deebcde066f45816a2197f2ec084598729c86a9bafa35a5e7`）；原文授权 `/home/zyc14588/.codex/visualizations/2026/10/05/01a10c5c-1d16-7a91-8c2f-1909e2af4f43/m1-host-capability-decision/OWNER_AUTHORIZATION.json` SHA256 `d0fc7adf81855c00c21d23d6c631e7a004c90ac1f64f7268a2e32c7388838214`。本次正常 PLAN freeze，v35→v36，B012 FROZEN；只允许能力登记、Manifest 和 package Schema 一致性小批次，既有七类名补充不授予任何默认权限、不执行 Host 操作、不更换依赖、许可、公开字段或 API 主版本。B004 BLOCKED，冻结合同和前置列表保持，解除阻塞需 B012 精确验收与接收。

B003 Linux 业务独立 PASS 精确来源 cf364dc 保留，完成状态 fbe5532 与阻塞状态 ab9bcd8 分别独立状态投影通过；本机 main 已实际接收 `ab9bcd8c818cb9f3f79f6ac031caa516a4705ca3` / tree `4f5bd74cb86691b528ee0cd0cc58ed8f6659aa42`，固定接收回执 `/tmp/trpg-m1-b003-20261006/main-reception-ab9bcd8/RECEPTION_RECEIPT.json` SHA256 `5d250f3db0279dcfda2edd1fca06997402f911d5107f89d7fc152c0d36ecb70f`。旧 FAIL、Windows/macOS NOT_RUN 及服务清理记录不变，不重标旧测试来源。

M1 ACTIVE；增加真实序号 12 的小批次后有效产品批次为 4/11（36.4%）按数量计，B010 superseded 不重复计数，分母变化来自已批准补充工作。当前 B012 业务 NOT_RUN、此状态未作为业务 PASS；M1 总出口未满足。本轮仅 Linux，新的公共语义或范围决定仍需停止提交 CHANGE。

## 历史能力登记阻塞/此前状态（原文保留）

## 当前 Linux M1：Owner 已批准能力登记补充，B012 PLANNED

Owner 已直接批准 CHANGE-M1-HOST-CAPABILITY-REGISTRY（提案 SHA256 `5d486dd0e47d2d0deebcde066f45816a2197f2ec084598729c86a9bafa35a5e7`）；原文授权 `/home/zyc14588/.codex/visualizations/2026/10/05/01a10c5c-1d16-7a91-8c2f-1909e2af4f43/m1-host-capability-decision/OWNER_AUTHORIZATION.json` SHA256 `d0fc7adf81855c00c21d23d6c631e7a004c90ac1f64f7268a2e32c7388838214`。本次正常 PLAN allocate，v34→v35，B012 PLANNED；只允许能力登记、Manifest 和 package Schema 一致性小批次，既有七类名补充不授予任何默认权限、不执行 Host 操作、不更换依赖、许可、公开字段或 API 主版本。B004 BLOCKED，冻结合同和前置列表保持，解除阻塞需 B012 精确验收与接收。

B003 Linux 业务独立 PASS 精确来源 cf364dc 保留，完成状态 fbe5532 与阻塞状态 ab9bcd8 分别独立状态投影通过；本机 main 已实际接收 `ab9bcd8c818cb9f3f79f6ac031caa516a4705ca3` / tree `4f5bd74cb86691b528ee0cd0cc58ed8f6659aa42`，固定接收回执 `/tmp/trpg-m1-b003-20261006/main-reception-ab9bcd8/RECEPTION_RECEIPT.json` SHA256 `5d250f3db0279dcfda2edd1fca06997402f911d5107f89d7fc152c0d36ecb70f`。旧 FAIL、Windows/macOS NOT_RUN 及服务清理记录不变，不重标旧测试来源。

M1 ACTIVE；增加真实序号 12 的小批次后有效产品批次为 4/11（36.4%）按数量计，B010 superseded 不重复计数，分母变化来自已批准补充工作。当前 B012 业务 NOT_RUN、此状态未作为业务 PASS；M1 总出口未满足。本轮仅 Linux，新的公共语义或范围决定仍需停止提交 CHANGE。

## 历史能力登记阻塞/此前状态（原文保留）

## 当前 Linux M1：B003 验收通过，B004 因公共能力登记门禁阻塞

正常 PLAN v33→v34 仅将 B004 IMPLEMENTING→BLOCKED，WIP=0；全部冻结字段、合同摘要、其他 batches、墓碑和下一序号 12 不变。B004 只进行了只读准备，业务 NOT_RUN。发现 `internal/package/capability/capability.go` 的闭合 v1 登记只接受 state/event/log，并要求新增名字显式变更合同；现有 Host API 规范另列 random/time/content/db/task/ai/rules，B004 的冻结 DB/task 原子验收需要该上游登记，而 B004 scope 不含 capability/Schema 路径。

CHANGE-M1-HOST-CAPABILITY-REGISTRY 已提出，Owner 决定在本状态候选写入时 PENDING。固定提案 `/home/zyc14588/.codex/visualizations/2026/10/05/01a10c5c-1d16-7a91-8c2f-1909e2af4f43/m1-host-capability-decision/CHANGE-M1-HOST-CAPABILITY-REGISTRY.md` SHA256 `5d486dd0e47d2d0deebcde066f45816a2197f2ec084598729c86a9bafa35a5e7`，发现基准 `fbe55322fe1d7a063e2f56b0379f39f61d268e09` / tree `5344c73bb3a2e40700f2e577f7b1e327d105a5cc`。推荐批准后正常 PLAN 分配下一真实小批次补齐规范已有七项登记及 Schema/授权一致性，独立验收和接收后再解除 B004 阻塞。未分配 B012，未修改登记、Schema、B001 已完成合同或 B004 冻结依赖，未降低门禁/默认权限；未获决定不施工。来源为自主规划公共合同门禁及 CHANGE_CONTROL 停止协议。

B003 业务独立 Linux PASS 仍绑定 `cf364dcec1d2b61afa12b4a2c79e58e8a338936d` / tree `bcf7957356588fcf325c6252e627fb7469e99c63`，回执 SHA256 `1386f3f2c2d50003c6f75f1450a43ffe612113a752f5077caee6624ba666be18`；732/37 实际 run/pass、0 fail/skip，原 FAIL 与跨平台 NOT_RUN 均保留，不重标业务测试来源。完成/激活状态 `fbe55322fe1d7a063e2f56b0379f39f61d268e09` 的独立状态投影 PASS 回执 `/tmp/trpg-m1-b004-linux-20261006/independent-activation-fbe5532/ACCEPTANCE_RECEIPT.json` SHA256 `e220f1ec4b2d702838c8046eb30179711edbba0cf214b48f0901011bcc5da7ea`，28 文件原哈希已核对；该 PASS 不授予公共合同变更或 B004 施工许可。

B003 自建 PostgreSQL 服务已按原容器 ID/标签/镜像核验、停止、删除并回读不存在；清理回执 SHA256 `3cf769c92d9bd41de69b71eaf4ff230f6c28e9e994520ebb3f133641d7d2d66a`。本 v34 状态候选仍待独立状态复核和本机 main 接收，业务及依赖字节不变。本次仅 Linux，M1 ACTIVE、有效产品批次 4/10（40%）按数量计，M1 总出口未满足；Windows/macOS 延期 NOT_RUN，V1 后续责任保留。

## 历史 B003 完成及 B004 激活状态（原文保留）

## 当前 Linux M1：B003 完成登记，B004 激活候选

B003 的独立 Linux 验收已 PASS，精确业务候选 `cf364dcec1d2b61afa12b4a2c79e58e8a338936d` / tree `bcf7957356588fcf325c6252e627fb7469e99c63`。回执 `/tmp/trpg-m1-b003-20261006/independent-b003-linux-cf364dc/ACCEPTANCE_RECEIPT.json` SHA256 `1386f3f2c2d50003c6f75f1450a43ffe612113a752f5077caee6624ba666be18`；原 002/004/005 的处置只适用于回执指定后继，原 FAIL 保留；003 和五个 Windows/macOS 行仍延期 NOT_RUN，非平台 PASS。完整新门禁 check/test/vet/license/ci exit 0，受影响 JSON 732 run/pass、真实 PostgreSQL JSON 37 run/pass，均 0 fail/skip；37 条实际 runner 身份/生命周期证据与原生 Creator 六阶段保留。证据索引 SHA256 `d2da1a8f5f5c7e43d83a671e85619976f8e91ec01e5a71029355b737d6825f91`，父封存包 SHA256 `d7771e4888c13145f91f166936d97ad3b37471dc93247892f9c9cc1fe87f6f39`。

目录治理独立 PASS 回执 `/tmp/trpg-m1-b003-20261006/independent-linux-catalog-02ed8ce/ACCEPTANCE_RECEIPT.json` SHA256 `622b7e0ebb97df7e932bbc45cadb9896ff600353ab5a0aa3dec3b8ede62b1b40`；Linux scope 治理 PASS 回执 SHA256 `49f631644c305480aa5fd5a1fee5cd39f423b5e74295570f26e413858990a85a`。该状态提交只在正常 PLAN 将 v32→v33、B003 VERIFYING→COMPLETED、B004 FROZEN→IMPLEMENTING，并更新两摘要；所有业务/依赖字节、冻结合同、其他 batches/墓碑/序号不变。B004 digest `4b7d73a8d566d3097f3afbb45f8271e00d7aead7192992bba59974a717c08e31`，其前置 B001/B002/B003 已完成，WIP=1；本候选尚未独立状态验收或 main 接收，B004 业务仍 NOT_RUN，fresh IMPLEMENT/check 与接收门禁通过后才开始施工。

M1 有效产品批次按完成数量计为 4/10（40%）；B010 为已被 B011 替代的历史实现，不重复计数。本次仅 Linux；M1 总出口尚未满足。下一实现为已冻结 B004 的 Host API、生产 callback 桥接、MutationWorkspace 全 effects 原子事务、分级数据库能力、预算和不可关闭审计，沿既有合同实施，不新增公开语义。新的范围/设计/公共合同/依赖/许可决定仍停止交由 owner。

## 历史 B004 冻结与 B003 验收投影（原文保留）

## 当前 B004 规划冻结候选（尚未开工）

本次正常 PLAN 将 M1/v31→v32、未开始 B004 PLANNED→FROZEN；首个冻结摘要 `4b7d73a8d566d3097f3afbb45f8271e00d7aead7192992bba59974a717c08e31`。B003 仍 VERIFYING，当前业务独立验收待回读，B004 的三个前置依赖和 WIP=1 不变；冻结不释放施工或宣称前置完成。其他 batch、已冻合同、墓碑、序号与业务字节全部保持。

B004 原 objective/requirements/acceptance/tests/non_goals/stop_conditions 和公共 Host API 语义保持。唯一工程路径细化是在未开始合同的 allowed_scope 增加 `internal/luaruntime/profile/**`，并增加 `SPEC-M1-LINUX-ACCEPTANCE-SCOPE` 阅读绑定。实际源码证明：`profile.Engine.runtime` 私有，New 只注册 print/require 等内部函数，没有数据型 Host Callback 注册接缝；`profile.Config` 只有 limits/modules，ipc.Serve 仅 initialize/state/execute/destroy；因此既有双向 IPC 回调义务需要在该引擎加最小内部桥接。固定 golua v2.0.5 的 GetFrameInfo/compiled Proto.Source 可用于 Go 侧校验真实模块来源；生产 Lua 的 debug 库继续禁止，不暴露 VM 指针或凭据，不重开 B002 合同、不削弱其既有 Profile/预算/检查点门禁。

B004 内部闭环：固定包/Host API 主次版本和 required entries/capability 验证 → 每次 command 的 opaque token 绑定 Session/workspace/包/hash/真实 module → 独立 runner 双向 IPC → 有界、只读自有副本的 MutationWorkspace → schema-bound 私有 namespaced get/put/delete/list/CAS 或经过审查的受信 named relational operations → Go 最终校验 → 单 PostgreSQL 事务提交 state/package-data/event/idempotency/task/continuation/outbox → 返回完整已提交结果。任何脚本、callback、预算、取消、验证、runner、DB 或审计失败须丢弃全部 staged effects，并销毁污染 VM；没有提前广播或客户端可见成功。

实施顺序为 callback/binding/budget/audit 数据型合同与单元 corpus、生产引擎桥接及 IPC、workspace/数据库能力分级与真实 PostgreSQL repository、命令级 commit/rollback、受影响回归/真实服务/独立验收。未实现符号统一 PLANNED_NEW：`internal/hostapi/` workspace/execute/policy/audit，`profile` 的数据型 callback 桥接，ipc/vm 适配，`internal/storage/postgres/` command repository，`tests/integration/hostapi/`。现存可复用的是 canonical archive/manifest/capability、checkpoint.Value、profile.Limits/Engine、ipc.Client/Serve 和 vm.Session；不复制旧分支产品代码。

风险与约束：执行令牌不得进入 Lua/日志/checkpoint；模块来源来自 runner 原生受控编译信息，不能取脚本自报字段；限制 callback/recursion/patch/rows/bytes/event/task/output 的硬上限，AUDIT-0 不可关闭；named operation 只接受 operator-reviewed ID/typed inputs，SQL和运行时 DDL 全拒绝；不允许 token/能力句柄跨 Session/module 生效。最终一致性以数据库 commit 为准；unknown acknowledgement 须通过同一 workspace/command identity 回读，不能盲重放 Lua。

必须证据沿冻结合同：TEST-LUA-003 全 effects 的成功原子提交及每个阶段失败回滚；TEST-LUA-004 真实 PostgreSQL 私有 namespace/受信 named ops/SQL-DDL/跨包跨 workspace 否定矩阵；TEST-LUA-006 实际生产 runner 的 token/session/module/预算/审计脱敏/污染重建；hostapi race；全部受影响 Lua/B003/Creator/package 回归和 canonical check/test/vet/license/ci。每组记录精确 candidate SHA/tree、native Linux amd64、实际 argv/cwd/退出/非零 named cases、服务与 runner/binary identity、原始日志哈希。Windows/macOS 按 owner 本次 M1 平台授权延期 NOT_RUN；所有 Linux 必须项保持。

交付物是上述 allowed 路径中的内部合同与实现、真实 integration corpus、签名候选和外置证据/Handoff；后者不等于独立 PASS。关键未批准依赖、公开 Host API/包格式变化、原子性不能保持或任何禁止目录需求仍触发原 stop/CHANGE。当前无新的 owner 产品决策，下一 gate 是独立核验冻结规划和 B003 完成接收；两者通过后正常 PLAN 激活 B004，fresh IMPLEMENT route/check 才开工。

## 历史 B003 验收及 Linux 范围投影（原文保留）

## 当前 B003 Linux 独立验收中（PLAN 生命周期候选）

本 tree 仅把 M1 plan v30→v31、B003 IMPLEMENTING→VERIFYING，标记已实现业务进入当前独立验收；业务候选 `df1fa6793317cf26660ee92803e63eaf5e7a254a` / `5d517c1b949a3e785629bb8563304bded0ed97ac` 不变。Scope 治理候选 `9fc76428e3f227c3337b32f4edd3e6077d47eb54` / `b82b9d5da5e1ff429c63c2ed26fe43b5889ce0e9` 已通过父 check/license 和 164 named projectctl run/pass（0 fail/skip），独立 scope 与 Linux 业务结论仍 PENDING。本生命周期不是业务 PASS 或 main 接收。

冻结字段/摘要 `59e0456c8ed261f08b1d1211fbdd1436cc50f4e875f36482732fe84e39d5167e`、其他 batches/依赖/墓碑/序号全部不变，B004 仍 PLANNED，active VERIFYING 仅 B003，WIP=1。Owner 授权与 Linux required/延期规则仍由 `SPEC-M1-LINUX-ACCEPTANCE-SCOPE` 补充约束；原 FAIL、003 历史 OPEN、所有原平台矩阵与原始业务证据字节保留。新独立 PASS 和合法完成登记前不释放 B004。

## 历史 Linux 范围登记投影（原文保留）

## 当前 M1 Linux 验收范围登记（治理候选）

Owner 直接指示“授权，但本次M1暂时不涉及多平台部分，先完成linux平台内容”；当前任务是继续完成 Linux M1，遇到新的范围/设计决策再停下。规范入口为 [M1 Linux 验收范围](../../docs/00-governance/M1_LINUX_ACCEPTANCE_SCOPE.md)，`SPEC-M1-LINUX-ACCEPTANCE-SCOPE` / `GOV-M1-LINUX-ACCEPTANCE-SCOPE`。Windows/macOS 本次延期、NOT_RUN，不记录为 PASS；V1 兼容矩阵和后续 RC/Stable 门禁保留。该段须由每个 M1 route 的 always-read 摘要带入。

当前工作线的业务被测身份为 `df1fa6793317cf26660ee92803e63eaf5e7a254a` / tree `5d517c1b949a3e785629bb8563304bded0ed97ac`。Linux 本地 `just check/test/license-check/ci` 与 `go vet ./...` 均 exit 0；affected Go JSON 638 run/pass、真实服务 JSON 37 run/pass、两组均 0 fail/skip（计数包括 named parent/subtests）；前端 Web Player 1/1、Creator 8/8，并有固定候选原生 Creator 的 import/inspect/edit/validate/export/reimport 及通用扩展往返证据。原始证据 index `/tmp/trpg-m1-b003-20261006/candidate-df1fa67-attempt1/EVIDENCE_INDEX.json` SHA256 `f5a35e4745f87463315b9eba15bd6bdc9f6625d345a74e784a596fe2b5861323`；环境读回 SHA256 `da4bfadcddaeb67a0f9601a4f84164a94c1bf65993237ccba5099db3dae42656`。

旧范围下独立后继 receipt `/tmp/trpg-m1-b003-20261006/independent-b003-df1fa67/ACCEPTANCE_RECEIPT.json` SHA256 `8595f592e84eda776a57bf6281ca260612c065b5d20e810dcd203a275d9b6a50`，仍为总体 FAIL / 本地修复 PASS；原首轮 receipt SHA256 `cc43cbfbf30996840881f96cec8d0a8b7b36fe7181855db873f415b72fe311e6` 原文保留。002/004 仅对该精确后继 CLOSED_FOR_VERIFIED_SUCCESSOR；003 原 OPEN_REQUIRED_NATIVE_EVIDENCE 保留，新 Linux 范围处置为 DEFERRED_BY_OWNER_M1_LINUX_ONLY、Windows/macOS NOT_RUN，非全局关闭。19 行原映射仍逐字保留在下方历史规划投影。当前 Linux 必须项为 I01、I03—I08、R01、R02、C01、C04；I02/C02/C03/C05/C06 按 owner 延期；S 行仍是原补充/下游义务。

可持久恢复的原始证据包 `/home/zyc14588/.codex/visualizations/2026/10/05/01a10c5c-1d16-7a91-8c2f-1909e2af4f43/m1-b003-native-evidence-decision/M1-B003_EVIDENCE.tar.gz` SHA256 `35f5d6e26b6ba6f142507e7d18c6910ac5f42ef935e06d87bb30dbc1abe0cba6`；授权与 CHANGE 在同级 `m1-linux-first/`。若 /tmp 消失，按封存包中的原路径映射恢复并核验哈希，不能重写原 receipt 以制造新鲜执行。

本提交只有四路径治理 scope delta；业务与依赖字节、M1/v30 ACTIVE、B003 IMPLEMENTING、frozen digest `59e0456c8ed261f08b1d1211fbdd1436cc50f4e875f36482732fe84e39d5167e`、其他 batch 和 next_batch_sequence=12 不变。治理 SHA/tree 由签名 Git object 和新 route/独立报告固定，与旧业务被测身份分列。当前尚未有新 Linux 独立业务 PASS，不自报 B003/M1 完成或 main 接收；当前 main authority 仍为 `a353b2da35d20eb162d1eb404ac33fc0a3eacfaf`。

下一 gate：独立验收本治理 scope delta，再独立验证 Linux B003 的完整必须项；通过后正常 PLAN 登记 VERIFYING→COMPLETED，再推进依赖满足的 B004，WIP 上限仍为 1。下方“本轮/当前/无业务候选/无后续授权”等句仅属历史阶段；本节及当前直接 owner 授权给出本次任务范围，不回写原记录。

## 历史 B003 实现启动投影（原文保留）

## M1-B003 实现启动治理候选

已接收冻结基线为 `7c6240e28c1ff4066debedffb43284b979dee260` / tree `c3a0f11cf2c99d1bfb6ec6d57c95d5016d7c3e8c`，M1/v29、B003 FROZEN；本机规划接收回执 `/home/zyc14588/engineering/codex-loop-state/owner-receptions/M1-B003-plan-freeze-4bd29bbb-a1/ACCEPTANCE_RECEIPT.json` 的 SHA256 为 `f303229da35630a25629fd935b7964ddba115fcefcec6c77902320ba3f925212`。原首轮 FAIL 与 `ACC-M1-B003-001 = CLOSED_FOR_VERIFIED_SUCCESSOR` 的后继处置均保留。

本 tree 仅作已授权 PLAN 生命周期转换：M1/v29→v30、B003 FROZEN→IMPLEMENTING。原生 `validateMilestonePlanChange` 要求 revision 加一；14 类冻结合同字段及摘要 `59e0456c8ed261f08b1d1211fbdd1436cc50f4e875f36482732fe84e39d5167e` 不变，下方 19 行平台/证据矩阵逐字保留。B001/B002 依赖已完成；其他 batch、依赖/并行字段、墓碑与 next_batch_sequence=12 不变，B004 PLANNED、B012 未分配；候选 active batch 仅 B003，WIP 上限 1。

本候选的精确 SHA/tree 由签名 Git object、fresh PLAN route 和独立治理报告固定。owner 本轮授权只允许独立验证通过的最小激活 delta 本地 ff-only 接收；沿用 `trpg-owner-local-batch-acceptance/1` 的 owner-local 惯例，stage 为 `IMPLEMENTATION_START`、scope 为 `B003_GOVERNANCE_IMPLEMENT_ACTIVATION_ONLY`，不是新增原生 schema。实际接收结果、intent、命令退出和生效读回见 `/home/zyc14588/engineering/codex-loop-state/owner-receptions/M1-B003-implementation-start-7c6240e-a1/`；本摘要不预告接收 PASS，也不替代实际 main/ref 与回执。

仅在激活接收与读回完成后，从实际生效 SHA/tree 创建独立 Builder worktree 和全新 IMPLEMENT/builder 上下文，生成 fresh route/check 与 `B003_IMPLEMENTATION_START_BINDING.json`。Builder 仍受原 allowed/forbidden scope 约束，不得修改 `.codex/**`。当前无 B003 业务候选，业务验收 `NOT_RUN_NO_BUSINESS_CANDIDATE`；业务合入、后续生命周期接收、B004、B002 重开、B012、旧 c1 及远端写入均不在本轮授权内。

## 历史 v29 规划冻结投影（原文保留）

以下“本轮”“当前”“未接收”“下一 gate”均指原规划冻结阶段；冻结合同、接口与 19 行测试映射继续适用，当前启动身份以上方实际基线及外置接收读回为准。

## M1-B003 原生 PLAN / planner 冻结候选（本轮）

### Authority 与合法改动

`CURRENT_MAIN_AUTHORITY = 4bd29bbb04aa951d91fc743fa075d1c794a780e5` / tree `ff2b7a9d404379d4c03f35fc941081831fa5ecd7` / `M1 ACTIVE v28`。已重新核验本地 `refs/heads/main`，不是锚点分支 HEAD。当前 B003 为 `PLANNED / UNFROZEN`，无 frozen_contract_sha256；未冻结合同规范化摘要 `bc75a0f7d2424e87efa933cfb816bf6dab00aa4ab4cc118e3fcf5c5182ccd176` 仅为比较指纹，不冒充已冻结值。

`PLANNING_CANDIDATE_AUTHORITY = 本 tree 的 M1/v29, B003 FROZEN`，拟冻结摘要 `59e0456c8ed261f08b1d1211fbdd1436cc50f4e875f36482732fe84e39d5167e`。具体 candidate SHA/tree 由本提交 Git object、fresh route 与外置独立报告绑定，避免自引用。`POST_RECEPTION_EFFECTIVE_AUTHORITY` 只有 owner 批准并完成精确 ff-only 接收及回执回读后才等于该候选。`FUTURE_IMPLEMENTATION_IDENTITY = UNALLOCATED`；后续须从实际接收的 SHA/tree 生成 fresh route，按原生 PLAN 合法完成 FROZEN→IMPLEMENTING 生命周期步骤，并另获 Builder 启动授权。不能在本轮伪造最终实现 source、epoch、lease、cycle 或 attempt。

机器合同唯一来源是 `.codex/state/MILESTONE_PLAN.yaml` 的 `M1-B003` 条目；本摘要按现有 BATCH_CONTRACT/HANDOFF 模板补充实施步骤、风险、测试映射和交接导航，不是第二份可覆盖机器合同的冻约。PLAN 层本轮 exact changed-file allowlist 为 `.codex/state/MILESTONE_PLAN.yaml`、`.codex/state/MILESTONE_STATUS.md`、`.codex/state/PROJECT_SNAPSHOT.md`；独立生成的 `.codex/runtime/READING_MAP.yaml` 不入 Git。B003 的未来业务 allowed/forbidden_scope 原文保留；其中禁止 `.codex/**` 是 IMPLEMENT 边界，不禁止本次获授权 PLAN 修改原生治理状态。

`EXISTING_AUTHORITY`：保留 B003 objective、requirements、acceptance、tests、stop_conditions、全部 allowed/forbidden scope、depends_on、并行字段。仅增加上述已存在规范章节 `SPEC-PACKAGE-TRUST-001, SPEC-MIGRATION-001, SPEC-RELEASE-GATE-MATRIX, SPEC-CREATOR-POSITIONING, SPEC-CREATOR-ROUNDTRIP` 的阅读绑定，补齐签名/迁移前检/平台/Creator 引用，不新增产品语义。按 `validateMilestonePlanChange` 与 `validBatchStateTransition` 从 v28 递增一版并作 PLANNED→FROZEN；未硬编码下一业务 revision。其余十个 batch、墓碑和 next_batch_sequence=12 均原样。B003 独占 REQ-PACKAGE-005；WIP 上限 1、active IMPLEMENTING/VERIFYING 为 0。FROZEN 不是开工。

`EXISTING_AUTHORITY`：B002 已完成。接收回执 `/home/zyc14588/engineering/codex-loop-state/owner-receptions/M1-B002-473c941-business-acceptance-a1/ACCEPTANCE_RECEIPT.json` SHA256 `c727815a4bf3b2af2abbf7108fce3819321a2fec6b98ca665f7230dc31eda572`，POST_ACCEPTANCE_NATIVE_READBACK SHA256 `1f9d3a47ec3e7e64ebdded98c6a8e927c7bbf14e3ff8123b4bd9674d99b677ed`；原业务测试 identity `473c94190b4eb9e203ab4ea823eaf72829960ad9 / ea87972af11c8c5956c116b58ff946291a0b3bdf`，治理完成 identity `4bd29bbb04aa951d91fc743fa075d1c794a780e5 / ff2b7a9d404379d4c03f35fc941081831fa5ecd7`，独立治理报告 SHA256 `ccdc83bba69c175d55129acc238c97642c1354657a0bc8d20653eba7db728fe6`。证据是原业务验收加无业务变化的独立治理验证；原 CI 未直接测试治理 tree。B002 frozen digest `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677` 保留。010 事件 SHA256 `2505adafa61d1ed9bb402d17d0d6da068731956adc0f2f8f1b8101aa9fb7a969` 已消费，复用回读、不重发；008/009 历史 HIGH/OPEN 保留，原精确业务候选 disposition PASS 不变。

### 范围、接口与实施闭环

以下表中 `EXISTING_AUTHORITY` 是已存在规范义务，`VERIFIED_EXISTING_INTERFACE` 是本轮源级核实，`PLANNING_DERIVATION_WITHIN_SCOPE` 是可调整的内部实现规划，后者不新增公共 API、包格式或冻约义务。每个新路径均位于原 B003 allowlist。`PLANNED_NEW` 只描述入口尚未实现。

| 环节 | 来源类型与依据 | 实际路径 / 符号或 PLANNED_NEW | 输入、输出、权限和失败边界 |
| --- | --- | --- | --- |
| 输入 | VERIFIED_EXISTING_INTERFACE；B001/B011 | `internal/package/archive/model.go`: Import/ImportBytes/FromFiles, Package.Manifest/ExactLock/ContentHash/ArtifactIdentity/Entries/Entry；`snapshot.go`: Snapshot.Bytes/Hash；`project.go`: ImportProject | Creator/CLI 先产出标准 immutable ZIP + META-INF envelope；B003 消费有界快照和本地精确依赖工件集合，不把用户目录路径当安装目标。Bundle 是分发容器，须由上层分解为 package 工件，不伪造运行时 bundle。 |
| 隔离暂存与安全 | EXISTING_AUTHORITY；B003 acceptance[0]、SPEC-PACKAGE-INSTALL/STORAGE；PLANNING_DERIVATION_WITHIN_SCOPE | PLANNED_NEW `internal/package/install/staging.go`, `validation.go`；复用 `archive.Import`→reader.readSnapshot→preflightZIP32（后两者内部，不从安装器直接调用） | 服务端自选临时根；只读取不可变限长副本，不向受管 active 工作区解包。沿用现有 ZIP/path/schema 上限，额外安装策略拒绝 bytecode/未声明二进制等原合同 corpus；通过 archive parser 不等于已完成全部安装校验。 |
| 身份和依赖 | VERIFIED_EXISTING_INTERFACE；B001/B011 | `manifest/artifact.go`: BuildArtifactIdentity/ArtifactIdentity.Digest；`dependency/lock.go`: ParseExactLock/BuildExactLock/Packages/CanonicalJSON/Digest | package_id、version、content hash、build provenance、rights、features/transitive exact lock 相互核对。缺依赖/重复版本/循环/换字节均拒绝；不联网求解最新版本。归档 hash、content hash、artifact digest 分列，不能以 ZIP hash 替换包 content identity。 |
| 权利、签名和能力 | EXISTING_AUTHORITY：R2-A11/A12/A14/A15、SPEC-PACKAGE-TRUST-*；VERIFIED_EXISTING_INTERFACE | `manifest.Package.Rights/Capabilities`、`capability.Resolve`；PLANNED_NEW `internal/package/install/policy.go` | 验证调用方 workspace 授权和可信策略来源，再验证实际签名证据与精确包/锁/Profile/Host API/套件绑定；包自报 trust 或 PASS 不可信。发布者签名不替代认证签名；撤销状态阻止安装。只允许规范允许的明确开发/CI 未签名上下文；本批次不构造私人房间确认 UI。清单权利字段有效不等于许可批准；未满足策略 fail closed。签名算法/证据容器是内部策略接缝的实现细化，不能新增公共包签名格式或签名服务。 |
| 生产 Profile 测试 | EXISTING_AUTHORITY：SPEC-PACKAGE-INSTALL 明确列出生产 Profile 测试；VERIFIED_EXISTING_INTERFACE | `vm/vm.go`: Options/New/Session.Execute/Destroy；`vm/package.go`: FallbackProof；`ipc/client.go`: Start/Call/Kill；`profile/profile.go`: ID/RuntimeVersion/ValidateSource/Limits/Failure/Code；PLANNED_NEW `internal/package/install/runtime_validation.go` | 必须在独立 runner 执行适用的生产 Profile 测试；静态 source 检查不能充当执行 PASS。可启动的 game-system 使用现有 vm.New 临时验证实例，非 Session-startable 包由安装测试适配层用同一 B002 IPC/production profile 隔离检查模块和显式测试输入；无脚本资产包记录该分支不适用及结构测试，不跳过整个 gate。库不能借测试开启正式 Session。没有 B004 回调时必需能力 fail closed；可选 fallback 证据必须实际执行并绑定 hash，不伪造 FallbackProof。 |
| B002 生命周期与错误 | VERIFIED_EXISTING_INTERFACE；SPEC-LUA-RUNTIME-VM/GLOBALS/BUDGET | `vm.New/Execute/Destroy`；`ipc.Start/Call/Kill`；`profile.Failure` 与 `Code`；`ipc/frame.go`: ErrRunner/ErrProtocol | 使用原 source-only、空主机能力、Linux 隔离边界、有限 CPU/指令/墙钟/内存。保留 SOURCE_REJECTED、SCRIPT_FAILED、BUDGET_EXCEEDED、VM_POISONED、VM_DESTROYED、CAPABILITY_DENIED、CONFIGURATION_REJECTED、cancel/deadline 的实际分类；IPC transport error 单列，不靠通用 Code 抹平原因。失败后销毁/reap，不重用污染实例。安装器保存阶段和安全错误码，不输出凭据或 VM token。B002 源码和语义均不修改。 |
| 数据迁移预检 | EXISTING_AUTHORITY：B003 acceptance[1]、SPEC-MIGRATION-*；PLANNING_DERIVATION_WITHIN_SCOPE | PLANNED_NEW `internal/package/install/preflight.go` 和 `internal/storage/postgres/` 的初装元数据 schema | 明确执行 fresh install 的兼容/数据目标预检：没有现存受影响状态时记录 no-op 理由，存在要求但无法证明安全的迁移则拒绝。本批次不执行包升级、Session migration 或任意包 DDL。若某真实包必须新增迁移语义/公共声明字段才能初装，触发原 stop/CHANGE gate，而不是默认成功或实现 B006。 |
| 不可变对象 | EXISTING_AUTHORITY：SPEC-DATA-OBJECT/TENANT；PLANNING_DERIVATION_WITHIN_SCOPE | PLANNED_NEW `internal/storage/object/`、`internal/package/store/` | 最小真实 backend 可用规范允许的本地目录；根由 operator 配置，服务进程独占写权限，不取包内路径。hash-keyed 不可变对象先落稳并复核，读取经过 workspace 元数据授权；对象 key 本身不授予权限。物理复用允许，ownership/rights/visibility/retention/grants 不复用。S3-compatible 是存储抽象后续可实现 backend，本轮不强加托管部署。 |
| 原子可见性 | EXISTING_AUTHORITY：B003 acceptance[1-3]；PLANNING_DERIVATION_WITHIN_SCOPE | PLANNED_NEW `internal/package/install/install.go`、`internal/package/store/`、`internal/storage/postgres/` | 一次 PostgreSQL 事务提交 install index、workspace ownership/rights/权限及完整对象引用作为唯一可见性线性化点；此前对象仅私有暂存/无可访问元数据。提交前确保所有对象存在且不可变。禁止声称 PostgreSQL 与对象存储天然跨系统原子；以先落稳对象、再原子发布引用实现合同观察语义。 |
| 失败、取消和重试 | EXISTING_AUTHORITY：B003 acceptance[2]；PLANNING_DERIVATION_WITHIN_SCOPE | 同上 + PLANNED_NEW `tests/integration/package_install/` | 提交前任何失败回滚元数据/grant，清理本次独占 staging；不得删除其他安装/并发赢家共享对象。crash 遗留不可见暂存可隔离回收，不是活跃 metadata。commit acknowledgement 丢失先按 workspace+精确 artifact/请求身份查询事务结果，已提交返回同一完整安装，未提交重试；不盲删或重复 grant。竞争不同内容不覆盖既有 immutable artifact。同 content 不同 provenance/版本的 identity 不混同。 |
| 可用结果与下游 | PLANNING_DERIVATION_WITHIN_SCOPE；B004 既有 depends_on | PLANNED_NEW `internal/package/store/` 的 workspace-scoped 查询结果与 `cmd/platformd/` 内部组合入口；现有 platformd/main.go 仍仅 baselinecli.Run | 内部结果含 workspace、artifact/content/lock identity 和已提交状态。仅 commit 后可读取，Player/Creator 以后经各自正式上层接口消费；本批次不新增 OpenAPI、UI、房间选择或自动启动 Session。B004 消费已经安装的不可变包和受管存储接口，不反向为 B003 提供 Host API。 |
| Creator 与通用扩展往返 | VERIFIED_EXISTING_INTERFACE；SPEC-PACKAGE-EXTENSIONS-M1-B010、SPEC-CREATOR-ROUNDTRIP | `apps/creator-studio/creator/service.go`: ImportArchive/Inspect/Edit/Export；`archive.Package.Export/ReplaceExtension`；`extension.Load` | Installer 复用 canonical package，不添加专用 manifest/内部私货。原 v1 无扩展、v2 required/optional、opaque optional raw bytes、第三方路径一致。Creator 编辑产生新 immutable hash，经正常输入再安装；不回写已安装对象。I07/R01/R02 验证原 model 到存储再 reload/export 的保留。 |

安装闭环（箭头是顺序，数据库提交前无外部可见安装）：

```mermaid
flowchart LR
  A[标准 package 工件与精确依赖] --> B[隔离暂存与 archive 安全]
  B --> C[Schema / identity / lock / rights / signature / capability]
  C --> D[独立 B002 runner 生产 Profile 测试]
  D --> E[数据迁移预检]
  E --> F[不可变对象落稳与复核]
  F --> G[PostgreSQL 一次可见性提交]
  G --> H[workspace 授权后的安装结果]
```

`PLANNING_DERIVATION_WITHIN_SCOPE`：未来实施步骤依次为安全 corpus/输入适配、可信安装策略与 production-runner 适配、真实对象与 PG store、初装 preflight、原子提交及不确定结果恢复、平台/租户/B011 回归证据。先完成 targeted/affected tests，再固定业务 candidate，完整 required suite 由独立 ACCEPT 执行；同一精确候选已有新鲜完整证据可复用，不能重复跑后混合计数，也不能删 gate。

`EXISTING_AUTHORITY`：依赖方向（前置→后继）B001→B002；B001/B002→B003；B001/B002/B003→B004；B001→B011。完整 graph 以机器 plan 为准，本轮不加边，无环；B011 已完成接口是共同 baseline 约束，不创建 B003↔B004 循环。B003 不是第二个 runtime/Host API 所有者。

`DEFERRED_OUT_OF_SCOPE`：远端下载/仓库、发布和认证签名服务、自动升级、卸载、迁移执行、热更新、完整 Creator/UI、Host API/B004、Room/Session、正式 revocation 运营、生产部署/备份。只做相关策略拒绝/预检和内部接缝。`OWNER_DECISION_REQUIRED = NONE`（本候选不触及新产品决策）；后续若发现必须新增公共格式/能力/签名规则/迁移语义/关键未批准依赖才能满足原合同，应带源级证据集中提交 CHANGE；不能靠本摘要静默授权。当前没有经独立复现的 B002 上游缺陷。

### 测试、平台与执行环境（冻结前映射）

`EXISTING_AUTHORITY`：REQ-PACKAGE-005→TEST-PACKAGE-005，来源 REQUIREMENTS/TEST_CATALOG/TRACEABILITY 的 M1 行和 R17-A06/A07；完整五条 acceptance、四条 tests、四条 stop_conditions 均在机器 plan 中冻结。`PLANNING_DERIVATION_WITHIN_SCOPE`：I01-I08 是上述合同案例分解，R/C 行是现存受影响兼容门禁，S 行仅补充或下游，均不分配新 TEST ID。所有业务执行状态为 `PLANNED_NOT_RUN`，不存在的目录不是本轮测试失败。

- E0：保留 command argv/cwd、开始结束时间、exit code、环境/平台实测 identity；可复现日志原文与 SHA256。复用必须绑定同一 exact candidate tree、未变依赖和完整 required case 集。
- E1：另含 exact candidate commit/tree、B003 frozen digest、test ID/case 名、run/pass/fail/skip/NOT_RUN 数；过滤/零执行/跳过不得假充 PASS。构建 driver 和被测 source identity 分列。
- E2：另含临时 PostgreSQL 与真实对象 backend/version/配置摘要（无秘密）、隔离 namespace、注入故障点、安装前后可见 metadata/permissions/object manifest、事务结果及重启/重试证据。真实目录 backend 不是 Mock；S3 若未来在本批次实现，该 backend 同样必须跑 I04-I08，不能只用本地 backend 证明其正确。
- E3：另含 Creator binary SHA256、嵌入 commit/tree、版本、build command 和真实 edit/validate/export/reimport/repeat 结果；不复用旧 binary identity 冒充新候选。

E-LINUX：本机是 Linux 环境；后续测试须记录实际 GOOS/GOARCH、locked Go/Node/pnpm/Just/Wails，确保 native Studio 库及 Compose 可用。当前不启动服务或安装依赖。
E-SERVICE：未来需要独立 PostgreSQL 实例与规范允许的真实对象 backend（最小本地目录）、临时 database/root、故障注入控制及清理。仓库目前没有 PostgreSQL 测试 service 或镜像 pin；实施时按允许的 tests/storage 路径提供固定版本/真实启动命令与证据，受影响依赖先过许可策略。不新增生产 deploy 或修改 toolchain/CI 路由以绕过门禁。服务未准备不会被本轮报告为已跑通。
E-WINDOWS：当前 workflow 的 windows-2025 native provider；E-MACOS：当前 macos-15 provider，arch 在实际 runner 回读（历史 arm64 不是本次保证）。本机不能替代这些 native runs。后续使用既有获授权 runner 或一次明确范围的 CI 执行授权；本轮无 push、dispatch、rerun、runner/billing 变更。Required gates 仍 required，环境依赖不构成豁免。

Windows Portable corpus 不继承 B002 的 special_socket/case-folded_parent 两项条件。Linux-only Core/runner cases 可按真实产品平台安排，portable archive 安全断言不能用平台标签整体排除；无法构造某 fixture 时显式记录未执行并取得针对实际候选的独立裁定，不能记 PASS。macOS 不承担 Core 或 Creator native shell。cross-build 仅编译。既有 frontend component gates 不等于全部 Chromium E2E；B003 没有新 UI 流程，产品 E2E 留在既有下游义务。

| ROW | REQUIREMENT_ID | AUTHORITY_SOURCE | COMPONENT | PLATFORM_ARCHITECTURE | NATIVE_OR_CROSS_BUILD | TEST_ID | ENTRYPOINT | EXISTING_OR_PLANNED_NEW | REQUIRED_OR_SUPPLEMENTAL | EXPECTED_ASSERTION | EVIDENCE_CONTRACT | EXECUTION_PROVIDER | ENVIRONMENT_OR_AUTHORIZATION_DEPENDENCY | STATUS |
| --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- | --- |
| I01 | REQ-PACKAGE-005 | B003 acceptance[0,1]; SPEC-PACKAGE-INSTALL/STORAGE | installer portable validation | linux/amd64 | NATIVE | TEST-PACKAGE-005 | go test -count=1 -json ./internal/package/install/... | PLANNED_NEW | REQUIRED | archive/path corpus; all validators precede visibility; exact lock/rights/signature/capability negative cases | E1 + E0 | local isolated Linux or approved Linux CI | E-LINUX | PLANNED_NOT_RUN |
| I02 | REQ-PACKAGE-005 | B003 acceptance[0]; PROJECTCTL-PLATFORM-CI windows portable-package test | installer portable validation | windows/amd64 | NATIVE | TEST-PACKAGE-005 | go test -count=1 -json ./internal/package/install/... | PLANNED_NEW | REQUIRED | same portable path/ZIP/schema/collision corpus; no inherited B002 skips | E1 + E0 | approved windows-2025 native job | E-WINDOWS | PLANNED_NOT_RUN |
| I03 | REQ-PACKAGE-005 | B003 acceptance[1]; SPEC-PACKAGE-INSTALL; SPEC-LUA-RUNTIME-PROFILE | Core production-profile adapter | linux/amd64 | NATIVE | TEST-PACKAGE-005 | go test -count=1 -json ./internal/package/install/... | PLANNED_NEW | REQUIRED | real isolated runner executes applicable production tests; bytecode/dangerous operations/cancel/budget/poisoned VM reject; destroy/reap | E1 + E0 | local isolated Linux or approved Linux CI | E-LINUX; exact runner SHA256/profile/runtime; no Host API | PLANNED_NOT_RUN |
| I04 | REQ-PACKAGE-005 | B003 acceptance[1,2]; SPEC-QUALITY-REAL-SERVICES | PostgreSQL + real object storage | linux/amd64 | NATIVE | TEST-PACKAGE-005 | go test -count=1 -json -tags=integration ./tests/integration/package_install/... | PLANNED_NEW | REQUIRED | initial install and idempotent retry; immutable verified objects exist before one DB visibility commit | E1 + E2 | temporary PostgreSQL + isolated filesystem object store on Linux | E-SERVICE | PLANNED_NOT_RUN |
| I05 | REQ-PACKAGE-005 | B003 acceptance[2]; SPEC-QUALITY-REAL-SERVICES | atomicity and recovery | linux/amd64 | NATIVE | TEST-PACKAGE-005 | go test -count=1 -json -tags=integration ./tests/integration/package_install/... | PLANNED_NEW | REQUIRED | fail/cancel/crash before object persist, before/during DB commit, after commit before acknowledgement; unknown commit resolves from DB; no partial visible install | E1 + E2 | same real-service run as I04, separate case evidence | E-SERVICE | PLANNED_NOT_RUN |
| I06 | REQ-PACKAGE-005 | B003 acceptance[3]; SPEC-DATA-OBJECT/TENANT | workspace store and dedup | linux/amd64 | NATIVE | TEST-PACKAGE-005 | go test -count=1 -json -tags=integration ./tests/integration/package_install/... | PLANNED_NEW | REQUIRED | two workspaces same physical hash retain independent rights/grants/retention; wrong tenant and object-key-only read denied; concurrent retry does not delete winner bytes | E1 + E2 | same real-service run as I04 | E-SERVICE | PLANNED_NOT_RUN |
| I07 | REQ-PACKAGE-005 | SPEC-PACKAGE-EXTENSIONS-M1-B010 9.4-9.6; B003 immutable identity | installed B011 canonical packages | linux/amd64 | NATIVE | TEST-PACKAGE-005 | go test -count=1 -json -tags=integration ./tests/integration/package_install/... | PLANNED_NEW | REQUIRED | v1/v2, required supported/unsupported, optional unknown, third-party parity; install-read-export/reload preserves identity and raw optional bytes; no installer format fork | E1 + E2 | same real-service run as I04 | E-SERVICE | PLANNED_NOT_RUN |
| I08 | REQ-PACKAGE-005 | SPEC-PACKAGE-INSTALL; B003 acceptance[1] | migration preflight | linux/amd64 | NATIVE | TEST-PACKAGE-005 | go test -count=1 -json -tags=integration ./tests/integration/package_install/... | PLANNED_NEW | REQUIRED | empty fresh-install preflight explicit; reject incompatible/unsupported migration requirements; active data and Session locks unchanged | E1 + E2 | same real-service run as I04 | E-SERVICE; no B006 migration implementation | PLANNED_NOT_RUN |
| R01 | REQ-PACKAGE-005 (consumer); REQ-PACKAGE-001/002/004 remain upstream-owned | SPEC-PACKAGE-EXTENSIONS-M1-B010; B003 immutable artifact boundary | Host/archive/Creator regression | linux/amd64 | NATIVE | TEST-PACKAGE-001/002/004 + TEST-PACKAGE-005 | go test -count=1 -json ./internal/package/... ./cmd/creator-cli/... ./apps/creator-studio/... ./tests/integration/package_extensions/... | EXISTING + PLANNED_NEW installer | REQUIRED | existing canonical preservation, deterministic second archive, extension negatives plus installer regression remain green | E1 + E0 | Linux full just test or targeted affected run | E-LINUX | PLANNED_NOT_RUN |
| R02 | REQ-PACKAGE-005 (consumer) | SPEC-PACKAGE-EXTENSIONS-M1-B010 9.5; SPEC-CREATOR-ROUNDTRIP | Creator binary | linux/amd64 | NATIVE | TEST-PACKAGE-005 (regression mapping) | go test -count=1 -json ./tests/integration/package_extensions/... | EXISTING | REQUIRED | real embedded-identity binary import/edit/validate/export/reimport and deterministic repeat; install produced package in I07 | E1 + E3 | Linux full just test or focused existing integration target | E-LINUX; source commit/tree and binary SHA256 | PLANNED_NOT_RUN |
| C01 | REQ-PACKAGE-005 | B003 tests[3]; SPEC-RELEASE-GATE-MATRIX | Core/toolchain aggregate | linux/amd64 | NATIVE | TEST-PACKAGE-005 + native governance gates | just check; just test; go vet ./...; just license-check; just ci | EXISTING | REQUIRED | all original aggregate commands; ci adds Go/frontend/native Studio/Compose build; actual required cases execute, no zero-test PASS | E1 + E0 | current m0-baseline.yml linux job or local exact-equivalent evidence | E-LINUX; locked tools; native Studio libs/Compose | PLANNED_NOT_RUN |
| C02 | REQ-PACKAGE-005 (compatibility) | PROJECTCTL-PLATFORM-CI; SPEC-RELEASE-GATE-MATRIX | portable packages / Creator CLI and Studio | windows/amd64 | NATIVE | TEST-PACKAGE-005 + existing compatibility checks | just ci | EXISTING (includes I02 when implemented) | REQUIRED | portable package/Creator/projectctl build/test/vet and native Studio; no Windows Core support assertion | E1 + E0 | current m0-baseline.yml windows job | E-WINDOWS | PLANNED_NOT_RUN |
| C03 | REQ-GOV-002/003 (compatibility) | PROJECTCTL-PLATFORM-CI; SPEC-RELEASE-GATE-MATRIX | projectctl toolchain | darwin/arm64 (runner identity verify at execution) | NATIVE | TEST-GOV-002/003 + existing compatibility checks | just ci | EXISTING | REQUIRED | projectctl build/test/vet; no macOS Core or native Creator shell claim | E1 + E0 | current m0-baseline.yml macos job | E-MACOS | PLANNED_NOT_RUN |
| C04 | REQ-PACKAGE-005 (affected compatibility) | SPEC-RELEASE-GATE-MATRIX; PROJECTCTL-PLATFORM-CI | Web Player and Creator frontend | linux/amd64 | NATIVE toolchain / frontend tests | existing frontend compatibility gates | just ci -> pnpm -r typecheck/build/test | EXISTING | REQUIRED | existing TypeScript/Vite/component gates; no B003 UI added; component tests are not Chromium E2E | E1 + E0 | linux job | E-LINUX | PLANNED_NOT_RUN |
| C05 | REQ-PACKAGE-005 (affected compatibility) | SPEC-RELEASE-GATE-MATRIX; PROJECTCTL-PLATFORM-CI | Web Player and Creator frontend | windows/amd64 | NATIVE toolchain / frontend tests | existing frontend compatibility gates | just ci -> pnpm -r typecheck/build/test | EXISTING | REQUIRED | existing TypeScript/Vite/component gates; no B003 UI added; component tests are not Chromium E2E | E1 + E0 | windows job | E-WINDOWS | PLANNED_NOT_RUN |
| C06 | REQ-PACKAGE-005 (affected compatibility) | SPEC-RELEASE-GATE-MATRIX; PROJECTCTL-PLATFORM-CI | Web Player and Creator frontend | darwin/arm64 (verify actual) | NATIVE toolchain / frontend tests | existing frontend compatibility gates | just ci -> pnpm -r typecheck/build/test | EXISTING | REQUIRED | existing TypeScript/Vite/component gates; no B003 UI added; component tests are not Chromium E2E | E1 + E0 | macos job | E-MACOS | PLANNED_NOT_RUN |
| S01 | REQ-PACKAGE-005 (supplement) | SPEC-RELEASE-GATE-MATRIX | Web Player browser experience | Windows/Linux/macOS Chromium; host arch recorded | NATIVE_BROWSER | TEST-PACKAGE-005 (optional consumer smoke) | PLANNED_NEW consumer smoke if an affected public flow exists | PLANNED_NEW | SUPPLEMENTAL | no B003 public UI path currently exists; full product E2E belongs downstream; does not replace CI component gates | E1 + E0 | future authorized Chromium environment | DEFERRED_OUT_OF_SCOPE for this PLAN/B003 public UI | PLANNED_NOT_RUN |
| S02 | REQ-GOV-002/003 (supplement) | SPEC-RELEASE-GATE-MATRIX | projectctl cross-build | linux/amd64 -> windows/amd64 | CROSS_BUILD | TEST-GOV-002/003 (supplement) | env GOOS=windows GOARCH=amd64 go build -o <evidence>/projectctl-windows.exe ./cmd/projectctl | EXISTING | SUPPLEMENTAL | compile only; never Windows native execution proof | E1 + E0 | local Linux | locked Go; write output outside tracked tree | PLANNED_NOT_RUN |
| S03 | REQ-GOV-002/003 (supplement) | SPEC-RELEASE-GATE-MATRIX | projectctl cross-build | linux/amd64 -> darwin/arm64 | CROSS_BUILD | TEST-GOV-002/003 (supplement) | env GOOS=darwin GOARCH=arm64 go build -o <evidence>/projectctl-darwin ./cmd/projectctl | EXISTING | SUPPLEMENTAL | compile only; never macOS native execution proof | E1 + E0 | local Linux | locked Go; write output outside tracked tree | PLANNED_NOT_RUN |


### 独立治理、接收目标和恢复规范

本轮仅运行受影响治理测试和静态检查：原生 plan/route/check、schema/contract digest、native legal transition、阅读绑定、平台计划静态测试、差异/签名/依赖图/历史保持。只读独立 Codex ACCEPT 上下文按合同→规范/机器→commit/diff→证据→最后 Handoff 检查；其 PASS/FAIL/BLOCKED 是治理候选结果，不是 TEST-PACKAGE-005 功能验收。Git 签名与 fresh route 的候选可读性也不是 main 已生效授权。

接收精确对象由外置 `reception/RECEPTION_HANDOFF.md`、candidate manifest 和独立报告最终固定；三方绑定为：机器 MILESTONE_PLAN 的合同/摘要/revision + always-read MILESTONE_STATUS/PROJECT_SNAPSHOT 的规划投影 + exact candidate Git/route/独立证据及外置 owner receipt。外置交接降权，不覆盖机器字段。

拟议 target：原仓库 `/home/zyc14588/TRPG_PLATFORM/.git` 的 `refs/heads/main`，预期 before SHA/tree `4bd29bbb04aa951d91fc743fa075d1c794a780e5 / ff2b7a9d404379d4c03f35fc941081831fa5ecd7`；实际 main 工作区 `/home/zyc14588/worktrees/TRPG_PLATFORM_MAIN_B002_RECEPTION_A2`。复用上一轮已成功 owner-local 流程：批准后检查 main/clean/signatures/ancestry/证据新鲜，先 no-replace 写入授权、PRE_ACCEPTANCE_GATE 和 REF_UPDATE_INTENT，再在 main 工作区执行 `git merge --ff-only <独立验证的精确候选SHA>`，记录 stdout/stderr/exit/before/after，fresh PLAN 与 B003 ACCEPT route/check 回读并保存，最后形成接收回执。不存在 M1 accept 写入 CLI；ACCEPT 路由是只读检查。

回执沿用 `schema_version = trpg-owner-local-batch-acceptance/1` 的已存在 owner-local 记录惯例，**不宣称是 projectctl native schema**。最小差异：record_kind 使用原 GOVERNANCE_CLOSEOUT_RECEPTION，阶段名 PLAN_FREEZE，明确 governance-only；business_tested_sha/tree 不填本候选，B003 business acceptance 为 NOT_RUN，B002 evidence 仅 baseline 引用；增加 before/after plan revision、B003 before/proposed state 和 digest。保存路径拟为 `/home/zyc14588/engineering/codex-loop-state/owner-receptions/M1-B003-plan-freeze-4bd29bbb-a1/ACCEPTANCE_RECEIPT.json`，此处仅预留交接地址，不创建伪回执。授权对象必须包含该最小差异和 exact candidate SHA/tree。

接收 mutation allowlist：main ref/reflog、该 main 工作区/index 的上述三个 tracked 文件、接收工作区 `.codex/runtime/` 可重建路由、上述新 owner-receptions 目录的授权/intent/result/readback/receipt/hash index。所有业务源、CI、锁文件、其他 refs、旧 ledger/owner closeouts/历史证据不在范围内。接收只使 `B003=FROZEN, M1/v29` 生效；B002=COMPLETED，B004=PLANNED，B012 未分配；不把 B003 改成 IMPLEMENTING，不创建 Builder task。

No-replace：新记录用 exclusive create；已存在完全相同身份记录则只读核验，不覆盖或另造编号逃避冲突。中断后先读实际 ref、reflog 和 durable intent：若仍是 baseline，重新核验后继续；若已是 exact candidate 但 receipt 缺失，标为 REF_UPDATED_RECEIPT_PENDING，补足命令与回读证据而不重复 merge；若第三个 SHA，停止集中报告，不能 reset/rebase。Git ref 与外置回执不声称跨系统原子。前轮业务/治理两阶段在这里收窄为一个规划冻结阶段，不重放 B002 接收。

最终 stop：独立治理 PASS 后提交一个集中 owner 接收包；本 PLAN 授权不包括执行接收。`MAIN_UPDATED=NO; BUSINESS_CODE_CHANGED=NO; B003_BUILDER_STARTED=NO; B004_STARTED=NO; B002_REOPENED=NO; OLD_C1_RESUMED=NO; B012_ALLOCATED=NO; BUSINESS_TESTS=NOT_RUN_NO_BUSINESS_CANDIDATE; REMOTE_WRITES=NONE`。

## 历史 v28 及更早摘要（原文保留）

以下所有“当前”“本 tree”“下一 gate”是对应历史阶段；本节保留原始失败、阻塞和接收前表述，上方新规划投影及外置已接收回执提供当前来源关系。

## M1-B002 固定业务候选的治理完成登记

本 tree 是 `PLAN / planner` 的 plan v28 治理完成登记，B002 从 `VERIFYING` 进入 `COMPLETED`。前一步 v27 `IMPLEMENTING -> VERIFYING` 为 `04f47cbf2b2098e99970facf1a7e4c41bd2775e1` / tree `5da2fdc0172afc206cb84cf5f56cf35f1f6df244`；两步均须独立验证并经本次 owner 授权的本地 ff-only 接收。两次 revision 各加一，依据 `internal/projectctl/codex.go:1244` 的 `validateMilestonePlanChange` 与 `:1345` 的 `validBatchStateTransition`。未分配新 task/cycle，未修改冻结目标、scope、tests 或门禁；M1 本身仍 ACTIVE。

业务验收固定为 commit `473c94190b4eb9e203ab4ea823eaf72829960ad9` / tree `ea87972af11c8c5956c116b58ff946291a0b3bdf`；合同 `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677`。候选已有实际 Lua runtime/profile/VM/checkpoint/IPC 和 runner evidence 代码，checkpoint 修复已保持合法值形状并验证销毁后 replacement restore。原 `d0055c712aa2d9048e35c014f015582adb272496` FAIL 与固定候选原环境 BLOCKED 均保留。本次治理提交不改变业务文件内容、类型或模式，也不重新实现 checkpoint。

### 接收证据及机器消费

- [独立业务 ACCEPT continuation](/home/zyc14588/engineering/m1-b002-010-repair-20260910/native-accept-execution-473c941-20260910/INDEPENDENT_ACCEPTANCE_CONTINUATION.md)，SHA256 `d53c1055d980fd7b3ac2df88552d549d61051acae3593ee7f60707dba2b18419`；[封存执行结论](/home/zyc14588/engineering/m1-b002-010-repair-20260910/native-accept-execution-473c941-20260910/FINAL_RESULT.json)，SHA256 `78ed66662fddf31214b5db5b3a5eec09dea9f1c22e1fd372dd9f7a5593b6e064`：`INDEPENDENT_ACCEPTANCE_RESULT=PASS`、`REMAINING_REQUIRED_GATES=NONE`。
- [required 平台矩阵](/home/zyc14588/engineering/m1-b002-010-repair-20260910/native-accept-execution-473c941-20260910/REQUIRED_NATIVE_GATE_MATRIX.json)，SHA256 `85dcda9a22e523f974309072bed5db666f39b2df3a2bd4a3bf3405a4ab0c99a2`：Linux amd64 承担完整 Core/Lua；Windows amd64 承担 portable package/Creator/projectctl 与前端；macOS arm64 承担 projectctl 与前端。GitHub Actions run `34435123523` / attempt `1`，driver `c8e11cea76b322bd0624c2451e5baa6278ac9b33`；实际 tested source 固定为上述业务 SHA/tree，driver 不是 tested candidate。
- [Windows 独立裁定](/home/zyc14588/engineering/m1-b002-010-repair-20260910/native-accept-execution-473c941-20260910/review/WINDOWS_REQUIRED_NATIVE_ACCEPTANCE.json)，SHA256 `61e0c50d5f2f569aa769791c9f6586d37ab2c1448db10f9eb4440e1eadc9a5af`：`TestImportProjectRejectsSymlinksSpecialFilesAndPortableCollisions/special_socket` 因 Windows runner 无法 bind Unix socket（invalid argument），`/case-folded_parent` 因文件系统大小写不敏感；两者始终 `SKIPPED_NOT_PASSED`，未计入 631 PASS、0 FAIL；required command/stage 均执行。对应 gate `m0-baseline/windows:ci`、`continuation/windows:test-observability`。本次 owner 仅接受此精确候选的两个已披露条件，不构成未来豁免或已执行断言。macOS 106/106 PASS；各平台 Web Player 1、Creator 前端 8 PASS，来源计数不合并。
- [ACC-M1-B002-010 原 owner 关闭事件](/home/zyc14588/engineering/codex-loop-state/owner-closeouts/M1-B002/ACC-M1-B002-010-473c941/FINDING_CLOSEOUT.json)，SHA256 `2505adafa61d1ed9bb402d17d0d6da068731956adc0f2f8f1b8101aa9fb7a969`；[原 owner decision](/home/zyc14588/engineering/codex-loop-state/owner-closeouts/M1-B002/ACC-M1-B002-010-473c941/OWNER_DECISION.md)，SHA256 `9928b218fa397c033fc290fb41445461c36933ad5e79d0123534a79180909fa3`。原事件绑定精确 candidate/contract，状态 `CLOSED_BY_OWNER_FOR_VERIFIED_CANDIDATE`；不以它单独证明 B002 完成。
- [010 机器消费记录](/home/zyc14588/engineering/codex-loop-state/owner-receptions/M1-B002-473c941-business-acceptance-a1/MACHINE_CLOSEOUT_CONSUMPTION.json)，SHA256 `0921c875c2449f6a45afcf38ba7c82da06006ee4596cff454fcaf6de1f7b539b`：读取完整文件并核验摘要、candidate/tree/contract 及接收判定 predicates 后，本原生 always-read 状态摘要登记其稳定引用，并实际采用该已验证关闭事件作为本候选接收前置。本准备记录自身的 `PREPARED_PENDING_NATIVE_REGISTRATION_AND_READBACK` 是发布时事实；独立治理验收及最终 main 回读须验证本登记关系并在外置回执报告结果。没有重发 owner 010 事件，没有修改旧 Controller ledger，没有新原生枚举或 validator 例外。
- [本次 owner 本地接收授权](/home/zyc14588/engineering/codex-loop-state/owner-receptions/M1-B002-473c941-business-acceptance-a1/OWNER_RECEPTION_AUTHORIZATION.json)，SHA256 `6cdce73fe07be2f290473d790feb61767b43eb19986862be119e003e57fb4934`：只接收固定业务候选和满足条件的独立治理增量。最终治理 SHA/tree、实际 main SHA/tree、每阶段退出结果及消费回读由外置本地回执绑定。本治理候选未被独立验收/接收之前不冒充已生效 main；接收后原业务验收与无业务变化的独立治理验证共同构成证据，不将原 CI 测试身份替换为治理 tree。

### 008/009 历史与当前候选义务

[008/009 原始 native finding payload](/home/zyc14588/engineering/codex-loop-state/native-acceptance/trpg-platform/M1-B002/native-acceptance-d1dcdb91ab2ba70fd7d3093d1dc9d8d20f9b548dc02468b65b1d5cdb65ac1d75.json)，SHA256 `ebe8b6a5a05b402dbff9f5f37197baedaac18edd81d1d0d4dd055d81fbe065e9`：原候选 `3a1e45b464ac66f4d3592e0574a27810205a6073` / tree `bce928aa7d3f8ff9e4c67947659f68a335e1684d`，blocking commit `27bc3b7ecf870a27348516ff950526c4fea5f0ed`；acceptance identity `native-acceptance:sha256:d1dcdb91ab2ba70fd7d3093d1dc9d8d20f9b548dc02468b65b1d5cdb65ac1d75`，finding-set digest `cf4a102beb92e6b63c4b78bc8d39c53cc329289a2e123d1be07764a2d2fbd9cd`。两个 HIGH 历史记录仍 OPEN_HISTORICAL，不删除、不改成“从未失败”、不批量 CLOSED。

[当前候选 008/009 与 B011 独立回归裁定](/home/zyc14588/engineering/m1-b002-010-repair-20260910/native-accept-execution-473c941-20260910/review/REUSED_FINDING_AND_B011_DISPOSITION.json)，SHA256 `d0c848f66443373ebe2252e1de830c6898dbc659d5389d069cccd058e6c4cd8c`：

| Finding | 对固定候选的适用性与实际满足情况 | 历史处理 |
| --- | --- | --- |
| ACC-M1-B002-008 | APPLICABLE；真实 IPC 在部分修改后值转换失败，后续操作被 VM_POISONED 拒绝，destroy/reap 后从权威值重建；PASS / VERIFIED_CORRECT_FOR_THIS_EXACT_CANDIDATE | OPEN_HISTORICAL_HIGH 保留，无全局关闭 |
| ACC-M1-B002-009 | APPLICABLE；正常 evidence 证明 TEST-LUA-001/002 实际执行；继承及持久 GOFLAGS=-run=^$ 均退出 1、suites NOT_RUN；PASS / VERIFIED_CORRECT_FOR_THIS_EXACT_CANDIDATE | OPEN_HISTORICAL_HIGH 保留，无全局关闭 |

当前判定依据 `docs/60-quality/ACCEPTANCE_POLICY.md` 的 `SPEC-ACCEPTANCE-RESULT`、`SPEC-ACCEPTANCE-FINDINGS` 与 `SPEC-ACCEPTANCE-EVIDENCE`：当前义务、精确候选和有效证据须满足，原失败记录不删除；这些规则未要求将历史 008/009 全局 owner CLOSED 才能接收新的已验证候选。v26 已要求未来实际候选独立给出 disposition，本次以上关联履行该要求，未隐藏历史 HIGH。

B011 当前候选回归 PASS：真实 `TestB011PackageInputsAndRejections` 与 Creator binary edit/validate/export/reimport/repeat，重复输出 hash `35de290d80f307b19548595c6e26b160b34cda012e1dd0343aa88c522a0810cc`；generic extension 和 Creator 往返保持。[完整封存索引](/home/zyc14588/engineering/m1-b002-010-repair-20260910/native-accept-execution-473c941-20260910/UPDATED_EXPORT_INDEX.json)，SHA256 `c83d91ce6f936a0a2ce355d0638e1e6861c5a9a63405462bf925ad5d503b7115` 保留原 25 项 frozen exports、前轮 140 项、本轮原生补验 391 项，以及原 FAIL/BLOCKED。业务证据复用，不重跑业务全套或 CI。

### 下游边界

依赖保持 B002←B001、B003←B001/B002、B004←B001/B002/B003；所有其他 dependencies、tombstones、IDs、parallel fields 不变，`next_batch_sequence=12`，B012 未分配。B003/B004 均仍 PLANNED、未启动；B002 完成接收后活跃批次为 0，WIP 上限仍为 1。按完成依赖及最小 sequence，下一 eligible batch 是 B003；它尚无 frozen_contract_sha256，须先通过 PLAN/planner 准备与接收合法冻结状态，才有 IMPLEMENT/builder 施工前置。`projectctl codex route` 只校验原生 target 和当前来源，不能把可生成 IMPLEMENT 阅读图当成冻结或启动证明。

仅在本地接收后的实际 SHA/tree 上生成 fresh route/check 与 B003 接续交接；不复用旧 epoch/envelope 或 c1，不开始包安装或 Host API。远端 CI 分支、历史失败候选和封存证据保留；本轮不执行远端接收。

## 历史 plan v26 与更早摘要（以下原文保留）

以下原文中“当前”“下一 gate”“源码未建立”“未集成”等仅指各自历史阶段；上方精确候选与本次治理登记给出新的来源关联，历史内容不改写。

## M1-B002 在已验收 B011 上的生命周期接续候选

本节为 plan v26 的治理候选投影；精确候选 Commit/tree 由外部验收绑定，避免自引用。未接收前，主线 authority 仍是 `b67298de2861643e61794ace51b71891d6417320`、plan v25、`M1-B002 = BLOCKED`。本候选不证明 runtime 已实现，不代替独立 ACCEPT、owner 接收或 human merge，不授权本会话启动业务角色。

- 实际 native batch/task target：`M1-B002`；本次治理入口是 `PLAN M1/MILESTONE`，role `planner`。没有新分配的治理 task 或 M1-B012。
- plan revision：`25 -> 26`；唯一 batch 状态变化：`M1-B002: BLOCKED -> IMPLEMENTING`。这是原生 `validBatchStateTransition` 允许的接续状态；不修改 frozen contract，不是重新定义目标/需求/scope 的 replan。
- 原冻结合同仍在 `.codex/state/MILESTONE_PLAN.yaml` 的 M1-B002 条目，SHA-256 `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677`。没有独立 contract revision 字段，没有新冻约或摘要替换。本文是 always-read 状态/基线投影，不是平行执行合同。
- 唯一 Lua runtime 主责任仍为 B002，拥有 `REQ-LUA-001/002` 和 `TEST-LUA-001/002`。`M1-B012` 未分配；`next_batch_sequence=12` 不变；所有其他 batch、dependencies、tombstones 和 parallel fields 均不变；WIP=1。
- 已验收 B011 产品/主线 baseline：commit `b67298de2861643e61794ace51b71891d6417320`，tree `4c4d3e613c48dcd829192da7e2eddcc9fd1b1d11`。验收记录 `M1-B011-PLATFORM-REMOTE-INTEGRATION-INDEPENDENT-ACCEPT` / `PASS_M1_B011_PLATFORM_REMOTE_INTEGRATION_ACCEPTED` 固定该 identity；B011 全部已验收能力继承。下方旧 preintegration snapshot 保留原文，只代表当时阶段，不撤销该后续已接收事实。
- 实现性质：`FIRST_IMPLEMENTATION_ON_ACCEPTED_B011_LINEAGE`。当前树只有 M0 lua-runner shell，没有 internal/luaruntime；未来不是对当前已有实现做两项小修。没有选择整合历史候选，也没有声明其 backend 合格。
- 生效实现 baseline：本候选正式接收/集成后的精确 authority SHA，由接收回执和新的 native reading map 固定；当前为 `NOT_EFFECTIVE_PENDING_ACCEPTANCE`。不得把未接收候选 SHA 当成已生效主线，不允许回到历史 B002 checkout 施工。
- 接收后按本 plan 状态映射得到的业务 mode/role 为 `IMPLEMENT / builder`，对象为 B002 完整原冻结合同；本轮只进行该路由的只读解析验证，不启动 Builder。旧 c1 的 `REPAIR / repair` envelope 不适用于 B011。

### B002 冻结义务和 finding lineage

原 objective、non_goals、requirements、allowed/forbidden scope、machine_contracts、reading_map_sections、acceptance、tests、stop_conditions、depends_on 与 parallel fields 逐项保留，原摘要不变。Machine contracts 为 `REQ-LUA-001/002`、`TEST-LUA-001/002`、`R14-A15/A22`、`R15-A09/A11/A13`。Frozen reading sections 为 `SPEC-V1-ROADMAP-M1`、`SPEC-M1-ALLOWED/FORBIDDEN/EXIT`、`SPEC-LUA-RUNTIME-001`、`SPEC-PACKAGE-001`、`SPEC-SECURITY-001`、`SPEC-QUALITY-001`；它们仍由原生 route materialization 强制覆盖。

| 必须继承的原义务 | 实现边界与未来验收 |
| --- | --- |
| pinned、许可兼容的 Lua 5.5 Platform Profile，真实语义一致性；没有合格 candidate 则停止 | profile/backend；TEST-LUA-001。没有冻结指定第三方库或 CGo/纯 Go 方案，历史 `golua/v2@v2.0.5` 不是新选型结论。 |
| 仅 UTF-8 source；拒绝 bytecode、io/os/debug、native loader、C module、动态库、文件/网络/进程/环境/原始凭据；无 production REPL | loader/production profile/runner；TEST-LUA-001 的每项拒绝负例，不能只执行 trivial script。 |
| 同主机独立 Lua process、本地双向有界 IPC、无公网接口/DB credentials/Go pointers、无权威写入 | cmd/lua-runner 与 ipc；进程与消息边界/错误传播测试，正式 Host Callback 属于 B004。 |
| 每 Session 独立长期 VM；globals/modules/coroutines/random/capability handles 不共享；结束销毁不影响其他 Session | vm；TEST-LUA-002 生命周期/销毁/交叉污染与 multi-Session race。 |
| checkpoint 仅 nil、bool、受限数值、UTF-8 string、规范数组/字符串键表；拒绝函数、闭包、coroutine、userdata、句柄、环、metatable 行为和能力 token | checkpoint；TEST-LUA-002 允许值 roundtrip、全部禁止值及不兼容绑定负例。 |
| checkpoint 绑定状态版本、包 hash、exact lock、Lua Profile、runtime version；从 authoritative state+checkpoint 重建；内存压力提前重建须先取得当前一致 checkpoint | checkpoint/vm；TEST-LUA-002 恢复等价、故障和内存压力；不得序列化 opaque VM memory 或只在 globals 保存权威事实。 |
| 执行失败污染 VM，下一命令前重建；包括执行后返回值转换错误 | vm/ipc；继承 ACC-M1-B002-008：部分写 globals 后返回 function 等不允许值，后续执行必须拒绝直到重建。 |
| CPU/指令、wall-clock、memory、递归、输出等 runtime 硬预算与取消/超时隔离 | profile/vm/ipc；TEST-LUA-001/002 的资源耗尽/取消/恢复测试；不能靠只在执行前后检查 context 允许无限脚本继续。 |
| declared ∩ trust ∩ execution-context capability；required 缺失拒绝、optional 有 tested fallback、无动态提权；权威 random/time 来自 Host API；稳定序列化/恢复 | 复用现有 package capability/lock model；runtime 必须拒绝未提供能力，跨 VM/过期/伪造句柄负例；完整事务与事件 replay 由现有下游批次承接。 |
| TEST-LUA-001/002 各有真实重复命令、exit code、machine evidence、exact candidate SHA | runner evidence；继承 ACC-M1-B002-009，拒绝/消除 test-selection overrides，证明必需测试实际执行；零执行绝不是 PASS。 |

Host Callback 次数、patch/DB rows/bytes/events/tasks、MutationWorkspace commit/rollback、AUDIT-0 及更高级审计的完整验收仍由 B004 的 `REQ-LUA-003/004/006` 和 `TEST-LUA-003/004/006` 主拥有；B002 的 runtime 安全、预算、污染与拒绝边界不能因其尚未实现而减免。正式 Session/replay/组合安全仍归 B005/B006/B008。该分工沿用现有合同，不创建 B002→B004 的反向依赖。

历史 FAIL candidate `3a1e45b464ac66f4d3592e0574a27810205a6073`，tree `bce928aa7d3f8ff9e4c67947659f68a335e1684d`；blocking commit `27bc3b7ecf870a27348516ff950526c4fea5f0ed`。原生 finding authority `native-acceptance:sha256:d1dcdb91ab2ba70fd7d3093d1dc9d8d20f9b548dc02468b65b1d5cdb65ac1d75`，finding-set digest `cf4a102beb92e6b63c4b78bc8d39c53cc329289a2e123d1be07764a2d2fbd9cd`。008/009 均继续 `OPEN_HISTORICAL`、owner B002，必须在未来实际 candidate 独立验收中明确 disposition；本状态接续不声称已修复或在 B011 已复现。

旧 controller task `M1-B002/c1` 保留其 immutable baseline `a6ce0e1f1f7cddcd191899946093626a81e00111` 和 `REPAIR / repair` envelope。它属于历史谱系，在当前产品计划原本已注明 inactive/not resumed；其 lease=0、RUNNING role=0 的观测不授权修改 ledger，也不把它重新定义为当前 source。此候选既不重绑旧 task，也不要求主机/controller 恢复。若未来另行使用旧 controller，必须先具备其适用的来源/登记 authority，不得把当前 projectctl reading-route PASS 当成旧 controller task 可运行的证明。

### B011 现存接口、路径和未来测试

所有 `PLANNED_NEW` runtime 子目录已包含在原 B002 frozen allowed_scope；目前仍未建立，不能对不存在入口跑测试再记作 runtime FAIL。

| 路径 | 状态 | 真实符号/未来边界 |
| --- | --- | --- |
| `cmd/lua-runner/main.go` | EXISTING | 当前仅 `baselinecli.Run` M0 shell；未来 runner lifecycle/process 入口。 |
| `cmd/lua-runner/evidence.go` | PLANNED_NEW | 未来候选绑定证据，不假称当前已有 evidence subcommand。 |
| `internal/luaruntime/profile/`、`vm/`、`checkpoint/`、`ipc/`、`testdata/` | PLANNED_NEW | 原 frozen scope 内完整 Profile、VM、恢复、IPC 与 fixtures。未授权在这些子目录外任意新增 runtime 文件。 |
| `go.mod`、`go.sum` | EXISTING | 原合同未来允许必要依赖选型；本治理轮不新增或升级依赖。 |
| `internal/package/manifest/manifest.go` | EXISTING | `Parse`/`Document`/`Package`：TOML→normalized model/error，含 Entrypoint/LuaProfile/HostAPIRange/Capabilities/Dependencies/Extensions；`validateRuntime` 要求 runtime 三字段成组、规范相对 .lua 路径，声明不等于执行。 |
| `internal/package/archive/model.go` | EXISTING | `FromFiles`/`Import`/`ImportBytes`→validated immutable Package；`Manifest()`、`ExactLock()`、`ContentHash()`、`ArtifactIdentity()`、`Entry(path)` 和 `Entry.Bytes()` 提供隔离副本及规范 source bytes。 |
| `internal/package/archive/project.go`、`writer.go`、`snapshot.go` | EXISTING | `ImportProject` 使用受限目录读取和相同 canonical model；`Package.Export()`→deterministic Snapshot/error，`Snapshot.Bytes/Hash` 为最终 ZIP identity。 |
| `internal/package/extension/extension.go`、`internal/package/archive/model.go` | EXISTING | `Descriptor/Support/Load`、`Document.ValidateReplacement`、`Package.ReplaceExtension` 保持 generic namespaced JSON、required unsupported 拒绝、optional unknown raw bytes 无损；extension 不能覆盖 core runtime 字段。 |
| `apps/creator-studio/creator/service.go`、`apps/creator-studio/main.go` | EXISTING | `Service.ImportArchive/Inspect/Edit/Export` 与 `runExtensionInspect/runExtensionEdit` 共享 archive+schema 校验；Edit 输入 conflictToken/namespace/JSON→EditResult/error，Export→ExportResult/error；已有 headless edit/validate/export/reimport，不增加 Creator 特权旁路。 |
| `internal/package/capability/capability.go` | EXISTING | `Resolve(declaration,level,policy,execution)`→Resolution/error，复用既有 capability 交集。 |
| `internal/package/install/`、`internal/hostapi/` | PLANNED_NEW | 仅下游 B003/B004 的未来路径，非本轮或 B002 写范围。 |

拟议 package-to-runtime adapter 放在原允许的 vm/profile/ipc 边界，只消费 `Package.Manifest/ExactLock/ContentHash/Entry`，按 `Entrypoint` 取得校验 source，绑定包 identity/profile/lock，进入独立 runner。以真实 v1/v2 B011 包 fixtures 验证加载/执行、缺入口/错 profile 拒绝、core override 禁止和原 extension 无损。包、Creator 的 EXISTING 文件只读；若必须修改其产品契约或 scope，触发原停止条件。

错误类别须区分装载/输入拒绝、profile/capability 拒绝、脚本失败、取消/预算、IPC/runner 故障、checkpoint 值/绑定失败、VM 污染/已销毁；具体新类型/码值尚未实现，不能把历史符号冒充当前 API。通过实际 typed error/有限响应验证传播、不泄露秘密和不继续执行污染 VM。

原测试命令完整保留：`go test ./internal/luaruntime/profile/... ./cmd/lua-runner/...`（TEST-LUA-001）；`go test ./internal/luaruntime/vm/... ./internal/luaruntime/checkpoint/...`（TEST-LUA-002）；`go test -race ./internal/luaruntime/...`；clean tracked tree 下 `just check`、`just test`、`go vet ./...`、`just license-check`。未来 IPC/包到 runtime/取消/008/009 regressions 在这些原 test targets 内落实。新增业务测试均 `PLANNED_NOT_RUN`。

B011 不回归的真实现存入口：`go test ./internal/package/... ./cmd/creator-cli/... ./apps/creator-studio/...`、Creator `TestHeadlessInspectEditValidateExportReimport`、以及当前三平台 `just ci` 分流。本轮均 `NOT_RUN_NO_BUSINESS_CANDIDATE`，不重用 B011 PASS 充当 Lua 验收。

平台责任保持 `SPEC-RELEASE-GATE-MATRIX`：Core/platformd/workerd/lua-runner 仅 Linux amd64 原生；Compose 为 Linux amd64，Windows 走 WSL2/Docker Desktop；Player 为 Windows/Linux/macOS Chromium；Creator Studio/CLI 为 Windows/Linux。`platformCIPlanForGOOS` 的 Linux 执行全部 Go build/test/vet+native Studio/Compose，Windows 执行 portable package/Creator 的 native build/test/vet，macOS 执行 projectctl 和 frontend gates。三平台 workflow 都调用 Just，不等于三个 OS 都原生执行 Lua；cross-build 不能当 native execution。原全 Go tests/vet 义务在正式 Linux Core 平台执行，不通过禁用必需能力令构建变绿。

### 下游与停止边界

依赖不变：B002←B001；B003←B001/B002；B004←B001/B002/B003；B005←B003/B004；B006←B002/B005；B007←B003/B006；B008←B003/B004/B005/B006/B007；B009←B007/B008；B010/B011←B001。无环，无第二个 Lua 主责任。B003 安装需要已验收 Lua profile/runner；B004 正式 Host API 需要 runtime lifecycle/IPC/errors/isolation 及 B003 安装/存储前置。此顺序来自现行 graph，不强制增加反向交叉依赖。

原四项 stop conditions 不变：不得削弱 Lua 5.5/source-only/process/checkpoint/production-debug 标准；不得接受未定/不兼容许可或未 pin 关键依赖；不得用 opaque VM memory/functions/coroutines/userdata 或缺失权威事实恢复；不得扩大到 Host API/Session/AI/Room/official game/Creator。原非目标及全部禁止目录保持原文。未来实施交付必须满足完整原冻约和 finding 回归，而非最小壳。

本轮 `BUSINESS_CODE_CHANGED=NO`、`LUA_BUILDER_STARTED=NO`、`LUA_IMPLEMENTATION_TESTS=NOT_RUN`、`BUSINESS_FULL_SUITE=NOT_RUN_NO_BUSINESS_CANDIDATE`、`REMOTE_WRITES=NONE`。候选接收前 `IMPLEMENTATION_AUTHORIZED=NO`；唯一下一 gate 为此治理候选的独立 ACCEPT 与原生要求的 owner/human 接收，不释放实现启动提示词。

## 历史 B011 preintegration 摘要（原文保留）

以下文字保留 b67298d 所承载的历史阶段及 evidence identity；其中“当前”“下一 gate”“未集成”均为该历史快照语境，不覆盖上文已经读取的实际 B011 remote-integration acceptance 或本候选状态。


产品：AI 原生在线桌面游戏平台。

V1：官方隐藏信息桌游 + 规则异常 TRPG + Creator Studio + 托管/自托管。

技术：单节点 Go、Lua 5.5 游戏包、React Web、Wails Studio、PostgreSQL。

M0 已独立验收 `PASS`、合并并关闭。当前边界为 `M1`，计划版本为 `25`，`next_batch_sequence = 12`。

`M1-B001` 已完成；`M1-B002` 为 `BLOCKED / GOVERNANCE_STATE_REPROJECTION_ONLY`；`M1-B003`—`M1-B009` 为 `PLANNED`；`M1-B010` 为 `COMPLETED / HISTORICAL_COMPLETION_PROJECTION`；`M1-B011` 为 `COMPLETED / LINEAGE_CLEAN_CONTENT_RECONSTRUCTION`。当前 preintegration governance rebind candidate 的 active verification target 为 `M1-B011-PLATFORM-PREINTEGRATION-GOVERNANCE-REBIND-INDEPENDENT-ACCEPT`。

`M1-B002` 的 blocked historical authority 是 `27bc3b7ecf870a27348516ff950526c4fea5f0ed`，冻结契约仍为 `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677`。B002 product code is NOT integrated into this lineage；其 `BLOCKED` 状态是 independently established historical evidence 的治理投影；product code imported: `false`。

`M1-B010` 的冻结契约 `452846abd429474fb57aaab1a4247308df5b78ea819601e113bd2a33d2368583` 保持精确历史等价。历史 lifecycle authority 为 `cde53640e143b7adb9e36d7d8bc4fedb6f0ed401`；accepted code/tree 为 `1339c490bd8cae1e2a6e6607fc0f1db019faab64` / `25d5645caf883a73f165db0179accbca48ddf526`；platform ACCEPT 为 `PASS`；`R2-INT-010` 为 `RESOLVED_ADAPTER`。这些都是历史完成事实投影：B010 product code integrated into clean lineage 为 `false`，当前 tree 不具有 B010 capability，旧 main integration 已由 `M1-B011` clean reconstruction 取代。

旧 plan authority `ea211cfaa0f9e6008da68688b076db281c8a45df` 与旧 B011 contract `b2a2ca0d410b2dec4977f4dd09529e4213eceb4d613d0a87a7b1b5044b3616fd` 保持 immutable historical evidence，分别分类为 `REJECTED_NON_INTEGRABLE_PLAN_AUTHORITY` 与 `REJECTED_HISTORICAL_PLAN_CONTRACT`，modified: `false`。新的 clean B011 contract 为 `fa7d260c4f77f164b6ae7f1582eb87b20a1f2b4cf59b37a799027e17bfe876a3 = ACTIVE_FROZEN_CONTRACT`，仅依赖 `M1-B001`，没有额外 pre-implementation governance prerequisite。Lineage-clean product identity 继续固定为 `b82d52571287d9d0ebcfa589fd575125ac578476`，tree 为 `b9be959f22a8b15080a1035fba0fb46c256edf62`；product accepted 为 `true`，本 transition 的 product changes 为 `NONE`。

Platform independent acceptance 为 `PASS`。历史 verdict `PASS_M1_B011_PLATFORM_ACCEPTED` 的 evidence root `112dfdd0d93ba936fba31108006c166ae21ea86a2e090b9dbac611fcca93e9a9` 仅为 `HISTORICAL_ONLY`：original payload unavailable、not reverified，不能作为 current authority。当前 durable authority 是 `M1-B011-PLATFORM-ACCEPTANCE-EVIDENCE-REISSUE-001` / `PASS_M1_B011_PLATFORM_ACCEPTANCE_EVIDENCE_REISSUED`，verified root `865a244e234dadb0f919fd42645b44ee74129c1e01b81d4736615bbe0659ea40`，archive SHA-256 `21ecfa011cee94197670e12f13c94201276585362e66fbed1c96a7c485f5d5dd`，Platform ACCEPT binding `aee21ae041faad20ba96379d7d2c9ecfae0cef372d40e3265cd08f03e7a1f081`。

Fresh/current Creator binary identity 为 `5b54b2d5c9db302baf965da0e5061b427652e2a7c58a2f647bf5d8a98e36bb55`；historical identity 为 `6158fe53646fc3d53f87f48ef675d0dc727806e24a0ee195d0992dd28681af87`；binary reproducibility 为 `DIFFERENT`，但 deterministic product output 继续为未变化的 `35de290d80f307b19548595c6e26b160b34cda012e1dd0343aa88c522a0810cc`。

历史 cross-repository closure 为 `PASS / SUPERSEDED_FOR_CURRENT_INTEGRATION`。历史 authority 是 `M1-B011-CROSS-REPOSITORY-EXACT-PAIR-ACCEPT-001` / `PASS_M1_B011_CROSS_REPOSITORY_EXACT_PAIR_ACCEPTED`，root `b154784d312babb7dc83b76f03f88c7ed879c5895d0dee8ea15b58f5573ea88d`，archive SHA-256 `3365c6adc84ebb490beb1a5fe9a56ecf1487436bc7f8ac6d2acad82f44dc0743`，Rules cross-ACCEPT binding `09ced89ea54e068d62aee2e2da288d1c5a8d22fb49d7330dd36f97080e522edf`。其 exact tuple 绑定 Rules adapter `455cd5d66c683565a4aad7ef8d6523421d37db71` / `d74aae0baf791566a770ef1aaed2da7562f681d5`、Rules rebind `00e015270b99d819ffefcb0e36442f0b4e7874ef` / `c63a63b051f6cecc6eec24b786d2b2a0f2b216d6`、Platform product `b82d52571287d9d0ebcfa589fd575125ac578476` / `b9be959f22a8b15080a1035fba0fb46c256edf62` 与 Platform VERIFYING predecessor `412111c05c087ed066a297304bed728870f03fc8` / `a77015feee62564ec8ec9744391a3b5de83e4217`。该历史 PASS 不接受新的 Rules / Platform governance tuple。

当前 preintegration Rules binding 为 `6caf57cdc1127e84546459766949e0da664bb2f9` / tree `ec6cb5e7fdc0c93f09862fcf032dbb61393293a1`，candidate acceptance task 为 `M1-B011-RULES-REBIND-INDEPENDENT-ACCEPT`，root `27d8cf103dd0ce6740330df74c813c9872133a484243d06c264ac5903f1348d5`，archive SHA-256 `b4c55e9886651945a693239799f64f1474cf13a15b8a745c7acb6baddb4b45df`。Preintegration prerequisite `M1-B011-RULES-REMOTE-INTEGRATION-INDEPENDENT-ACCEPT` 的外部证据 root 为 `5ca14d526359972dec8b345e3c537e5d9d5b751695277302cc4d7bd935e0d9d3`，archive SHA-256 为 `a0ed5564e60ea072ed0210c87da0a1fc0545ab158292d157ea2e2e06b83568da`；远端写入结果只记录于该外部审计包。

Recovery PLAN-003（root `2fc1d3ba75e457a329f46a4ebf9292801424598b6480dd81048fa460bdf8f995`；独立验收 root `008abc6ae98d9d0820c4bd5eb94f21fbcf3f4056994d85f294f762d7e297f47f`）要求先验收本次治理绑定，再执行 `M1-B011-NEW-CROSS-PAIR-ACCEPT` 和 `M1-B011-COMPLETION-REACCEPT`。新的 cross-pair 和 completion reaccept 均为 `REQUIRED_NOT_RUN`；不复用历史配对 PASS。

M1-B011 lifecycle state: `COMPLETED`。

Identity 分层保持明确：

- Product identity: `b82d52571287d9d0ebcfa589fd575125ac578476` / `b9be959f22a8b15080a1035fba0fb46c256edf62`。
- VERIFYING governance identity: `412111c05c087ed066a297304bed728870f03fc8` / `a77015feee62564ec8ec9744391a3b5de83e4217`。
- Completion transition identity: commit `e99f31ad842ebfda4c51808f50c46be3f2a97cfc` / tree `a15ded87265b2ae30da20baa9810cca056e30798`；parent `412111c05c087ed066a297304bed728870f03fc8`。
- Accepted post-transition identity projection repair: `967aa4f628a521265e82eeb746bad629f23d5358` / `cac9af0744e0acf8a08cc937990a242ceb12b96c`。
- Current preintegration governance rebind candidate: product and lifecycle identities remain unchanged；this new candidate's exact commit/tree are bound externally by its signed Git object and the next independent acceptance, avoiding a self-reference。

历史 completion transition independent acceptance `M1-B011-COMPLETION-INDEPENDENT-ACCEPT-001` 的结果为 `FAIL_M1_B011_COMPLETION_TRANSITION`，failure 为 `FAIL_COMPLETION_TARGET_BINDING`，当时 disposition 为 `REPAIR_REQUIRED`。随后 `M1-B011-COMPLETION-INDEPENDENT-ACCEPT-RETRY-001` 在 governance head `967aa4f628a521265e82eeb746bad629f23d5358` 通过，结果为 `PASS_M1_B011_COMPLETION_TRANSITION_ACCEPTED_AFTER_PROJECTION_REPAIR`，root 为 `9091af0acf5a48177ca7fea8708aa87a7c07a77ac0f861ee46d25f30ad865b46`。这是旧配对的已接受历史事实；新的 Rules binding 仍要求 completion reaccept。

读取图缺失的规范章节以 exact accepted semantic authority `cde53640e143b7adb9e36d7d8bc4fedb6f0ed401` 补齐：`PACKAGE_MODEL.md` 为 mode/blob `100644 3322861b08979c328d068b1c6040494238cd8186`，`M1_SCOPE_AND_EXIT_GATE.md` 为 `100644 dee317bded7541582f4b168a52bd85213a462e3b`。Creator boundary governance repair authority `54f5dea3438d32a04a14f8888347aa3dfc222ee9` 保持 accepted，manifest-v2 Creator boundary 保持 `GENERIC JSON`。Clean lineage base 为 `main@36e0009e779564ad44799f54f9ccc1c74ac412a8`；B002 ancestry 为 `ABSENT`；old rejected PLAN ancestry 为 `ABSENT`；old B010 product ancestry 为 `ABSENT`。

Rules 状态仍为 `M2-B001 = ACCEPTED`、effective `R2-INT-010 = RESOLVED_ADAPTER`、`M2-B002 = NOT_STARTED`、`M2-B002_READY = NO`；不宣称 Rules aggregate M2 PASS、formal M2-B002 PASS 或 formal M2-B009 PASS。

原 completion transition `e99f31ad842ebfda4c51808f50c46be3f2a97cfc` 继续保持 `M1-B011 = COMPLETED`；本 preintegration governance rebind 只更新当前绑定和验收依赖，不产生第二次 lifecycle transition。治理修复范围由 `GOV-M1-B011-PREINTEGRATION-BINDING` 限定。新的治理候选尚待独立验收；Platform `integration_authorized = false`、`push_authorized = false`、`merge_authorized = false`。下一 gate 是 `M1-B011-PLATFORM-PREINTEGRATION-GOVERNANCE-REBIND-INDEPENDENT-ACCEPT`，之后依次为新 cross-pair、completion reaccept、transport PLAN 及其独立验收、staging flow。

状态摘要不覆盖权威规范。
