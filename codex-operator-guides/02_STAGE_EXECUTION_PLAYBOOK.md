# 阶段施工 — v2.21

## 用途

按 S00 到 S13 顺序执行，每次只处理一个阶段，防止跨阶段 scope creep。

## 必读输入

1. `AGENTS.md`
2. `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
3. `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
4. `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
5. `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
6. `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
7. `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
8. `V1_ACCEPTANCE_EVIDENCE_MATRIX.md`

## 可复制给 Codex 的中文提示词

```text
请只执行 SXX 阶段。读取该阶段六件套、关联 execution batch、per-file prompts 和 normalized maps。只修改当前阶段允许的文件，运行阶段测试，写入 `evidence/stages/SXX/IMPLEMENTATION_SUMMARY.md`，然后停止等待验收。
```

## 执行步骤

1. 读取 `stages/SXX/**` 六件套。
2. 读取关联 batch 和 per-file prompts。
3. 套用 current-safe module/output 映射。
4. 按最小 scope 编码或补测试。
5. 运行阶段测试并写 evidence。

## 命令 / 检查

```powershell
$stage = "SXX"
New-Item -ItemType Directory -Force "evidence/stages/$stage"
git diff --name-only
cargo test --workspace --all-features
pnpm test
```

## 预期证据

`evidence/stages/SXX/IMPLEMENTATION_SUMMARY.md`

## 失败处理

若发现需要跨阶段修改，记录为 blocker 或 change-control，不得顺手扩大范围。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
