# Migrate Player State Store To Portable JSON

## Objective
- Replace Node-only binary player-state persistence with portable JSON text so CLI state files can be consumed cross-platform/runtime.

## Scope
- In scope:
  - Change CLI player-state schema to `player-state.v2`.
  - Replace `v8.serialize/deserialize` state I/O with JSON parse/stringify plus portable value codec.
  - Preserve runtime snapshot behavior while supporting `Map` and non-finite numbers in serialized payloads.
  - Update CLI docs/default state-file path to `.json`.
  - Update unit tests for new storage format and breaking non-compat behavior.
- Out of scope:
  - Script source format migration (`.script.xml/.defs.xml` remain unchanged).
  - Snapshot schema changes (`snapshot.v2` remains unchanged).

## Interfaces / Contracts Affected
- Public API changes:
  - TUI default state file path changes to `./.scriptlang/save.json`.
  - Agent/TUI state files are JSON text payloads instead of Node binary blobs.
- Snapshot/schema changes:
  - Player state schema changes from `player-state.v1` to `player-state.v2`.
  - Runtime snapshot schema remains `snapshot.v2`.
- XML syntax/semantic changes:
  - None.

## Implementation Steps
1. Update product/user docs and examples to describe JSON state-file format and `.json` default path.
2. Replace state-store implementation with JSON parser/stringifier and internal portable codec for `Map` + non-finite numbers.
3. Keep existing state validation semantics (`CLI_STATE_NOT_FOUND`, `CLI_STATE_SCHEMA`, `CLI_STATE_INVALID`) with JSON parse errors mapped to `CLI_STATE_INVALID`.
4. Update CLI/unit tests to remove `v8` assumptions, add invalid-JSON coverage, and add map/non-finite roundtrip resume coverage.
5. Run full gate (`npm test`), then move this plan to completed in the same delivery commit.

## Verification
- Unit tests:
  - `test/unit/cli/core/state-store.test.ts`
  - `test/unit/cli/commands/agent.test.ts`
  - `test/unit/cli/commands/tui.test.ts`
- Integration tests:
  - `npm run player:agent -- start --scripts-dir examples/scripts/06-snapshot-flow --state-out /tmp/sl-state.json`
  - `npm run player:agent -- choose --state-in /tmp/sl-state.json --choice 0 --state-out /tmp/sl-next.json`
- Full gate:
  - `npm test`

## Risks and Mitigations
- Risk:
  - JSON cannot natively encode `Map` and non-finite numbers.
  - Mitigation:
    - Add explicit tagged portable codec with strict decode validation.
- Risk:
  - Existing `.bin` files fail to resume after schema bump.
  - Mitigation:
    - Keep deliberate non-compat policy and return explicit `CLI_STATE_INVALID`/`CLI_STATE_SCHEMA`.

## Rollout
- Migration notes:
  - Regenerate state files with `agent start` or TUI save after this change.
  - Prefer `.json` suffix for `--state-file`, `--state-in`, `--state-out`.
- Compatibility notes:
  - Deliberate breaking change during active development; `player-state.v1` binary payloads are not supported.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
