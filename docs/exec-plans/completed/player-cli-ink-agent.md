# ScriptLang Player CLI (Ink + Agent)

## Objective
- Deliver a playable ScriptLang CLI with two modes:
  - `tui` for interactive play.
  - `agent` for non-interactive command orchestration.

## Scope
- In scope:
  - ESM migration (`package.json`, TypeScript module settings, import paths).
  - `scriptlang-player` binary and npm scripts.
  - Ink-powered TUI mode with save/load/restart/help/quit controls.
  - Agent mode with line-based protocol and state handoff.
  - `examples/scripts` scenario set (6 core examples).
  - Unit tests for CLI behavior and regression checks.
- Out of scope:
  - Web player.
  - macOS app packaging.
  - ScriptLang syntax/runtime semantic changes.

## Interfaces / Contracts Affected
- Public API changes:
  - Add CLI binary: `scriptlang-player`.
  - Add CLI commands:
    - `tui --example <id> [--state-file <path>]`
    - `agent list`
    - `agent start --example <id> --state-out <path>`
    - `agent choose --state-in <path> --choice <index> --state-out <path>`
- Snapshot/schema changes:
  - none (reuse `snapshot.v1`).
- XML syntax/semantic changes:
  - none.

## Implementation Steps
1. Update product specs and docs index for player CLI behavior.
2. Migrate project to ESM (NodeNext module mode + `.js` relative imports).
3. Add scenario registry and examples under `examples/scripts`.
4. Add shared CLI core:
   - argument parsing
   - run-to-boundary engine execution
   - binary state store
5. Implement `agent` subcommands and line protocol output.
6. Implement Ink TUI mode with keyboard controls.
7. Update README with human-play and agent-play usage.
8. Add tests and verify full gate.

## Verification
- Unit tests:
  - `test/cli-agent.test.ts`
  - `test/cli-tui-core.test.ts`
  - `test/esm-migration-smoke.test.ts`
- Integration tests:
  - command-level agent flow (`list/start/choose`).
- Manual checks:
  - `scriptlang-player tui --example 04-call-ref-return`
  - `scriptlang-player agent start ...` then `choose ...`

## Risks and Mitigations
- Risk: ESM migration breaks imports.
  - Mitigation: convert all local imports to explicit `.js` paths and run full typecheck/tests.
- Risk: `Map` state corruption in serialization.
  - Mitigation: use Node `v8.serialize/deserialize` for binary state files.
- Risk: agent parser fragility.
  - Mitigation: fixed-prefix line protocol with JSON payload lines.

## Rollout
- Migration notes:
  - consumers should use ESM-compatible import/runtime.
- Compatibility notes:
  - language runtime behavior remains unchanged.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed
