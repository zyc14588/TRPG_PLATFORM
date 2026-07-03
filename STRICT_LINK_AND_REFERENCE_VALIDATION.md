# Strict Link and Reference Validation — v2.21

本文档是 v2.21 当前包的硬性校验入口。Codex 或人工验收人员应在解压后的包根目录执行以下 Python 代码块。所有代码块必须 PASS。旧路径只允许出现在 `inventory/PATH_REWRITE_MAP.md` 的 `old_path` 或 provenance/source 字段中，不得作为当前操作入口。

## 1. Manifest 与文件集合闭合

```python
from pathlib import Path
import hashlib
import re
root = Path.cwd()
manifest = root / 'MANIFEST.md'
assert manifest.exists(), 'MANIFEST.md missing'
md_files = sorted(p.relative_to(root).as_posix() for p in root.rglob('*.md'))
rows = {}
for line in manifest.read_text(encoding='utf-8').splitlines():
    m = re.match(r"\| `([^`]+)` \| ([^|]+) \| `?([^`|]+)`? \|", line)
    if m and m.group(1) != 'path':
        rows[m.group(1)] = (m.group(2).strip(), m.group(3).strip())
assert set(rows) == set(md_files), f'manifest mismatch: missing={set(md_files)-set(rows)}, extra={set(rows)-set(md_files)}'
self_ref = {'MANIFEST.md', 'manifests/CURRENT_PACKAGE_MANIFEST.md', 'manifests/SELF_CONTAINED_PACKAGE_MANIFEST.md'}
for rel in md_files:
    size, sha = rows[rel]
    if rel in self_ref:
        assert size == 'SELF-REFERENTIAL' and sha == 'SELF-REFERENTIAL', rel
        continue
    data = (root / rel).read_bytes()
    assert int(size) == len(data), rel
    assert sha == hashlib.sha256(data).hexdigest(), rel
print('PASS manifest', len(md_files))
```

## 2. Cleanup audit 覆盖全包

```python
from pathlib import Path
import re
root = Path.cwd()
audit = root / 'inventory/V221_FULL_FILE_CLEANUP_AUDIT.md'
assert audit.exists(), 'V221_FULL_FILE_CLEANUP_AUDIT.md missing'
md_files = sorted(p.relative_to(root).as_posix() for p in root.rglob('*.md'))
text = audit.read_text(encoding='utf-8')
m = re.search(r'Markdown files traversed:\s*(\d+)', text)
assert m and int(m.group(1)) == len(md_files), 'cleanup audit declared count mismatch'
rows = []
for line in text.splitlines():
    m = re.match(r"\| `([^`]+)` \|", line)
    if m and m.group(1) != 'path':
        rows.append(m.group(1))
assert sorted(rows) == md_files, 'cleanup audit rows do not match package files'
print('PASS cleanup audit', len(rows))
```

## 3. 结构深度与文件名长度

```python
from pathlib import Path
root = Path.cwd()
for p in root.rglob('*.md'):
    rel = p.relative_to(root).as_posix()
    parent_depth = len(p.relative_to(root).parent.parts)
    assert parent_depth < 4, f'parent depth >=4: {rel}'
    assert len(p.name) <= 96, f'basename too long: {rel}'
    assert len(rel) <= 180, f'relative path too long: {rel}'
print('PASS structure')
```

## 4. 行级 Markdown fence parity 全包检查

```python
from pathlib import Path
import re
root = Path.cwd()
fence_re = re.compile(r'^ {0,3}```')
issues = []
for p in root.rglob('*.md'):
    lines = p.read_text(encoding='utf-8', errors='replace').splitlines()
    count = sum(1 for line in lines if fence_re.match(line))
    if count % 2:
        issues.append((p.relative_to(root).as_posix(), count))
assert not issues, issues[:20]
print('PASS line-level fence parity')
```

## 5. Batch 与 per-file prompt 引用闭合

```python
from pathlib import Path
import re
root = Path.cwd()
prompt_ids = {}
for p in root.glob('codex-prompts/*/P*.md'):
    text = p.read_text(encoding='utf-8', errors='replace')
    m = re.search(r'Prompt ID[：:]\s*`?([A-Za-z0-9\-]+)`?', text)
    assert m, f'Prompt ID missing: {p}'
    pid = m.group(1)
    assert pid not in prompt_ids, f'duplicate Prompt ID: {pid}'
    prompt_ids[pid] = p.relative_to(root).as_posix()
rows = 0
for b in sorted(root.glob('batches/B*.md')):
    for line in b.read_text(encoding='utf-8', errors='replace').splitlines():
        if not line.startswith('| CODEX-'):
            continue
        cols = [c.strip() for c in line.strip('|').split('|')]
        pid = cols[0]
        prompt_path = cols[-1].strip('`')
        assert (root / prompt_path).exists(), f'missing prompt path {prompt_path} in {b}'
        assert prompt_ids.get(pid) == prompt_path, f'Prompt ID path mismatch {pid}: {prompt_ids.get(pid)} != {prompt_path}'
        rows += 1
assert len(prompt_ids) == 1109, len(prompt_ids)
assert rows == 1109, rows
assert len(list(root.glob('batches/B*.md'))) == 52
print('PASS batch prompt refs', rows)
```

## 6. Manual batch prompt 与中文提示词门禁

```python
from pathlib import Path
import re
root = Path.cwd()
starts = sorted(root.glob('batch-prompts/start/B*.md'))
accepts = sorted(root.glob('batch-prompts/accept/B*.md'))
assert len(starts) == 52 and len(accepts) == 52, (len(starts), len(accepts))
manual_files = [root/'CODEX_MASTER_EXECUTION_GUIDE.md', root/'CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md', root/'CODEX_STRICT_OPERATION_CHECKLIST.md', root/'CODEX_RELEASE_PREPARATION_GUIDE.md', root/'codex-operator-guides/09_CODEX_SESSION_PROMPTS.md'] + starts + accepts
cn_re = re.compile(r'[\u4e00-\u9fff]')
for p in manual_files:
    text = p.read_text(encoding='utf-8', errors='replace')
    assert cn_re.search(text), f'no Chinese instruction text: {p}'
    if 'Copy-ready' in text or '可复制' in text or p.name.startswith('B'):
        assert any(v in text for v in ['请', '必须', '不得', '验收', '执行', '读取']), f'weak Chinese directive: {p}'
print('PASS manual prompts')
```

## 7. CI/CD canonical source 与 YAML 解析

```python
from pathlib import Path
import yaml
root = Path.cwd()
workflows = sorted(root.glob('ci-cd/workflows-extractable/target-*.yml.md'))
assert len(workflows) == 5, len(workflows)
for p in workflows:
    text = p.read_text(encoding='utf-8')
    parts = text.split('```yaml')
    assert len(parts) >= 2, f'no yaml block: {p}'
    block = parts[1].split('```', 1)[0]
    data = yaml.safe_load(block)
    assert data and 'jobs' in data, f'invalid workflow: {p}'
    assert 'options: -js' not in block and 'options: server /data' not in block, f'bad service options: {p}'
print('PASS ci workflows', len(workflows))
```

## 8. Detailed fixture 充分性

```python
from pathlib import Path
import json
import re
root = Path.cwd()
required = {'inputs','actions','expected_events','expected_records','expected_errors','failure_cases','required_evidence','automation_target','pass_criteria'}
fixtures = sorted(root.glob('fixtures/stages/detailed/S*.md'))
assert len(fixtures) == 14, len(fixtures)
covered = set()
for p in fixtures:
    text = p.read_text(encoding='utf-8')
    m = re.search(r'```json\n(.*?)\n```', text, re.S)
    assert m, f'no json fixture: {p}'
    data = json.loads(m.group(1))
    missing = required - set(data)
    assert not missing, f'{p}: missing {missing}'
    sm = re.search(r'S(\d{2})', p.name)
    assert sm, p.name
    covered.add(sm.group(1))
assert covered == {f'{i:02d}' for i in range(14)}, covered
print('PASS detailed fixtures')
```

## 9. 当前路径引用与旧路径回流检查

```python
from pathlib import Path
root = Path.cwd()
active = [p for p in root.rglob('*.md') if 'source-archive' not in p.parts and p.relative_to(root).as_posix() != 'inventory/PATH_REWRITE_MAP.md']
text = '\n'.join(p.read_text(encoding='utf-8', errors='replace') for p in active)
tokens = [
    'source-' + 'materials/',
    'docs/codex/90-' + 'traceability/per-file-prompts/',
    'docs/codex/90-' + 'traceability/execution-batches/',
    'codex-operator-guides/' + 'batch-prompts/',
    'BATCH-001-' + 'START_PROMPT.md',
    'BATCH-001-' + 'ACCEPTANCE_PROMPT.md',
]
for token in tokens:
    assert token not in text, f'old active path reference: {token}'
assert 'source-archive/' in text
print('PASS active path references')
```

## 10. README 当前验收入口与路径映射完整性

```python
from pathlib import Path
import re
root = Path.cwd()
readme = (root/'README.md').read_text(encoding='utf-8')
required = ['STRICT_SELF_CONTAINED_ACCEPTANCE_REPORT.md', 'STRICT_V221_ACCEPTANCE_REPORT.md', 'V221_FULL_PACKAGE_MARKDOWN_CLEANUP_REPORT.md', 'manifests/V221_STRICT_VALIDATION_REPORT.md', 'inventory/V221_FULL_FILE_CLEANUP_AUDIT.md']
for rel in required:
    assert rel in readme, f'README missing current report reference: {rel}'
    assert (root/rel).exists(), f'README references missing file: {rel}'
for line in (root/'inventory/PATH_REWRITE_MAP.md').read_text(encoding='utf-8').splitlines():
    m = re.match(r"\| `([^`]+)` \| `([^`]+)` \|", line)
    if m and m.group(1) != 'old_path':
        new_path = m.group(2)
        assert (root/new_path).exists(), f'PATH_REWRITE_MAP new_path missing: {new_path}'
print('PASS README and path rewrite map')
```

## 11. Active 当前语义与历史边界

```python
from pathlib import Path
root = Path.cwd()
active_text = '\n'.join(p.read_text(encoding='utf-8', errors='replace') for p in root.rglob('*.md') if 'source-archive' not in p.parts)
old_markers = [
    'v2.' + '20 当前',
    'V' + '220 Strict',
    'PENDING_' + 'UNTIL_ARCHIVE',
]
for old in old_markers:
    assert old not in active_text, old
assert 'v2.21' in active_text
assert 'docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md' in active_text
assert not (root/'docs/codex/00-index/LEGACY_TOKEN_REWRITE_TABLE.md').exists()
print('PASS current semantics')
```
