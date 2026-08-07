---
document_id: SPEC-RELEASE-GATE-001
authority: normative-spec
status: ACTIVE
language: zh-Hans
baseline: R0-R24
---

# 兼容矩阵、不可变产物与发布门禁

<a id="SPEC-RELEASE-GATE-MATRIX"></a>
## 1. V1 兼容矩阵

- 服务端、workerd、lua-runner：Linux amd64；
- Compose：Linux amd64；Windows WSL2/Docker Desktop 使用路径；
- Web Player：Windows、Linux、macOS Chromium；
- Creator Studio：Windows、Linux；
- Creator CLI：Windows、Linux；
- 手机/平板、Firefox、Safari、Linux arm64：不承诺。

<a id="SPEC-RELEASE-GATE-ARTIFACT"></a>
## 2. 构建一次晋级

同一源码 Commit 构建一次、签名一次，以同一哈希产物晋级 Alpha/Beta/RC/Stable。任何代码、依赖、迁移或官方内容变化都生成新候选，不能同版本静默重建。

<a id="SPEC-RELEASE-GATE-CI"></a>
## 3. CI

仓库内 `projectctl` 是工程逻辑来源，Justfile 是统一薄入口，GitHub Actions 只调用相同 Just 目标。所有进入 main 的 Commit 可验证签名。

<a id="SPEC-RELEASE-GATE-RC"></a>
## 4. RC 强制门禁

完整兼容矩阵、数据库迁移、包升级、备份恢复演练、最小游戏、官方桌游 3/4 人、TRPG 单次/短战役、Lua Profile、真实模型代表组合、全量安全扫描、SBOM/许可、来源签名、故障注入和人工批准全部通过，发布阻塞项为零。

<a id="SPEC-RELEASE-GATE-BLOCK"></a>
## 5. 不可绕过

恢复演练失败、官方游戏 E2E 失败、权限/秘密泄露、许可证未清、迁移不一致、SBOM/签名缺失或任何 P0/P1 发布阻塞均禁止 Stable。紧急发布也不能绕过安全、数据和许可硬门禁。
