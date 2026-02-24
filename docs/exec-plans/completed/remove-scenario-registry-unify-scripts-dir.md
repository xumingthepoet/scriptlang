# Remove Scenario Registry And Unify Scripts-Dir Source

## Objective
- Remove special treatment for built-in examples so CLI source handling matches ordinary ScriptLang project directories.

## Scope
- In scope:
  - Remove scenario registry and `--example` source path from CLI.
  - Use only `--scripts-dir` in TUI and agent start flows.
  - Remove `agent list` subcommand.
  - Make state restore accept only `scripts-dir:<absolute-path>` scenario refs.
  - Update docs and tests for the breaking CLI behavior.
- Out of scope:
  - Runtime semantics and language syntax changes.
  - Snapshot schema version bump.

## Interfaces / Contracts Affected
- Public API changes:
  - CLI removes `agent list`.
  - CLI removes `--example` source selector.
  - `tui` command now requires `--scripts-dir <path>`.
  - `agent start` now requires `--scripts-dir <path>`.
- Snapshot/schema changes:
  - No schema change; `player-state.v1` remains.
  - `scenarioId` stored by `start` remains a string, now always `scripts-dir:<absolute-path>`.
- XML syntax/semantic changes:
  - None.

## Implementation Steps
1. Update product and user docs to remove `--example`/`agent list` and describe `--scripts-dir` as the only source mode.
2. Introduce `src/cli/core/source-loader.ts` for scripts-dir loading and scenario-ref parsing.
3. Remove `src/cli/core/scenario-registry.ts` and switch all CLI imports/types to source-loader.
4. Update agent/tui command parsing and usage messages to the new source contract.
5. Rewrite CLI tests to validate source-loader, scripts-dir-only flows, removed list command, and non-compat old state refs.
6. Run full test gate and move this plan to completed in the delivery commit.

## Verification
- Unit tests:
  - `test/cli-agent.test.ts`
  - `test/cli-tui-core.test.ts`
- Integration tests:
  - `npm run player:dev -- agent start --scripts-dir examples/scripts/09-random --state-out /tmp/a.bin`
  - `npm run player:dev -- agent choose --state-in /tmp/a.bin --choice 0 --state-out /tmp/b.bin`
  - `npm run player:dev -- agent list` should fail with usage error.
- Full gate:
  - `npm test`

## Risks and Mitigations
- Risk:
  - Existing CLI invocations with `--example` fail after change.
  - Mitigation:
    - Update docs and error text to point to `--scripts-dir`.
- Risk:
  - Existing state files with legacy example ids cannot resume.
  - Mitigation:
    - Fail fast with explicit `CLI_STATE_INVALID` message.

## Rollout
- Migration notes:
  - Replace `--example <id>` with `--scripts-dir examples/scripts/<id>`.
  - Regenerate state via `agent start` before `agent choose`.
- Compatibility notes:
  - Deliberate breaking change during active development.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
