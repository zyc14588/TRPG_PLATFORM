# P1.5 Blockers

## License strategy resolved

Status: resolved.

Maintainer decision date: 2026-06-27.

The maintainer selected `AGPL-3.0-or-later` as the project license. The repository license text, workspace package metadata, crate manifest inheritance, and README legal note are now aligned:

- `LICENSE` contains the GNU Affero General Public License v3 text.
- `Cargo.toml` workspace package metadata declares `AGPL-3.0-or-later`.
- Rust crate manifests inherit the workspace `AGPL-3.0-or-later` license.
- `README.md` has a license section declaring `AGPL-3.0-or-later`.

## Server module boundary note for P2

`crates/server/src/lib.rs` still contains config, DTOs, route handlers, auth helpers, in-memory test store, and tests in one module. P2 RAG endpoints should be added behind small route/DTO modules instead of appending another large feature block to this file.
