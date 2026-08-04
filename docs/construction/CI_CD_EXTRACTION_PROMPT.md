# CI/CD Extraction Prompt — v2.21 Canonical Workflow 落库

你是 Codex，负责把本施工包中的 Markdown-only workflow 源转为目标仓库可执行 GitHub Actions YAML。必须使用唯一 canonical source，不得混用历史 `github-actions-*.yml.md` provenance 文件。

## 1. Canonical input files

```text
ci-cd/workflows-extractable/target-ci.yml.md
ci-cd/workflows-extractable/target-contracts.yml.md
ci-cd/workflows-extractable/target-golden-scenarios.yml.md
ci-cd/workflows-extractable/target-docker-compose-smoke.yml.md
ci-cd/workflows-extractable/target-release.yml.md
```

## 2. Target output files

```text
.github/workflows/ci.yml
.github/workflows/contracts.yml
.github/workflows/golden-scenarios.yml
.github/workflows/docker-compose-smoke.yml
.github/workflows/release.yml
```

## 3. Extraction rules

- Extract only the fenced `yaml` block from each canonical source file.
- Do not copy Markdown headings, prose, or triple backticks into `.github/workflows/*.yml`.
- Preserve GitHub Actions expressions such as `${{ github.ref }}` exactly.
- If a workflow references a script that does not exist yet, create the script in the owning stage or mark the stage blocked; never delete the gate to make CI green.
- Do not read `source-archive/provenance/**` except to explain provenance.

## 4. Validation command

```powershell
New-Item -ItemType Directory -Force .github/workflows | Out-Null
python - <<'PY_CI_EXTRACT'
from pathlib import Path
import re, yaml
mapping = {
  'ci-cd/workflows-extractable/target-ci.yml.md': '.github/workflows/ci.yml',
  'ci-cd/workflows-extractable/target-contracts.yml.md': '.github/workflows/contracts.yml',
  'ci-cd/workflows-extractable/target-golden-scenarios.yml.md': '.github/workflows/golden-scenarios.yml',
  'ci-cd/workflows-extractable/target-docker-compose-smoke.yml.md': '.github/workflows/docker-compose-smoke.yml',
  'ci-cd/workflows-extractable/target-release.yml.md': '.github/workflows/release.yml',
}
for src, dst in mapping.items():
    text = Path(src).read_text(encoding='utf-8')
    fence = '`' * 3
    m = re.search(fence + r'ya?ml\s*\n(.*?)\n' + fence, text, re.S | re.I)
    assert m, src
    yaml_text = m.group(1).rstrip() + '\n'
    yaml.safe_load(yaml_text)
    Path(dst).parent.mkdir(parents=True, exist_ok=True)
    Path(dst).write_text(yaml_text, encoding='utf-8')
for dst in mapping.values():
    text = Path(dst).read_text(encoding='utf-8')
    assert ('`' * 3) not in text, dst
    assert 'name:' in text and 'jobs:' in text, dst
print('workflow extraction ok')
PY_CI_EXTRACT
```

## 5. Expected evidence

Write extraction evidence to:

```text
evidence/ci/WORKFLOW_EXTRACTION.md
evidence/ci/YAML_PARSE_OUTPUT.txt
evidence/ci/WORKFLOW_DIFF.md
```
