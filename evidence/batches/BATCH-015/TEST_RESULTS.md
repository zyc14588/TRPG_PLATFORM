# BATCH-015 Test Results

- Batch: `BATCH-015-03-runtime-orchestration`
- Date: 2026-07-04
- Scope: documentation-or-traceability and supplemental requirement Markdown only.

## Minimal Scope Checks

| Command | Result | Notes |
| --- | --- | --- |
| `git diff --name-only -- crates/trpg-runtime/src crates/trpg-runtime/tests` | PASS | No runtime Rust source or Rust test diffs. |
| B015 target file existence PowerShell check | PASS | `expected=25 missing=0`. |
| changed/untracked path legacy-token check | PASS | Final check: `changed_or_untracked_paths=29 bad_current_paths=0`. |
| changed/untracked runtime src/test path check | PASS | Final check: `changed_or_untracked_paths=29 runtime_src_or_test_paths=0`. |
| `rg -n "OpenAI\|openai\|Ollama\|ollama\|llama\|chat_completion\|responses\\.create\|OPENAI_API_KEY\|reqwest\|ureq\|hyper::Client\|ProviderAdapter\|ModelProvider" crates\\trpg-runtime\\src crates\\trpg-runtime\\tests --glob "*.rs"` | PASS | No matches; command returned exit code 1 because ripgrep found no direct-provider patterns. |

The broad historical-token grep against `docs/codex/03-runtime-orchestration` found existing governance text and one B015 supplemental rule that explicitly says legacy tokens are forbidden as current names. This is not a current output-name violation.

## Stage Checks

| Command | Result | Notes |
| --- | --- | --- |
| `cargo fmt --all -- --check` | PASS | Warning only: `could not canonicalize path C:\\Users\\zyc14588`. |
| `cargo check --workspace --all-features` | PASS | Finished `dev` profile successfully. |
| `cargo test -p trpg-runtime --all-features --jobs 1` | PASS | 43 runtime integration tests passed; 0 failed. Warning only: `could not canonicalize path C:\\Users\\zyc14588`. |
| `cargo test --test runtime_pending_decision --jobs 1` | PASS | 1 passed; 0 failed. Warning only: `could not canonicalize path C:\\Users\\zyc14588`. |
| `cargo test --test workflow_engine_contract --jobs 1` | PASS | 1 passed; 0 failed. Warning only: `could not canonicalize path C:\\Users\\zyc14588`. |

## Test Responsibility Decision

BATCH-015 has zero primary implementation prompts, so this batch did not create or modify Rust test files. The test responsibility is satisfied by:

- documenting the supplemental assertions future primary prompts must merge;
- verifying no Rust source/test files were touched;
- running the S06 runtime orchestration checks that already cover authority, pending decision, workflow/event pipeline, tool gate, visibility, provenance, realtime, saga, scheduler, and direct agent write rejection.
