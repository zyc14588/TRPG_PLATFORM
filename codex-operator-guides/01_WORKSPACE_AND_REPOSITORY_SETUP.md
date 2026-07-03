# 工作区与仓库准备 — v2.21

## 用途

准备 git branch、仓库结构、依赖工具、运行环境和 evidence 目录，避免 Codex 在脏工作区施工。

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
请准备 v2.21 施工工作区。检查 `git status --short`，确认或创建 `codex/v1-coc-runtime-s00` 风格分支，识别 Rust、Node、pnpm、Docker、数据库迁移和前后端目录。只允许准备工作区和 evidence 目录，不得实现功能。请输出 `evidence/operator/WORKSPACE_SETUP.md`。
```

## 执行步骤

1. 定位 git repository root。
2. 确认当前 branch 与 dirty state。
3. 记录 Rust、Node、pnpm、Docker 版本。
4. 检查 lockfile、migrations、compose、workflow 目录。
5. 创建 evidence 基础目录。

## 命令 / 检查

```powershell
git rev-parse --show-toplevel
git status --short
git switch -c codex/v1-coc-runtime-s00
rustc --version
node --version
pnpm --version
docker --version
```

## 预期证据

`evidence/operator/WORKSPACE_SETUP.md`

## 失败处理

若 git dirty 且无用户授权，不得继续；若依赖安装失败，不得擅自改 lockfile。

## 退出标准

所有要求的 evidence 已生成，且下一步操作明确；任何未运行检查不得写成 PASS。
