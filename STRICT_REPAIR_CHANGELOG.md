# Strict Repair Changelog — v2.21

## Fixed P1 findings

1. Rewrote `README.md` as a true project and package entry instead of a repair-history-first document.
2. Fixed all Codex guide files so `##` sections render as Markdown headings rather than indented code blocks.
3. Removed active `CURRENT_TOKEN_REWRITE_TABLE.md` aliases and made `docs/codex/00-index/CURRENT_TOKEN_REWRITE_TABLE.md` the only canonical token rewrite source.
4. Moved duplicate `github-actions-*.yml.md` files formerly stored under `ci-cd/` files to provenance and made `ci-cd/workflows-extractable/target-*.yml.md` the only canonical CI/CD extraction source.
5. Added `DOCUMENT_ORGANIZATION_AND_AUDIT_BOUNDARY.md` to separate audit/design docs from Codex prompt docs and provenance-only materials.
6. Strengthened `STRICT_LINK_AND_REFERENCE_VALIDATION.md` with guide heading, README directory, token canonical, CI canonical and stale-current gates.

## Current baseline

The current package baseline is v2.21 / V221. Earlier package reports and aliases are provenance only and must not be used as current execution authority.

## v2.21 strict extended acceptance repair

- Fixed GitHub Actions service command overrides: NATS and MinIO now use `services.<id>.command`, not Docker `options`.
- Replaced fail-late `cargo install ... || true` patterns in canonical CI with explicit command-existence checks and fail-fast installs.
- Added 52 batch start prompt references and 52 batch acceptance prompt references under ``batch-prompts/start/B###.md` 与 `batch-prompts/accept/B###.md``.
- Added detailed fixtures for S00, S01, S09, S10, and S12.
- Replaced stage old ambiguous stage fixture headings with `v2.21 当前阶段扩展 fixture`.
- Rebuilt strict validation gates for CI semantics, batch prompt completeness, and stage fixture completeness.
