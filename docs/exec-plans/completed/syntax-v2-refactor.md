# ScriptLang Syntax V2 Refactor

## Objective
- Move language surface from `script + vars + step` to direct script-body execution with script-name identity and block-scoped typed `<var>` declarations.

## Scope
- In scope:
  - Compiler support for script `name/args` + executable `<var>`.
  - Runtime support for block-scoped var declarations and script-param contracts.
  - API mapping by script name instead of file key.
  - Docs, examples, and tests migration.
- Out of scope:
  - Alternative expression syntaxes (CDATA / element-based expressions).
  - Legacy compatibility for `<vars>` and `<step>`.

## Interfaces / Contracts Affected
- Public API changes:
  - `entryScript` now targets script `name`.
  - `compileScriptsFromXmlMap` returns map keyed by script `name`.
- Snapshot/schema changes:
  - runtime frame payload includes optional `varTypes` for accurate restore checks.
- XML syntax/semantic changes:
  - remove `<vars>` and `<step>`.
  - add script-level `args` declaration.
  - `<var>` becomes executable block-scoped declaration.

## Implementation Steps
1. Update product specs (`index`, `syntax-manual`) for V2 grammar.
2. Extend core types for `scriptName`, `params`, and `VarNode`.
3. Refactor compiler root/arg parsing and executable node compilation.
4. Update runtime call/arg validation and executable var declaration behavior.
5. Switch API script map registration to script name.
6. Migrate examples and CLI scenario references to script names.
7. Rewrite tests to V2 syntax + new runtime contracts.
8. Run validate/typecheck/tests/coverage strict gate.

## Verification
- Unit tests:
  - parser/compiler node validation and arg parsing.
  - runtime var scope and call ref constraints.
- Integration tests:
  - API create/resume flow by script name.
  - CLI agent and TUI scenarios under new syntax.
- Manual checks:
  - `npm run player:agent -- start --example 06-snapshot-flow --state-out /tmp/sl.bin`

## Risks and Mitigations
- Risk: breaking all existing XML fixtures.
  - Mitigation: systematic fixture migration + explicit errors for removed nodes.
- Risk: var scope/type mismatch after snapshot restore.
  - Mitigation: persist `varTypes` in runtime frame snapshots.
- Risk: ambiguous call/ref semantics.
  - Mitigation: strict compile/runtime contract checks and dedicated negative tests.

## Rollout
- Migration notes:
  - Update every script root to `name="id"` and remove `vars/step` wrappers.
  - Convert root variable declarations to `script args` or executable `<var>` nodes.
- Compatibility notes:
  - old syntax is rejected with explicit compile errors.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed
