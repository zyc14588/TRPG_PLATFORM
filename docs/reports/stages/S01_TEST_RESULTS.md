# S01 Test Results

## Commands

```text
cargo fmt --all -- --check
cargo clippy -p trpg-shared-kernel --all-targets --all-features -- -D warnings
cargo test -p trpg-shared-kernel --all-features
cargo test --workspace --all-features
```

## Result

All commands exited 0.

```text
29 integration contract tests passed.
0 failed.
Doc-tests passed.
```

## Boundary Checks

No matches were found in `crates/trpg-shared-kernel/src` or
`crates/trpg-shared-kernel/tests` for:

```text
ModuleService|ModuleCommand|ModuleError|serde_json::Value
openai|ollama|llama\.cpp|chat_completion|responses
source-archive|generated-from-source|docs-implementation|v5|V5|v4|V4
```

