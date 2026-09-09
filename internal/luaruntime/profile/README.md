# Platform Lua 5.5 p1 implementation

`vm.New` is the host entrypoint. It consumes an immutable validated B011 package,
its exact dependencies, authoritative state, explicit limits, an absolute runner
executable and a mandatory AUDIT-0 sink. Each Session owns one `lua-runner serve`
process; package code only runs there. `profile.New` is the in-process engine used
inside that worker and by focused conformance tests, not a host sandbox API.

The pure Go backend is `github.com/iceisfun/golua/v2 v2.0.5`, fixed by go.mod and
go.sum. Production startup checks the compiled dependency/version/checksum and
Go 1.26.5 and rejects replacement modules. Its MIT notice is embedded and available
through `lua-runner licenses`. This does not change the repository's license.

Core execution is supported on Linux amd64. Other targets compile the explicit
unsupported-Core path; compilation is not native execution or a new support claim.
The production binary must remain free of cgo-linked dependencies: the all-thread
Linux sandbox primitive fails closed if unavailable. No native Lua/C module is used.

## Versioned restrictions

Only UTF-8 source up to 256 KiB is loaded; no bytecode, filesystem loader, REPL,
debug, OS, environment, network, process, C library or credential provider exists.
The standard loader is replaced by immutable package modules: `require("lua.util")`
selects `lua/util.lua`; `require("example.test/library:lua.util")` selects the exact
locked dependency's module. Missing, cyclic, conflicting and traversal names fail.
All modules share the Session invocation budget and have per-Session instances.

Basic/table/string/utf8/math/coroutine are restricted. `math.random`, randomseed,
wall time and host callbacks are absent: authoritative random/time grants belong
to B004. `pairs`/`next` sort primitive keys; opaque keys and pointer formatting are
rejected. `tostring` accepts primitive values. Metatables and coroutine execution
remain available locally but never cross the checkpoint boundary. Hidden coroutine
creation through `wrap` also receives the instruction and recursion hooks.

The default ceilings (only reducible) are 100,000 instructions, 500 ms wall time,
128 MiB Linux data/anonymous writable mappings, 2 CPU seconds, 128 aggregate call
depth and 64 KiB output. A Linux RLIMIT_DATA hard ceiling bounds Go heap allocations;
GOMEMLIMIT only assists collection. RLIMIT_CPU, an independent process wall timer,
parent cancellation/kill and instruction hooks cover native library work as well
as Lua loops. AUDIT-0 cannot be disabled. NoNewPrivs applies to all runtime threads,
core dumps are disabled and the child receives no inherited application secrets.

## Lifecycle and recovery

Execution requires an opaque, nonserializable per-generation token. Host callers
serialize each Session's requests through its mutex. Tokens never enter Lua or IPC.
Any failed executed command, result conversion, IPC operation or audit write makes
the Session unusable until reconstruction. A captured checkpoint must be one basic
acyclic value; numbers preserve int64 or finite restricted float values, arrays are
dense and tables have UTF-8 string keys. Depth, nodes, bytes and JSON ambiguity are
bounded. Functions, coroutine state, metatables, opaque handles and capability
token strings are rejected.

Checkpoint bindings include Session identity, state version, every package content
hash, canonical exact-lock digest, profile and runtime version. `Reconstruct`
requires authoritative state and an optional exactly matching checkpoint. It starts
a fresh worker with those inputs, executes the same entrypoint, then replaces the
old worker and invalidates old tokens. Globals are caches, never persisted facts.
Memory-pressure rebuilding first captures the current state version successfully;
a failed capture cannot promote stale cache data or replace the worker. Destruction
reaps the process and its coroutines. Audit sinks must not re-enter that Session.

Package capabilities are resolved through the existing declaration/trust/context
intersection, with empty grants until B004. Required capabilities fail closed.
Optional capabilities require a trusted caller's already-tested fallback evidence,
bound to the exact package hash and declared behavior. `FallbackProof` is an input
to that trusted validation boundary, not a package-supplied claim or a capability.

## Candidate evidence

From a clean frozen candidate, build the runner and execute:

```
/absolute/lua-runner evidence --candidate-sha <full SHA> --output-dir <new external directory>
```

This executes the original TEST-LUA-001/002 directories, records exact SHA/tree,
commands, exit codes, effective and inherited Go settings, JSON run/pass events and
artifact hashes. It rejects GOFLAGS overrides (including `-run=^$`), external Go
workspaces, zero/missing/skipped/failed required tests and candidate changes.
Precommit Builder logs are development evidence; independent ACCEPT performs the
full required suite against the immutable candidate in a fresh context.
