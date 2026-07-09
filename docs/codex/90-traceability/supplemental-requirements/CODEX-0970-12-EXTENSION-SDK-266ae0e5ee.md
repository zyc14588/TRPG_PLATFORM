# Supplemental Requirement: Extension SDK Readme Boundary

Prompt ID: `CODEX-0970-12-EXTENSION-SDK-266ae0e5ee`
Primary Prompt: `CODEX-0957-12-EXTENSION-SDK-2c33b70efe`
Current-safe module: `extension_sdk::readme`

## Merge Result

P0: The SDK overview must keep the canonical boundary explicit: extensions propose or record governed SDK facts, while formal game state remains on the command, workflow, decision, event, and projection path.

P1: Player-facing UI must not expose developer-only debug, Tool Gate internals, or restricted extension traces.

## Boundary

This supplemental prompt owns no Rust src/test output. Implementation changes, if needed later, belong to the primary prompt above.

