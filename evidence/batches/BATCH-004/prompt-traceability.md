# BATCH-004 Prompt Traceability

## Read Inputs

- `AGENTS.md`
- `CODEX_STANDALONE_BOOTSTRAP_PROMPT.md`
- `SOURCE_BUNDLE_INTEGRATION_GUIDE.md`
- `docs/top-level-design/CURRENT_TOP_LEVEL_DESIGN.md`
- `docs/codex/00-index/CURRENT_NORMALIZED_PROMPT_EXECUTION_MAP.md`
- `docs/codex/00-index/CURRENT_SAFE_MODULE_AND_OUTPUT_MAP.md`
- `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md`
- `batches/B004.md`
- S01 README, START, TEST_PLAN, TEST_DATA, ACCEPTANCE, and REPAIR prompts
- B004 per-file prompts `P0023`, `P0024`, `P0027`, and `P0029` through `P0050`
- Required operator guides named by `AGENTS.md`

## Trace Summary

| Prompt ID | Disposition |
|---|---|
| CODEX-0164-01-FOUNDATION-15e9dcbbc4 | Implemented current-safe `constitution` module and contract tests |
| CODEX-0165-01-FOUNDATION-b8fe9316ba | Implemented current-safe `document_set` module and contract tests |
| CODEX-0166-01-FOUNDATION-0d8f080d4d | Implemented current-safe `system_context` module and contract tests |
| CODEX-0167-01-FOUNDATION-03125d8734 | Supplemental only; no B004 Rust output |
| CODEX-0168-01-FOUNDATION-b45ac02ea6 | Supplemental merged into `constitution` tests |
| CODEX-0169-01-FOUNDATION-b05b9a90b3 | Supplemental merged into `document_set` tests |
| CODEX-0170-01-FOUNDATION-306fa4ab16 | Supplemental only; existing open-source reference module remains owner |
| CODEX-0171-01-FOUNDATION-8a4d752663 | Supplemental merged into `system_context` tests |
| CODEX-0172-01-FOUNDATION-e9e4f0b4c1 | Supplemental only; future primary owner not started |
| CODEX-0173-01-FOUNDATION-08cdc4a342 | Supplemental only; existing cargo workspace module remains owner |
| CODEX-0174-01-FOUNDATION-ba41854b91 | Supplemental only; existing config model module remains owner |
| CODEX-0175-01-FOUNDATION-e659435928 | Supplemental only; existing crate ownership module remains owner |
| CODEX-0176-01-FOUNDATION-958c2094d1 | Supplemental only; existing dependency direction module remains owner |
| CODEX-0177-01-FOUNDATION-4971602f41 | Supplemental only; existing error model module remains owner |
| CODEX-0178-01-FOUNDATION-561743b9ad | Implemented current-safe `readme` module and contract tests |
| CODEX-0179-01-FOUNDATION-dc81653f92 | Supplemental only; existing rust coding model module remains owner |
| CODEX-0180-01-FOUNDATION-b6f4482b91 | Supplemental only; existing shared kernel module remains owner |
| CODEX-0181-01-FOUNDATION-5c3ee7877d | Supplemental merged into `readme` tests |
| CODEX-0182-01-FOUNDATION-1226a36865 | Supplemental only; existing rust coding model module remains owner |
| CODEX-0183-01-FOUNDATION-c715e94c4a | Supplemental only; existing shared kernel module remains owner |
| CODEX-0184-01-FOUNDATION-da009f8e1c | Implemented current-safe `workspace_and_governance` module and contract tests |
| CODEX-0185-01-FOUNDATION-2d0624102a | Supplemental merged into `constitution` tests |
| CODEX-0186-01-FOUNDATION-337c5fa749 | Implemented current-safe `cargo_workspace_impl` module and contract tests |
| CODEX-0187-01-FOUNDATION-5869480a21 | Implemented current-safe `constitution_impl` module and contract tests |
| CODEX-0188-01-FOUNDATION-6619678123 | Implemented current-safe `document_set_impl` module and contract tests |

## Boundary Notes

- Prompt IDs and source hashes remain traceability metadata only.
- Rust module names, event names, test names, and output paths use normalized
  current-safe names.
- Supplemental prompts did not create independent Rust outputs.
- No `source-archive/**` path was promoted into implementation naming.
