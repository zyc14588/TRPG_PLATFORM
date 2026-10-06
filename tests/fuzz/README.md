# Bounded M1 fuzz targets

SPDX-License-Identifier: PolyForm-Noncommercial-1.0.0

The eight frozen target names cover protocol envelopes, source archive import,
manifest TOML, the offline package-bound Schema loader, Host Callback wire
parameters, authoritative event decoding, declarative Upcasters, and data-only
migration plans. Each target has committed `testdata/fuzz/<target>` seeds and
executes those seeds before Go begins mutation. Inputs are capped at 64 KiB
(16 KiB for schema definitions); migration snapshots and authority identities
remain fixed trusted test inputs.

Run each exact frozen command with `-run='^$' -fuzz='^<target>$' -fuzztime=30s`
and `GOMAXPROCS=2`. The run must be tied to the exact clean candidate in M1
evidence. Minimized failures remain failures, are copied into that target's
corpus, and return to the owning component's repair route.

Callback fuzzing covers production strict decoding, operation registration,
value limits, and immutable package-bound parameter validation. Real token,
origin, capability-intersection and phase checks run in the separate Host/VM
and security M1 exit gates. This fuzz suite executes no SQL, model, generated
Session command sequence, or package-supplied migration script.
