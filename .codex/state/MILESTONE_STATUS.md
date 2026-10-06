---
document_id: CODEX-MILESTONE-STATUS
schema_version: 1
document_kind: state-summary
authority: state-summary
status: ACTIVE
source_commit: "bce7d4052cb0d8530a851061685bd2692d9c1a83"
---

# 里程碑状态

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

## 当前 Linux M1：B003 验收通过，B004 公共能力登记阻塞

- M1 ACTIVE v34；正常 PLAN 仅 B004 IMPLEMENTING→BLOCKED，WIP=0，冻结字段/摘要及所有其他批次/序号保留。有效产品批次 4/10（40%），M1 出口未满足。
- B003 独立 Linux 业务 PASS 精确来源 cf364dc；fbe5532 完成状态独立 PASS_FOR_STATE_PROJECTION_ONLY，回执 SHA256 `e220f1ec4b2d702838c8046eb30179711edbba0cf214b48f0901011bcc5da7ea`。业务/依赖字节不变，旧 FAIL 与跨平台 NOT_RUN 保留；自建服务已清理。
- B004 业务 NOT_RUN；CHANGE-M1-HOST-CAPABILITY-REGISTRY 等待 Owner 决定，未分配新批次、未修改公共登记/Schema/冻结合同。提案与停止原因见 PROJECT_SNAPSHOT 本节。本状态候选待独立状态复核和本机 main 接收。

## 历史 B003 完成及 B004 激活状态（原文保留）

## 当前 Linux M1：B003 完成登记，B004 激活候选

- M1 ACTIVE v33；正常 PLAN 仅 B003 VERIFYING→COMPLETED、B004 FROZEN→IMPLEMENTING，冻结摘要/其他 batches/序号不变，WIP=1。有效产品批次 4/10（40%），尚未满足 M1 出口。
- B003 精确独立 PASS：`cf364dcec1d2b61afa12b4a2c79e58e8a338936d` / `bcf7957356588fcf325c6252e627fb7469e99c63`，回执 SHA256 `1386f3f2c2d50003c6f75f1450a43ffe612113a752f5077caee6624ba666be18`。完整新 Linux 732/37 named run/pass，0 fail/skip；Windows/macOS 延期 NOT_RUN，原 FAIL 与 003 保留。
- 本状态候选待独立状态验收和 main 本地接收，不冒充已经接收。B004 业务 NOT_RUN；前置与冻结规划核验通过后 fresh IMPLEMENT route/check 才开工。详情及固定证据在 PROJECT_SNAPSHOT 本节。

## 历史 B004 冻结与 B003 验收状态（原文保留）

## 当前 B004 规划冻结候选（尚未开工）

- M1 ACTIVE v32；B004 PLANNED→FROZEN，首个摘要 `4b7d73a8d566d3097f3afbb45f8271e00d7aead7192992bba59974a717c08e31`；新增最小 profile 引擎路径与 Linux scope 阅读绑定，原业务义务/公共语义保持。
- B003 VERIFYING、独立结论待回读；B004 前置尚未全部释放，冻结不是开工。WIP=1、active 仅 B003；所有其他 batch/冻约/序号保持，B012 未分配。
- 当前 Linux 必须证据、实际接口、实施步骤、风险与原停止门禁见 PROJECT_SNAPSHOT 本次 B004 节。新产品/公共合同决策 NONE；B003业务PASS与完成接收、B004冻结独立核验之后才激活 IMPLEMENT。

## 历史 B003 验收及 Linux 范围状态（原文保留）

## 当前 B003 Linux 独立验收中（PLAN 生命周期候选）

- 本 tree：M1 ACTIVE v31，B003 IMPLEMENTING→VERIFYING；scope 与 Linux 业务独立验收 PENDING，不声明 PASS、COMPLETED 或 main 接收。
- 固定业务 SHA/tree：df1fa6793317cf26660ee92803e63eaf5e7a254a / 5d517c1b949a3e785629bb8563304bded0ed97ac；scope 治理9fc76428e3f227c3337b32f4edd3e6077d47eb54独立结论待回读。原 frozen digest 59e0456c8ed261f08b1d1211fbdd1436cc50f4e875f36482732fe84e39d5167e 保持。
- 其他 batches、墓碑与 next_batch_sequence=12 不变；B004 PLANNED，WIP=1、仅 B003 VERIFYING。当前 M1 Linux 范围、原平台延期与历史 FAIL 保留；完成 PASS 后才进入正常 PLAN 完成登记。

## 历史 Linux 范围登记状态（原文保留）

## 当前 M1 Linux 范围登记（治理候选）

- Owner 当前授权：完成 Linux amd64 的 M1；Windows/macOS 本次 DEFERRED_BY_OWNER_M1_LINUX_ONLY / NOT_RUN，不能标 PASS。规范补充入口 `docs/00-governance/M1_LINUX_ACCEPTANCE_SCOPE.md`（SPEC-M1-LINUX-ACCEPTANCE-SCOPE）；V1 多平台支持及 RC/Stable 门禁保留。
- 机器 plan 未变：M1 ACTIVE v30；B003 IMPLEMENTING，原 frozen digest `59e0456c8ed261f08b1d1211fbdd1436cc50f4e875f36482732fe84e39d5167e`；B001/B002/B010/B011 COMPLETED、B004—B009 PLANNED、next_batch_sequence=12、WIP=1。本维护不登记 lifecycle 或完成。
- 精确业务测试：`df1fa6793317cf26660ee92803e63eaf5e7a254a` / `5d517c1b949a3e785629bb8563304bded0ed97ac`，Linux 638 affected + 37 real-service named run/pass，0 fail/skip；canonical check/test/vet/license/ci exit 0。原证据 index SHA256 `f5a35e4745f87463315b9eba15bd6bdc9f6625d345a74e784a596fe2b5861323`，路径与 Creator/E3、原始日志封存见 PROJECT_SNAPSHOT 当前节。新增治理 tree 未被原业务 CI 直接测试。
- 原独立 FAIL 保留：后继 receipt SHA256 `8595f592e84eda776a57bf6281ca260612c065b5d20e810dcd203a275d9b6a50`；002/004 仅对上述精确业务后继 CLOSED_FOR_VERIFIED_SUCCESSOR；003 原 OPEN_REQUIRED_NATIVE_EVIDENCE 保留，本次 Linux 平台欠项按 owner 延期，非全局 CLOSED。原 19 行平台矩阵与历史 bytes 保留。
- 当前独立 Linux 业务验收 PENDING；B003/M1 完成 NOT_CLAIMED，main authority 仍 `a353b2da35d20eb162d1eb404ac33fc0a3eacfaf`。下一 gate 是 scope 治理独立 ACCEPT → Linux B003 独立 ACCEPT → 正常 PLAN lifecycle → B004；下方阶段性授权和无候选陈述均为历史。

## 历史 B003 实现启动状态（原文保留）

## M1-B003 实现启动治理候选

- 已接收冻结基线：`7c6240e28c1ff4066debedffb43284b979dee260` / `c3a0f11cf2c99d1bfb6ec6d57c95d5016d7c3e8c`，M1/v29、B003 FROZEN；规划回执 SHA256 `f303229da35630a25629fd935b7964ddba115fcefcec6c77902320ba3f925212`。
- 本 tree：原生 PLAN 的 M1/v30、B003 IMPLEMENTING 候选；冻结合同摘要 `59e0456c8ed261f08b1d1211fbdd1436cc50f4e875f36482732fe84e39d5167e`、14 类字段及 PROJECT_SNAPSHOT 的 19 行测试映射不变。B001/B002/B010/B011 COMPLETED，B004 PLANNED，B012 UNALLOCATED，next_batch_sequence=12；仅 B003 active，WIP=1。
- 精确激活 SHA/tree、独立治理结果与本地接收生效身份由签名 Git object 和 `/home/zyc14588/engineering/codex-loop-state/owner-receptions/M1-B003-implementation-start-7c6240e-a1/` 的实际证据绑定。接收前仍以已接收 v29/FROZEN 为 main authority；本候选不自报接收 PASS。
- 接收及读回成功后才移交独立工作区中的全新 IMPLEMENT/builder；原业务范围与禁止目录不变。当前业务候选不存在、业务验收 NOT_RUN_NO_BUSINESS_CANDIDATE；无业务合入、B004、B002 重开、B012、旧 c1 或远端写入授权。

## 历史 v29 规划冻结状态（原文保留）

以下“当前”“本 tree”“未接收”属于原规划冻结阶段；原首轮 FAIL 与 ACC-M1-B003-001 后继处置保留，当前启动投影见上文。

## M1-B003 PLAN 冻结候选（未接收）

- `EXISTING_AUTHORITY`：当前 main `4bd29bbb04aa951d91fc743fa075d1c794a780e5` / `ff2b7a9d404379d4c03f35fc941081831fa5ecd7`，M1/v28，B002 COMPLETED，B003 PLANNED/unfrozen。本地回执 SHA256 `c727815a4bf3b2af2abbf7108fce3819321a2fec6b98ca665f7230dc31eda572` 已核验；010 已消费，008/009 历史和原候选 disposition 均保留。
- 本 tree：`PLANNING_CANDIDATE_AUTHORITY = M1/v29, M1-B003 FROZEN`；拟冻结 digest `59e0456c8ed261f08b1d1211fbdd1436cc50f4e875f36482732fe84e39d5167e`。原 objective/scope/dependencies/acceptance/tests/stop_conditions 不变，只补齐既有规范阅读引用，再按原生规则递增 revision 并冻结。
- 机器合同唯一入口 `.codex/state/MILESTONE_PLAN.yaml`；接口/测试/平台/环境/接收规范在 `.codex/state/PROJECT_SNAPSHOT.md` 本轮 B003 节，由 fresh route always-read 绑定。外置 Git identity/独立报告/owner receipt 避免提交自引用。
- 独立治理结果尚由外置精确证据给出，不从本摘要推导 PASS；未正式接收前 main 仍 v28。接收 target 为原 common directory 的 refs/heads/main，before 固定上述 baseline，方式为 owner 批准后的 exact-candidate ff-only；不是 M1 accept CLI。
- 接收后仅 B003 FROZEN 生效；未来 FROZEN→IMPLEMENTING 和 Builder source 须重新原生 PLAN 绑定并另获启动授权，本轮不释放。B004 仍依赖未完成 B003。B001/B002/B010/B011 COMPLETED，其余状态/墓碑/序号原样；next_batch_sequence=12，WIP=1，active=0，B012 UNALLOCATED。
- `OWNER_DECISION_REQUIRED=NONE`（范围决策）；最终 owner 精确候选接收批准仍 REQUIRED_NOT_PERFORMED。业务测试 PLANNED_NOT_RUN；无业务修改、旧 c1、CI 或远端写入。

## 历史 v28 及更早状态（原文保留）

以下“当前/本 tree/下一”是原历史阶段表述，不覆盖上方当前已接收 baseline 与本轮未接收候选的区分。

## 当前固定候选治理登记：M1-B002 / COMPLETED

- 本 tree：`M1 / ACTIVE / plan_version 28 / next_batch_sequence 12`；本次合法状态转换为 `M1-B002 VERIFYING -> COMPLETED`，前一步 v27 为 `04f47cbf2b2098e99970facf1a7e4c41bd2775e1` / tree `5da2fdc0172afc206cb84cf5f56cf35f1f6df244`。本完成登记须经独立治理验证及本次 owner 授权的本地接收后生效。
- `BUSINESS_TESTED_SHA=473c94190b4eb9e203ab4ea823eaf72829960ad9`；业务 tree `ea87972af11c8c5956c116b58ff946291a0b3bdf`；原冻结合同 `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677` 不变。接收前 main 固定为 `bb90989a128e6cdf9c64bde1129496d9baed8478` / tree `9f3220edb96c9f373ac170ec75edd08d120cd5a9`。
- 业务事实：固定候选已实现并通过独立业务验收的 Lua 5.5 source-only profile、隔离 Session VM/IPC、checkpoint 值形状保持与重建；本轮仅登记治理状态及证据引用，没有重新编写业务实现。下方 v26 的“源码未建立”和“尚无业务候选”只属于原历史阶段。
- 独立业务验收：`PASS`，`REMAINING_REQUIRED_GATES=NONE`；[封存独立 ACCEPT](/home/zyc14588/engineering/m1-b002-010-repair-20260910/native-accept-execution-473c941-20260910/INDEPENDENT_ACCEPTANCE_CONTINUATION.md)，SHA256 `d53c1055d980fd7b3ac2df88552d549d61051acae3593ee7f60707dba2b18419`；[required 平台矩阵](/home/zyc14588/engineering/m1-b002-010-repair-20260910/native-accept-execution-473c941-20260910/REQUIRED_NATIVE_GATE_MATRIX.json)，SHA256 `85dcda9a22e523f974309072bed5db666f39b2df3a2bd4a3bf3405a4ab0c99a2`。原 CI 测试的是固定业务候选；其后的治理 tree 另须独立增量验证，不能声称原 CI 直接测试了治理提交。
- 010 owner 状态：`CLOSED_BY_OWNER_FOR_VERIFIED_CANDIDATE`；实际读取并核验 [原 owner 关闭事件](/home/zyc14588/engineering/codex-loop-state/owner-closeouts/M1-B002/ACC-M1-B002-010-473c941/FINDING_CLOSEOUT.json)，SHA256 `2505adafa61d1ed9bb402d17d0d6da068731956adc0f2f8f1b8101aa9fb7a969`；原 owner decision SHA256 `9928b218fa397c033fc290fb41445461c36933ad5e79d0123534a79180909fa3`。原事件不重发，原失败报告不改写。
- 010 机器消费关联：[机器消费准备记录](/home/zyc14588/engineering/codex-loop-state/owner-receptions/M1-B002-473c941-business-acceptance-a1/MACHINE_CLOSEOUT_CONSUMPTION.json)，SHA256 `0921c875c2449f6a45afcf38ba7c82da06006ee4596cff454fcaf6de1f7b539b`。本 always-read 摘要引用并采用该记录已验证的原关闭事件，作为当前精确候选接收前置；该记录发布时尚待 native registration/readback。只有独立治理检查及接收后回读证明本关联，才能报告最终 `MACHINE_CLOSEOUT_CONSUMPTION=PASS`；这不是新增 projectctl 状态枚举或 M1 close CLI 输出。
- 008/009 历史均保留 `OPEN_HISTORICAL_HIGH`；对本业务 SHA/tree 的继承义务均为 `APPLICABLE / PASS / VERIFIED_CORRECT_FOR_THIS_EXACT_CANDIDATE`，不作全局 CLOSED。完整历史 identity、当前适用性、Windows 两项 skip 与 B011 回归见本次 PROJECT_SNAPSHOT 当前节。
- Owner 本地接收授权：[APPROVE_WITH_CONDITIONS 授权对象](/home/zyc14588/engineering/codex-loop-state/owner-receptions/M1-B002-473c941-business-acceptance-a1/OWNER_RECEPTION_AUTHORIZATION.json)，SHA256 `6cdce73fe07be2f290473d790feb61767b43eb19986862be119e003e57fb4934`。治理候选精确 SHA/tree 由签名 Git object、独立验收与外置接收回执绑定，避免自引用；候选登记本身不证明 main 已更新。仅在上述条件满足且 main ff-only 接收后，本登记才成为生效本地状态；远端接收另有边界。
- 其他 batch、dependencies、tombstones、parallel fields 不变：B001/B010/B011 `COMPLETED`，B003—B009 `PLANNED`，B012 `UNALLOCATED`；WIP 上限 1，本 tree 活跃批次为 0；B002 已登记 COMPLETED。B003/B004 未启动，旧 c1 不恢复。
- 生效接收 gate：两步治理增量独立验证，再按 owner 授权接收固定业务候选及治理后续，外置回执证明实际完成。完成登记生效后最早依赖满足批次是 B003；B003 仍 PLANNED 且未冻结，下一原生模式/角色为 PLAN/planner 冻结准备。B004 仍依赖尚未完成的 B003；不因本登记获得施工授权。
- 本轮业务全套复跑 `NO`；新 CI `NO`；远端写入 `NONE`。外置回执记录实际阶段退出结果；不声称 Git ref 与外置回执跨系统原子。

## 历史 plan v26 与更早状态（以下原文保留）

以下所有“当前”“下一”“未实现”“未接收”均为各自历史阶段的记录，不覆盖上方固定候选事实；原 FAIL/BLOCKED、008/009 与旧治理 gate 不被删除或改写。

## 当前治理候选状态：M1-B002 continuation

- Current milestone/plan candidate: `M1 / ACTIVE / plan_version 26 / next_batch_sequence 12`。
- Candidate batch transition: `M1-B002 BLOCKED -> IMPLEMENTING`，沿原生 PLAN 状态转换；原 frozen contract `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677` 完全不变。
- Accepted authority before this candidate: `b67298de2861643e61794ace51b71891d6417320` / tree `4c4d3e613c48dcd829192da7e2eddcc9fd1b1d11`，plan v25，B002 仍 BLOCKED；候选尚未生效，不证明施工开始。
- B011 product baseline/acceptance: 上述精确 main SHA/tree；`PASS_M1_B011_PLATFORM_REMOTE_INTEGRATION_ACCEPTED`；原 B011 COMPLETED、generic extension、Creator 往返及平台职责全部保持。下方旧 preintegration gate 是历史阶段，已由外部实际 main 验收回执接续。
- Implementation kind: `FIRST_IMPLEMENTATION_ON_ACCEPTED_B011_LINEAGE`；Lua 源码未建立，历史候选未整合。
- Candidate native next business mode/role after acceptance: `IMPLEMENT / builder`，batch/task target `M1-B002`；this work: `PLAN / planner`。仅解析路由，不启动业务角色。
- B002 findings: `ACC-M1-B002-008`、`ACC-M1-B002-009` 均 `OPEN_HISTORICAL`，owner B002；未来 actual candidate 独立验收必须验证并给出处置，不因 state transition 关闭。
- B002 original source history: `27bc3b7ecf870a27348516ff950526c4fea5f0ed` 保留；旧 controller c1 的 immutable REPAIR envelope 不被重绑。当前接续不从旧 controller 恢复业务、不重开主机治理。
- B001/B010/B011: `COMPLETED`；B003—B009: `PLANNED`；B012: `UNALLOCATED`；dependencies/tombstones/IDs 原样；WIP=1。
- Contract inheritance, actual B011 interfaces, EXISTING/PLANNED_NEW paths, future tests and platform matrix: `.codex/state/PROJECT_SNAPSHOT.md` 的本次 continuation 部分（原生 always-read 绑定），以及原 frozen plan 条目。
- Governance candidate SHA/tree: 由本候选 Git object 和独立验收外部固定，不自引用。Effective implementation baseline: `NOT_EFFECTIVE_PENDING_ACCEPTANCE`，由后续被接收 authority 及 fresh route 精确绑定。
- Governance candidate acceptance: `REQUIRED`；owner/human reception: `REQUIRED_NOT_PERFORMED`；`IMPLEMENTATION_AUTHORIZED=NO`。
- Next exact gate: 独立 ACCEPT 本 state-only B002 continuation governance candidate，之后等待原生要求的 owner/human 接收；本轮不执行业务或远端集成。
- Business implementation tests: `NOT_RUN`；business full suite: `NOT_RUN_NO_BUSINESS_CANDIDATE`；Builder started: `NO`；remote writes: `NONE`。

此处 IMPLEMENTING 仅是尚待接收候选的目标 lifecycle state，不能冒充主线已恢复施工、runtime 已实现或旧 findings 已关闭。

## 历史 B011 preintegration 状态（原文保留）

以下原文记录旧阶段身份及 gate；“当前/下一”仅适用于历史快照，不撤销上面的 B011 已验收主线事实。


- Completed milestone: `M0`
- M0 result: `PASS / MERGED / CLOSED`
- Current milestone: `M1`
- M1 status: `ACTIVE / plan_version 25 / next_batch_sequence 12`
- Completed M1 batches: `M1-B001`, `M1-B010`, and `M1-B011`; current preintegration governance rebind awaits independent acceptance
- `M1-B002`: `BLOCKED / GOVERNANCE_STATE_REPROJECTION_ONLY` (inactive, not resumed, frozen contract `5dbcccd9ddda20bbcbdcbff473fb9197b83bb087a10f1ee802ed8ad547d74677`)
- `M1-B003`—`M1-B009`: `PLANNED`
- `M1-B010`: `COMPLETED / HISTORICAL_COMPLETION_PROJECTION`
- `M1-B010` frozen contract: `452846abd429474fb57aaab1a4247308df5b78ea819601e113bd2a33d2368583`
- `M1-B010` historical lifecycle authority: `cde53640e143b7adb9e36d7d8bc4fedb6f0ed401`
- `M1-B010` accepted code/tree: `1339c490bd8cae1e2a6e6607fc0f1db019faab64` / `25d5645caf883a73f165db0179accbca48ddf526`
- Historical platform ACCEPT: `PASS`
- Historical `R2-INT-010`: `RESOLVED_ADAPTER`
- B010 product code integrated into clean lineage: `false`
- B010 main integration: `superseded by M1-B011 clean reconstruction`
- `M1-B011`: `COMPLETED / LINEAGE_CLEAN_CONTENT_RECONSTRUCTION`, depends only on `M1-B001`
- Rejected historical B011 plan authority: `ea211cfaa0f9e6008da68688b076db281c8a45df` (`REJECTED_NON_INTEGRABLE_PLAN_AUTHORITY`, immutable, modified: `false`)
- Old B011 contract: `b2a2ca0d410b2dec4977f4dd09529e4213eceb4d613d0a87a7b1b5044b3616fd = REJECTED_HISTORICAL_PLAN_CONTRACT`
- New clean B011 contract: `fa7d260c4f77f164b6ae7f1582eb87b20a1f2b4cf59b37a799027e17bfe876a3 = ACTIVE_FROZEN_CONTRACT`
- B011 pre-implementation governance prerequisite: `NONE`
- B011 implementation authorization: `CLOSED / PRODUCT_CANDIDATE_COMMITTED`
- B011 lineage-clean product candidate: `b82d52571287d9d0ebcfa589fd575125ac578476` / tree `b9be959f22a8b15080a1035fba0fb46c256edf62`
- B011 product accepted: `true`; product changes in this transition: `NONE`
- B011 platform independent acceptance: `PASS`
- B011 fresh platform acceptance reissuance: `PASS`
- B011 historical cross-repository exact-pair acceptance: `PASS / SUPERSEDED_FOR_CURRENT_INTEGRATION`
- B011 historical completion transition independent acceptance retry: `PASS`; current binding completion reaccept: `REQUIRED_NOT_RUN`
- Active verification target: `M1-B011-PLATFORM-PREINTEGRATION-GOVERNANCE-REBIND-INDEPENDENT-ACCEPT`
- B002 or old B010 product code imported into this lineage: `false`
- Normative closure provenance: `cde53640e143b7adb9e36d7d8bc4fedb6f0ed401`; `docs/30-package-spec/PACKAGE_MODEL.md` mode/blob `100644 3322861b08979c328d068b1c6040494238cd8186`; `docs/80-roadmap/M1_SCOPE_AND_EXIT_GATE.md` mode/blob `100644 dee317bded7541582f4b168a52bd85213a462e3b`
- Next gate: `M1-B011-PLATFORM-PREINTEGRATION-GOVERNANCE-REBIND-INDEPENDENT-ACCEPT`

B002 product code is NOT integrated into this lineage.

No B002 or old B010 product ancestry is present in this clean lineage.

## M1-B011 identity layers

- Product identity: `b82d52571287d9d0ebcfa589fd575125ac578476` / `b9be959f22a8b15080a1035fba0fb46c256edf62`.
- VERIFYING governance predecessor: `412111c05c087ed066a297304bed728870f03fc8` / `a77015feee62564ec8ec9744391a3b5de83e4217`.
- Completion transition commit: `e99f31ad842ebfda4c51808f50c46be3f2a97cfc`.
- Completion transition tree: `a15ded87265b2ae30da20baa9810cca056e30798`.
- Completion transition parent: `412111c05c087ed066a297304bed728870f03fc8`.
- Accepted post-transition identity projection repair: `967aa4f628a521265e82eeb746bad629f23d5358` / `cac9af0744e0acf8a08cc937990a242ceb12b96c`.
- Current preintegration governance rebind candidate: exact commit/tree bound externally by its signed Git object and the next independent acceptance; product and lifecycle identities remain unchanged.

## Completion transition acceptance history

- Prior task/result: `M1-B011-COMPLETION-INDEPENDENT-ACCEPT-001` = `FAIL_M1_B011_COMPLETION_TRANSITION`.
- Failure: `FAIL_COMPLETION_TARGET_BINDING`.
- Historical failure disposition: `REPAIR_REQUIRED`, followed by the accepted projection repair.
- Completion acceptance retry task: `M1-B011-COMPLETION-INDEPENDENT-ACCEPT-RETRY-001`.
- Retry result: `PASS_M1_B011_COMPLETION_TRANSITION_ACCEPTED_AFTER_PROJECTION_REPAIR` at governance head `967aa4f628a521265e82eeb746bad629f23d5358`.
- Retry root: `9091af0acf5a48177ca7fea8708aa87a7c07a77ac0f861ee46d25f30ad865b46`.
- Current Rules binding completion reaccept: `REQUIRED_NOT_RUN`; the historical PASS does not accept the new tuple.

## Platform acceptance authority

- Historical verdict: `PASS_M1_B011_PLATFORM_ACCEPTED`.
- Historical evidence root: `112dfdd0d93ba936fba31108006c166ae21ea86a2e090b9dbac611fcca93e9a9` (`HISTORICAL_ONLY`; original payload unavailable; not reverified; not current authority).
- Current durable task/result: `M1-B011-PLATFORM-ACCEPTANCE-EVIDENCE-REISSUE-001` / `PASS_M1_B011_PLATFORM_ACCEPTANCE_EVIDENCE_REISSUED`.
- Current durable Platform evidence root: `865a244e234dadb0f919fd42645b44ee74129c1e01b81d4736615bbe0659ea40` (`verified`).
- Current durable Platform archive SHA-256: `21ecfa011cee94197670e12f13c94201276585362e66fbed1c96a7c485f5d5dd`.
- Platform ACCEPT route binding: `aee21ae041faad20ba96379d7d2c9ecfae0cef372d40e3265cd08f03e7a1f081`.
- Fresh/current Creator binary SHA-256: `5b54b2d5c9db302baf965da0e5061b427652e2a7c58a2f647bf5d8a98e36bb55`.
- Historical Creator binary SHA-256: `6158fe53646fc3d53f87f48ef675d0dc727806e24a0ee195d0992dd28681af87`.
- Creator binary reproducibility across historical/current builds: `DIFFERENT`; deterministic product output remains unchanged at `35de290d80f307b19548595c6e26b160b34cda012e1dd0343aa88c522a0810cc`.

## Historical cross-repository acceptance authority

- Task/result: `M1-B011-CROSS-REPOSITORY-EXACT-PAIR-ACCEPT-001` / `PASS_M1_B011_CROSS_REPOSITORY_EXACT_PAIR_ACCEPTED`.
- Evidence root: `b154784d312babb7dc83b76f03f88c7ed879c5895d0dee8ea15b58f5573ea88d`.
- Archive SHA-256: `3365c6adc84ebb490beb1a5fe9a56ecf1487436bc7f8ac6d2acad82f44dc0743`.
- Rules adapter: `455cd5d66c683565a4aad7ef8d6523421d37db71` / `d74aae0baf791566a770ef1aaed2da7562f681d5`.
- Rules rebind: `00e015270b99d819ffefcb0e36442f0b4e7874ef` / `c63a63b051f6cecc6eec24b786d2b2a0f2b216d6`.
- Rules cross-ACCEPT binding: `09ced89ea54e068d62aee2e2da288d1c5a8d22fb49d7330dd36f97080e522edf`.
- Classification: `SUPERSEDED_FOR_CURRENT_INTEGRATION`; preserve the historical exact tuple and evidence.

## Current preintegration binding

- Rules candidate: `6caf57cdc1127e84546459766949e0da664bb2f9` / tree `ec6cb5e7fdc0c93f09862fcf032dbb61393293a1`.
- Rules candidate acceptance: `M1-B011-RULES-REBIND-INDEPENDENT-ACCEPT`, root `27d8cf103dd0ce6740330df74c813c9872133a484243d06c264ac5903f1348d5`, archive SHA-256 `b4c55e9886651945a693239799f64f1474cf13a15b8a745c7acb6baddb4b45df`.
- External prerequisite: `M1-B011-RULES-REMOTE-INTEGRATION-INDEPENDENT-ACCEPT`, root `5ca14d526359972dec8b345e3c537e5d9d5b751695277302cc4d7bd935e0d9d3`, archive SHA-256 `a0ed5564e60ea072ed0210c87da0a1fc0545ab158292d157ea2e2e06b83568da`; remote outcomes remain in that external audit.
- Recovery PLAN-003 root: `2fc1d3ba75e457a329f46a4ebf9292801424598b6480dd81048fa460bdf8f995`; independent acceptance root: `008abc6ae98d9d0820c4bd5eb94f21fbcf3f4056994d85f294f762d7e297f47f`.
- Current maintenance contract: `GOV-M1-B011-PREINTEGRATION-BINDING`.
- Governance rebind independent acceptance: `REQUIRED_NOT_RUN`.
- `M1-B011-NEW-CROSS-PAIR-ACCEPT`: `REQUIRED_NOT_RUN`.
- `M1-B011-COMPLETION-REACCEPT`: `REQUIRED_NOT_RUN`.
- Next sequence: governance rebind ACCEPT → new cross-pair ACCEPT → completion reaccept → new transport PLAN → transport PLAN independent ACCEPT → staging flow.

## Completion boundary

- Rules remains `M2-B001 = ACCEPTED`, effective `R2-INT-010 = RESOLVED_ADAPTER`, `M2-B002 = NOT_STARTED`, and `M2-B002_READY = NO`.
- No Rules aggregate M2 PASS, formal M2-B002 PASS, or formal M2-B009 PASS is claimed. Platform transport, staging, merge and main alignment remain pending.
- `current_binding_completion_reacceptance = REQUIRED_NOT_RUN`; Platform `integration_authorized = false`; `push_authorized = false`; `merge_authorized = false`.

`M1-B002` 的 `BLOCKED` 状态是对 independently established historical evidence 的治理投影，保持 inactive / not resumed。`M1-B010` 是 historical semantic predecessor；其完成状态只投影已接受历史事实，旧 integration 已由 `M1-B011` clean reconstruction 取代。当前 product identity 继续绑定已接受的 B011 lineage-clean reconstructed capability。原 completion transition `e99f31ad842ebfda4c51808f50c46be3f2a97cfc` 维持 `M1-B011 = COMPLETED`；本 preintegration governance rebind 更新当前 Rules 身份和后续验收依赖，不改变 lifecycle state，不充当候选独立验收或集成回执。
