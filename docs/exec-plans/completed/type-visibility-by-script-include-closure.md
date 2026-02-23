# Enforce Type Visibility By Script Include Closure

## Objective
- Make compilation fail when a script references a custom type that is not reachable from that script file's own include closure.

## Scope
- In scope:
  - compiler visibility checks for custom type usage in `<script args>` and `<var type>`.
  - tests that prove missing per-script include causes compile error.
  - docs alignment for include/type visibility semantics.
- Out of scope:
  - runtime behavior changes.
  - syntax surface changes.
  - namespace/type alias features.

## Interfaces / Contracts Affected
- Public API changes:
  - none.
- Snapshot/schema changes:
  - none.
- XML syntax/semantic changes:
  - semantic tightening: custom type resolution is include-scoped per script, not globally visible from `main` closure.

## Implementation Steps
1. Update product specs to define script-scoped custom type visibility.
2. Extend compiler project compile path to compute per-script reachable type names from include graph.
3. Enforce visibility during script compilation and return compile error when type is out of scope.
4. Add regression tests for failing and passing include-visibility cases.
5. Run full `npm test` gate and move plan to completed with delivery commit.

## Verification
- Unit tests:
  - `test/compiler.test.ts` include-visibility regression cases.
- Integration tests:
  - existing API/CLI/runtime suites via full gate.
- Manual checks:
  - example 07 continues compiling and running.

## Risks and Mitigations
- Risk:
  - tightening visibility may break existing fixtures relying on global leakage.
  - Mitigation:
    - update fixtures to include required `.types.xml` from each script that uses custom types.

## Rollout
- Migration notes:
  - any script that uses custom type names must include (directly or transitively) the relevant `.types.xml`.
- Compatibility notes:
  - intentionally breaking behavior during active development phase.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
