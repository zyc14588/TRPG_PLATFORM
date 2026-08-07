---
document_id: SPEC-RESTART-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 原仓库冻结、分支规范化与零迁移

<a id="SPEC-RESTART-FREEZE"></a>
## 1. 冻结

执行时动态获取远端默认分支，记录 Commit、Tree、许可发现、分支/Tag、文件与依赖清单，并创建远端可验证的签名注释 Tag。签名或远端验证失败时不得清理活动树。

<a id="SPEC-RESTART-BRANCH"></a>
## 2. 分支

当前公开观察默认分支为 `master`，新治理要求受保护 `main`。在冻结 Tag 后使用 GitHub 支持的分支重命名或等价无历史重写方式规范化，随后创建 `restart/v1-baseline`。

<a id="SPEC-RESTART-ZERO-MIGRATION"></a>
## 3. 零迁移

旧代码、测试、迁移、配置、CI、设计、Codex 提示词和素材不得复制、移动、Cherry-pick 或改名迁入。旧项目只通过 Tag、Git 历史和冻结证据阅读。通用标准模板必须记录新来源。

<a id="SPEC-RESTART-HISTORY"></a>
## 4. 历史

不删除旧 Commit、Tag、Issue、PR 和 Release。旧开放 Issue 归类并关闭，不自动转成 V1 要求。M0 不创建并列 `legacy/` 活动树。

<a id="SPEC-RESTART-PR"></a>
## 5. PR

M0 使用短生命周期 `restart/v1-baseline`，保留多个逻辑签名 Commit，独立验收 PASS 后以非 Squash merge 合并 main。
