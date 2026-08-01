# V1 验收证据矩阵（仅 CI 生成）

本 tracked 文件不承载候选发布状态，也不得手工填入 `PASS`。

候选矩阵由 `scripts/ci/acceptance_evidence_matrix.py` 在仓库外生成，并与同目录的
`acceptance-evidence-manifest.json` 一同验证和上传。当前 RF01–RF04 尚未提供
closure evidence；生成器会把关联项（包括第 15 项）标记为 `BLOCKED`，其他未提供
逐项证据的条目标记为 `NOT_RUN`。

生成契约见 `docs/reports/V1_ACCEPTANCE_EVIDENCE_MATRIX.md`。任何把本文件当作 release
evidence 的流程都必须 fail closed。
