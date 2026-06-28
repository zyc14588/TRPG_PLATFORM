# P2 Execution Rules for Codex

## Session model

每个批次至少需要两个 Codex session：

1. implementation session：可以修改代码、测试、文档。
2. acceptance session：只读验收，不修改任何 tracked file。

如果验收 FAIL，不要让验收 session 直接修复。回到 implementation session 或新建 repair session。

## Branch model

建议分支：

```powershell
git checkout -b codex/p2-b00-docs-install
git checkout -b codex/p2-b01-domain
git checkout -b codex/p2-b02-storage-rls-db
git checkout -b codex/p2-b03-ingest-worker
git checkout -b codex/p2-b04-rig-agent-engine
git checkout -b codex/p2-b05-server-api
git checkout -b codex/p2-b06-frontend-ui
git checkout -b codex/p2-b07-hardening
git checkout -b codex/p2-db-setup
```

## 初始检查

每个 implementation session 开始时运行或记录：

```powershell
git status --short
git branch --show-current
git log --oneline -8
cargo metadata --no-deps
```

如果存在 frontend：

```powershell
pnpm --version
```

## Windows / PowerShell 规则

- 优先使用 PowerShell 兼容命令。
- 除非 shell 明确是 Git Bash/WSL，不使用 `set -euo pipefail`、`export FOO=bar`、`cmd || true` 等 Bash-only 写法。
- `rg` 不存在时，使用 `git grep -n` 或 `Select-String`。
- 不假设本机有 PostgreSQL、Docker、browser E2E dependency。

## 环境缺失处理

如果 DB、Docker、SQLx offline data、browser dependency 不可用：

- 继续完成静态审查、编译检查、单元测试。
- 最终报告列出未运行命令、失败原因、需要的环境变量。
- 不把环境缺失的检查写成 PASS。
- 对安全/RLS 相关缺失，最多给 CONDITIONAL 或 BLOCKED，不能给无条件 PASS。

## 代码修改规则

- 优先最小 patch。
- 不大范围格式化不相关文件。
- 不自动升级依赖。
- 不把 TODO/FIXME 当成交付证明。
- 不在 docs/examples/tests/snapshots/logs 写 secret。
- 不通过隐藏 failing tests 或降低断言让 CI 通过。

## 生成物卫生

必须避免 tracked generated artifacts：

```powershell
git ls-files | rg '(^target/|node_modules/|\.next/|dist/|\.tsbuildinfo$|tsconfig\.tsbuildinfo$)'
```

如果有匹配，说明生成物仍被 Git 跟踪，需要从 index 移除并更新 `.gitignore`。
