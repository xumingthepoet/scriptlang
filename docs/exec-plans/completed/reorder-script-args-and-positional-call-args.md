# Reorder Script Args And Positional Call Args

## Objective
- Change script argument declaration syntax to `[ref:]type:name` and call argument syntax to positional `[ref:]value` without repeating parameter names.

## Scope
- In scope:
  - compiler parsing for new script/call arg grammar.
  - runtime call binding by position instead of arg name.
  - example and test migration to new syntax.
  - product spec updates.
- Out of scope:
  - runtime behavior changes unrelated to arg binding.
  - new language nodes or built-ins.

## Interfaces / Contracts Affected
- Public API changes:
  - none.
- Snapshot/schema changes:
  - none.
- XML syntax/semantic changes:
  - script arg declaration changes from `name:type[:ref]` to `[ref:]type:name`.
  - call arg syntax changes from `name:value` to positional `[ref:]value`.
  - call binding changes from by-name to by-position.

## Implementation Steps
1. Update product specs (`index.md`, `syntax-manual.md`) to define new grammar and positional binding.
2. Update core types and compiler arg parsers to produce positional call arguments and new script param declarations.
3. Update runtime `executeCall` to map call args to script params by declaration order.
4. Migrate examples and tests to the new syntax and expected error paths.
5. Run full `npm test`, then move plan to completed in delivery commit.

## Verification
- Unit tests:
  - `test/compiler.test.ts`, `test/runtime.test.ts`, `test/coverage-branches.test.ts`.
- Integration tests:
  - full `npm test`.
- Manual checks:
  - run agent/TUI with example scenarios that use `<call ... args="...">`.

## Risks and Mitigations
- Risk:
  - broad fixture breakage due to syntax churn.
  - Mitigation:
    - migrate all `args` usages in tests/examples together and keep coverage gate green.

## Rollout
- Migration notes:
  - all `<script args>` and `<call args>` must be rewritten to new grammar.
- Compatibility notes:
  - no backward compatibility for old arg grammar during active development.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
