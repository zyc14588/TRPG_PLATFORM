# Strict Link and Reference Validation

本文档定义文档收敛后的路径与引用校验。所有命令都从仓库根执行；任何未运行检查不得记录为 PASS。

## 1. 根目录与规范文件

```bash
python3 - <<'PY'
from pathlib import Path

root = Path.cwd()
allowed = {"README.md", "AGENTS.md", "MANIFEST.md"}
actual = {path.name for path in root.glob("*.md")}
assert actual == allowed, ("unexpected root Markdown", sorted(actual - allowed))

required = {
    "docs/README.md",
    "docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md",
    "docs/construction/CODEX_STANDALONE_BOOTSTRAP_PROMPT.md",
    "docs/construction/SOURCE_BUNDLE_INTEGRATION_GUIDE.md",
    "docs/governance/DOCUMENT_ORGANIZATION_AND_AUDIT_BOUNDARY.md",
    "docs/acceptance/V1_ACCEPTANCE_EVIDENCE_MATRIX.md",
}
missing = sorted(path for path in required if not (root / path).is_file())
assert not missing, ("missing canonical documents", missing)
print("PASS document layout")
PY
```

## 2. README 与文档索引链接

```bash
python3 - <<'PY'
from pathlib import Path
import re

root = Path.cwd()
for relative in ("README.md", "docs/README.md"):
    source = root / relative
    for target in re.findall(r"\[[^]]+\]\(([^)]+)\)", source.read_text(encoding="utf-8")):
        if target.startswith(("http://", "https://", "mailto:", "#")):
            continue
        resolved = (source.parent / target.split("#", 1)[0]).resolve()
        assert resolved.is_relative_to(root.resolve()), (relative, target)
        assert resolved.exists(), ("broken link", relative, target)
print("PASS README links")
PY
```

## 3. 旧根路径回流检查

以下命令应无输出。规范分类目录前缀既允许仓库根相对形式（例如
`docs/acceptance/...`），也允许 `docs/` 内文档使用的相对形式（例如
`acceptance/...` 或 `../acceptance/...`）。`source-archive/**` 和历史 evidence
保持原始 provenance，不属于本检查范围。

```bash
rg -n -P '(?<!construction/)CODEX_STANDALONE_BOOTSTRAP_PROMPT\.md|(?<!construction/)SOURCE_BUNDLE_INTEGRATION_GUIDE\.md|(?<!acceptance/)(?<!reports/)V1_ACCEPTANCE_EVIDENCE_MATRIX\.md|(?<!planning/)PER_STAGE_FIXTURE_EXPANSION_PLAN\.md' \
  -g '!source-archive/**' -g '!evidence/**' -g '!inventory/**' \
  -g '!docs/governance/STRICT_LINK_AND_REFERENCE_VALIDATION.md' \
  -g '!MANIFEST.md' -g '!manifests/*PACKAGE_MANIFEST.md'
```

同时确认旧操作指南目录已经移除：

```bash
test ! -e codex-operator-guides
test -f docs/construction/operator-guides/README.md
```

## 4. 代码与治理引用

迁移后的权威文件同时被 Python 生成器、PowerShell 治理门禁和 Rust 编译期合约读取：

```bash
python3 scripts/ci/test_acceptance_evidence_matrix.py
cargo test -p trpg-testing --test golden_ci_test_matrix_contract_tests --locked
cargo test -p trpg-testing --test implementation_acceptance_checklist_source_contract_contract_tests --locked
cargo test -p trpg-testing --test requirement_to_test_trace_contract_tests --locked
cargo test -p trpg-testing --test research_decision_matrix_contract_tests --locked
pwsh -NoProfile -File scripts/verify-governance-boundary.ps1
```

若环境只有 Windows PowerShell，可将 `pwsh` 替换为 `powershell.exe`。

## 5. Manifest 与仓库门禁

`MANIFEST.md`、`manifests/CURRENT_PACKAGE_MANIFEST.md` 和 `manifests/SELF_CONTAINED_PACKAGE_MANIFEST.md` 必须由生成器维护。写入前把完整预期变更放入真实或临时 Git index；不要手工修改 hash。

```bash
python3 scripts/ci/manifest.py --check
python3 scripts/ci/verify_manifest.py
git diff --check
```

提交后的完整合并门禁：

```bash
bash scripts/ci/test-all.sh contracts
```

该入口要求干净工作树，并可能需要 PowerShell、网络和固定版本工具。工作树尚未提交时，`repo_truth` 因 dirty 状态失败是预期行为，不能据此伪报完整门禁 PASS。

## 6. 通过标准

- 根目录没有额外说明性 Markdown。
- `README.md` 和 `docs/README.md` 的本地链接都可解析。
- active 路径不再引用被移除的根文档或 `codex-operator-guides/`。
- V1 验收生成器、治理脚本和 Rust contract tests 都读取新 canonical 路径。
- 三份 source manifest 与预期 Git tree 一致。
- 所有实际运行命令退出码为 0，且没有把 dirty-worktree 或未运行检查解释为 PASS。
