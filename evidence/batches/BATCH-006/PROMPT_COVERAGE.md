# BATCH-006 Prompt Coverage

## Read Inputs

- AGENTS.md
- CODEX_STANDALONE_BOOTSTRAP_PROMPT.md
- SOURCE_BUNDLE_INTEGRATION_GUIDE.md
- docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md
- docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md
- docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md
- docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md
- CODEX_MASTER_EXECUTION_GUIDE.md
- CODEX_START_ACCEPT_TEST_RELEASE_GUIDE.md
- CODEX_STRICT_OPERATION_CHECKLIST.md
- codex-operator-guides/README.md
- V1_ACCEPTANCE_EVIDENCE_MATRIX.md
- PER_STAGE_FIXTURE_EXPANSION_PLAN.md
- docs/codex/00-index/codex-persistent-context.md
- docs/codex/00-index/codex-prompt-boundary.md
- stages/s01-foundation-shared-kernel/README.md
- stages/s01-foundation-shared-kernel/START_PROMPT.md
- stages/s01-foundation-shared-kernel/TEST_PLAN.md
- stages/s01-foundation-shared-kernel/TEST_DATA.md
- stages/s01-foundation-shared-kernel/ACCEPTANCE_PROMPT.md
- stages/s01-foundation-shared-kernel/REPAIR_PROMPT.md
- batches/B006.md
- codex-prompts/01-foundation/P0078.md
- codex-prompts/01-foundation/P0084.md

## Per-file Prompts

| Prompt | Prompt ID | Role observed | Repair action | SHA256 |
| --- | --- | --- | --- | --- |
| P0076.md | CODEX-0214-01-FOUNDATION-79c289242e | documentation-or-traceability | Markdown traceability output exists. | 6c2cb5ea4cdf67c7f7fd773efe13ed1bac90e44215342db48433f9908be5f208 |
| P0077.md | CODEX-0215-01-FOUNDATION-411484506a | documentation-or-traceability | Markdown traceability output exists. | e5ec547529560ef247c53eb2f5e2dd8dc1369fd751582b52e011f5628e1f64e4 |
| P0078.md | CODEX-0216-01-FOUNDATION-d39bc6ef34 | primary-implementation | Implemented current-safe Rust src/test. | af2dee9d1a3de347d75eb0815960f62fc6d80830f043e822446a4135290a690d |
| P0079.md | CODEX-0217-01-FOUNDATION-6163fad1ce | supplemental-requirement | Recorded only; no Rust output. | e38b402a76b79612a77de37dd679f189016803835a275909a889659085820830 |
| P0080.md | CODEX-0218-01-FOUNDATION-3659d645a7 | supplemental-requirement | Recorded only; no Rust output. | f55f9327bad1272630096726f8eefe23032cf68e0115181f0a1e6d75522fc214 |
| P0081.md | CODEX-0219-01-FOUNDATION-e312e3694b | supplemental-requirement | Recorded only; no Rust output. | c46be12ea407024fd81141c91a7d2e27b5e0489eecd7b4ee1bbaa68570f8e194 |
| P0082.md | CODEX-0220-01-FOUNDATION-e6fa26b126 | supplemental-requirement | Recorded only; no Rust output. | 62d57842677fd50af800c507da6f452057b9e87a650b993e913c99dcb46c4bb9 |
| P0083.md | CODEX-0221-01-FOUNDATION-e2b2757285 | supplemental-requirement | Recorded only; no Rust output. | 6e1b4495d542c7e3be64f29999e7120054a01028b945ebec7e40ca0ab39cfd21 |
| P0084.md | CODEX-0222-01-FOUNDATION-c1599cb21d | primary-implementation | Implemented current-safe Rust src/test. | a371ce5a8f0173c65a6ab0b248b3fc0eba40ce751fc8dc1ba87ad680726422d2 |
| P0085.md | CODEX-0223-01-FOUNDATION-809f614768 | supplemental-requirement | Recorded only; no Rust output. | 5b883e225dc48b4d982ce51cb1f2a623a64ef8d8e8ee0f9a3f09cb606b2fc95a |
| P0086.md | CODEX-0224-01-FOUNDATION-b690e9cf59 | supplemental-requirement | Recorded only; no Rust output. | ade662486392f03f3f62ee34b385b356000f8bba31ceb99766fc57e07efffde3 |
| P0087.md | CODEX-0225-01-FOUNDATION-1d889d2a5d | supplemental-requirement | Recorded only; no Rust output. | 0b6fedebe2913000d8466b61128a2d0ee1553a26ffd91d6f4ad21282338a94b1 |
| P0088.md | CODEX-0226-01-FOUNDATION-6ba5e9acb1 | supplemental-requirement | Recorded only; no Rust output. | e5189a644880c136bfc9dfac2b3a2414da9be92f530206f6c0a8fc382d0bd7ac |
| P0089.md | CODEX-0227-01-FOUNDATION-52cad60d03 | supplemental-requirement | Recorded only; no Rust output. | fe967c6a0543d61365c0131155860e3c20beff45d5d50c1590519bc800bd773f |
| P0090.md | CODEX-0228-01-FOUNDATION-f021763065 | supplemental-requirement | Recorded only; no Rust output. | 312fd270dc75aa42a1a780cf8bd7277620ffd0d0b12172028d6fb3be822e8cb2 |
| P0091.md | CODEX-0229-01-FOUNDATION-979f0dbec3 | supplemental-requirement | Recorded only; no Rust output. | 3d84d1c6f31663f88ae28dec3a9d27a0c8e1d94c07113cfa6f85115f727af87d |
| P0092.md | CODEX-0230-01-FOUNDATION-dd68b1a909 | supplemental-requirement | Recorded only; no Rust output. | 9ffd8db159e1f806695e308c5a8f8e9efd90854c7661c1426c0cc45080582927 |
| P0093.md | CODEX-0231-01-FOUNDATION-656bf64610 | supplemental-requirement | Recorded only; no Rust output. | 3d46a1c67c3160580e0c9f5f5ff0854885d3e6108c9afda43136b7adf7c9e36e |
| P0094.md | CODEX-0232-01-FOUNDATION-66823f36cd | supplemental-requirement | Recorded only; no Rust output. | 33893ffd82fe905268522f0971f7a48faa0cd07bf504af144e3b6d7ed7f63c6e |
| P0095.md | CODEX-0235-01-FOUNDATION-68c446a379 | supplemental-requirement | Recorded only; no Rust output. | c439f5efdf736a4374cff1673d1f547c5b3cdbf48fd52c754f53aa7664857662 |
| P0096.md | CODEX-0236-01-FOUNDATION-76f8f9fc5c | supplemental-requirement | Recorded only; no Rust output. | cd24d9eb73cb2609a8c04bb8934295709a8d56149825a2fd40624ad108f50c7e |
| P0097.md | CODEX-0233-01-FOUNDATION-7a59670976 | supplemental-requirement | Recorded only; no Rust output. | 2d099c35cad4631fbb5d25f5b10d16c3d3fbf9701bd4bb01ef5d5b523e32a56d |
| P0098.md | CODEX-0234-01-FOUNDATION-dc1cae257b | supplemental-requirement | Recorded only; no Rust output. | 3d5951aa1e0f60ed54f309d2337a7a04a94dac183d7fb89c0963edb1bd2d3b37 |

## Coverage Result

- 23 prompts accounted for.
- 2 documentation outputs materialized.
- 2 primary prompts implemented with current-safe Rust src/test outputs.
- 19 supplemental prompts preserved as no-Rust-output constraints.
