---
document_id: SPEC-DOCUMENT-AUTHORITY-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 文档权威、冲突处理与阅读顺序

<a id="SPEC-DOCUMENT-AUTHORITY-PURPOSE"></a>
## 1. 目的

本规范定义新主线中哪些文件构成权威事实来源、不同来源发生冲突时如何停止施工，以及人类维护者、Codex、CI 和审计人员的最小阅读顺序。

<a id="SPEC-DOCUMENT-AUTHORITY-HIERARCHY"></a>
## 2. 权威层级

从高到低依次为：

1. **适用法律、已生效许可和安全硬边界**；
2. **机器契约**：OpenAPI、JSON Schema、事件模式、包清单模式、Lua Host API 描述；
3. **当前 ACTIVE 的规范性设计文档**；
4. **冻结的批次契约**，只能收窄上级要求，不能扩张或覆盖；
5. **代码与通过的自动测试所证明的实际行为**；
6. **状态摘要、路线计划和 Handoff**，仅用于导航；
7. **历史 Git、旧 Tag、旧 Issue 和聊天记录**，仅用于审计和来源说明。

任意下级来源与上级来源不一致时，必须停止相关施工并登记阻塞，不得选择更方便的一方继续。

<a id="SPEC-DOCUMENT-AUTHORITY-SOURCES"></a>
## 3. 机器与自然语言事实来源

| 事项 | 权威来源 |
|---|---|
| 产品范围、V1 非目标 | `V1_SCOPE.md` |
| 产品体验与官方游戏 | `docs/10-product/` |
| Go/Lua 单节点架构 | `docs/20-architecture/` |
| 包、Host API、迁移 | `docs/30-package-spec/` 与 `schemas/` |
| 安全和撤销 | `docs/40-security/` |
| 部署、备份、升级 | `docs/50-operations/` |
| 测试与发布门禁 | `docs/60-quality/` |
| 决策状态 | `DECISION_REGISTER.yaml` |
| 里程碑出口 | `docs/80-roadmap/` |
| 需求与测试追踪 | `docs/90-traceability/*.yaml` |
| 当前 Codex 施工范围 | 当前冻结 `BATCH_CONTRACT.md` |

<a id="SPEC-DOCUMENT-AUTHORITY-STATUS"></a>
## 4. 文档状态

活动文档只能使用：`DRAFT`、`ACTIVE`、`SUPERSEDED`、`DEFERRED`、`REJECTED`。

- `ACTIVE`：可用于施工和验收。
- `SUPERSEDED`：保留来源关系，但不得用于新施工。
- `DEFERRED`：设计有效但不在当前交付版本。
- `DRAFT`：不得作为发布、许可签署或公共契约。
- `REJECTED`：明确不采用。

<a id="SPEC-DOCUMENT-AUTHORITY-LANGUAGE"></a>
## 5. 语言与标识

简体中文是当前权威自然语言。代码标识、Schema ID、API、事件、包 ID 和稳定 Section ID 使用英文。未来英文翻译不得修改中文规范语义；若翻译冲突，以当前中文 ACTIVE 规范为准，直至正式双语治理另行批准。

<a id="SPEC-DOCUMENT-AUTHORITY-GENERATED"></a>
## 6. 生成文件

标记为生成的文件只能通过 `projectctl generate` 更新。CI 必须在临时目录重新生成并比较。禁止手工修改生成索引、类型绑定或追踪摘要。

<a id="SPEC-DOCUMENT-AUTHORITY-READING"></a>
## 7. 阅读顺序

- 普通开发者：根 README → 当前里程碑 → 当前批次契约 → 精确引用规范。
- Codex 施工：`.codex/SESSION_START.md` → 模式路由 → `READING_MAP.yaml`。
- 独立验收：批次契约 → 需求/机器契约 → Diff → 测试证据 → Handoff。
- 安全审计：事件与证据清单优先，普通日志和 Handoff 仅作辅助。

<a id="SPEC-DOCUMENT-AUTHORITY-CHAT"></a>
## 8. 聊天与旧设计

R0 以前及已废弃重构聊天不属于活动事实来源。R0—R24 的有效结论已写入机器决策登记和主题规范。任何需要完整聊天才能理解的施工文件均为不合格文档。
