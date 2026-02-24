# Require Choice Text Attribute

## Objective
- Enforce authoring rule that every `<choice>` must include a non-empty `text` prompt.

## Scope
- In scope:
  - Product spec updates for `<choice text>` requirement.
  - Compiler validation change for missing/empty choice prompt text.
  - Update examples and tests that currently compile `<choice>` without `text`.
- Out of scope:
  - Changing `<option text>` semantics.
  - Snapshot schema changes.
  - CLI UI redesign.

## Interfaces / Contracts Affected
- Public API changes:
  - None.
- Snapshot/schema changes:
  - None.
- XML syntax/semantic changes:
  - `<choice text="...">` remains the syntax, but `text` becomes required and non-empty.

## Implementation Steps
1. Update product specs (`index.md`, `syntax-manual.md`, `player-cli.md`) to mark choice prompt as required.
2. Update compiler parsing in `src/compiler/compiler.ts` to reject missing `choice text` (`XML_MISSING_ATTR`) and whitespace-only prompt (`XML_EMPTY_ATTR`).
3. Update examples/tests to provide explicit `<choice text="...">` where missing, and add compiler coverage for missing attribute.
4. Run targeted tests for compiler/runtime/API/CLI suites impacted by choice parsing.

## Verification
- Unit tests:
  - `test/compiler.test.ts`
  - `test/runtime.test.ts`
  - `test/api.test.ts`
  - `test/coverage-branches.test.ts`
  - `test/cli-agent.test.ts`
  - `test/cli-tui-core.test.ts`
- Integration tests:
  - N/A for this change.
- Manual checks:
  - Run agent mode against `03-choice-once` and verify prompt line is emitted.

## Risks and Mitigations
- Risk:
  - Existing scripts without choice prompt fail to compile after change.
  - Mitigation:
    - Update bundled examples/tests in same delivery.

## Rollout
- Migration notes:
  - Add `text` to all `<choice>` nodes in authored scripts.
- Compatibility notes:
  - This is an intentional behavior break during active development.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
