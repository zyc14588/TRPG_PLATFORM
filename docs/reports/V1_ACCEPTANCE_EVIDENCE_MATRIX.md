# V1 验收证据矩阵生成契约

V1 的 17 项定义以仓库根目录
`V1_ACCEPTANCE_EVIDENCE_MATRIX.md` 为唯一来源。候选矩阵不得在仓库中手工填入
`PASS`；release job 必须在仓库外从 evidence manifest 生成
`V1_ACCEPTANCE_EVIDENCE_MATRIX_FILLED.md`。

生成与验证入口：

```bash
python3 scripts/ci/acceptance_evidence_matrix.py generate \
  --manifest "$EVIDENCE_DIR/acceptance-evidence-manifest.json" \
  --output "$EVIDENCE_DIR/V1_ACCEPTANCE_EVIDENCE_MATRIX_FILLED.md"
python3 scripts/ci/acceptance_evidence_matrix.py validate \
  --manifest "$EVIDENCE_DIR/acceptance-evidence-manifest.json" \
  --matrix "$EVIDENCE_DIR/V1_ACCEPTANCE_EVIDENCE_MATRIX_FILLED.md" \
  --require-ready
```

validator 必须同时证明：

- candidate commit 和 tree 分别精确等于当前 `HEAD` 与 `HEAD^{tree}`；
- 根级定义 hash 匹配，行号恰为 1–17，且没有重复或缺失；
- manifest、生成矩阵和所有 source evidence 均位于仓库外；
- 每个 evidence 文件可读、不是 symlink，且 SHA-256 匹配；
- 每个 `PASS`/`FAIL` 行都引用完整的 repository evidence report，且该 report 的
  command、exit code、原始日志、JUnit/SARIF 与 artifact hash 通过现有强校验；
- 行 command 精确等于根级定义中的权威命令，evidence 必须实际执行该命令（复合
  命令只允许通过 `bash -c`/`bash -lc` 原样执行）；
- repair batch 的 PASS evidence 必须绑定同一 HEAD/tree、实际执行完整
  `bash scripts/ci/test-all.sh`、包含非空负向测试与空 `not_run`；每个负向测试名
  必须出现在绑定 JUnit 的成功 testcase 中，并且该 batch 的每个依赖行必须分别
  具有自身权威命令的机器证据；RF01–RF04 的最小负向 testcase 数分别为
  5、4、5、4；
- 生成矩阵与 manifest 的 canonical 渲染逐字节一致；
- RF01–RF04 未由外部 closure evidence 关闭时，关联行不能为 `PASS`；
- `--require-ready` 下工作树必须 clean，所有 repair batch 和 17 行都必须为 `PASS`。

`.github/workflows/release.yml` 会上传 manifest 与生成矩阵。任一旧 SHA、缺行、
hash 漂移、手工改状态或仓库内 evidence 都会阻断 release readiness。
