# fixture-minimal

SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

This internal Linux M1 certification fixture contains two versions of a synthetic
counter and an exact five-role graph: game-system, library, content, assets, and
ui-extension. Each source packet carries its original rights-declared TOML,
UTF-8 source files, exact dependency lock, content hash, and artifact identity.
The catalog binds every packet by SHA-256. No binary is part of this fixture.

`fixed-replay-v1.json` specifies commands +7 and +3, recorded time/random inputs,
expected states 1/8/11, event payloads, GM/player views, their hashes, a complete
expected snapshot, the reconstructible checkpoint value, and the safe-boundary
counter→total / docs.score→records.value plan. Workspace, Session, and source-built
Runner bindings are validated at execution time. The private marker and fixed
publisher keys are artificial test data.

The loader rebuilds the exact archive model from these source packets and checks
every byte and identity against the existing internal daemon fixture definition.
The real installer then executes certification and authenticates that graph.
The daemon's `m1-fixture` / `m1-migration-fixture` commands use those same checked
source bytes. The fixture exercises only minimal Session behavior.

Reproduce the source packets with `go run ./tests/fixture-minimal/export` and
review the resulting diff. This command exports source and fixed expectations;
it does not execute or certify the package.

The acceptance command is
`go test -json -tags=m1_acceptance ./tests/m1/... ./tests/fixture-minimal/...`.
Provide the disposable PostgreSQL DSNs documented by the test suite. It must
run on a clean, exact candidate and emits JSON artifacts with that commit SHA.
Linux M1 deployment smoke uses the isolated `m1-fixture` Compose profile.
Windows/macOS and production operations are outside this fixture's evidence.
