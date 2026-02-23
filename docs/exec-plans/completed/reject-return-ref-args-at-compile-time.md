# Reject Return Ref Args At Compile Time

## Objective
- Enforce `<return script="..." args="...">` as value-only at compile time by rejecting any `ref:` segment.

## Scope
- In scope:
  - compiler validation for return args.
  - doc updates for compile-time error semantics.
  - test updates to assert compile error instead of runtime error.
- Out of scope:
  - enabling `return` ref semantics.
  - snapshot/runtime protocol changes.

## Interfaces / Contracts Affected
- Public API changes:
  - none.
- Snapshot/schema changes:
  - none.
- XML syntax/semantic changes:
  - `ref:` inside return args is a compile error.

## Implementation Steps
1. Update product specs to declare compile-time rejection.
2. Add compiler guard in return node compilation.
3. Remove/adjust runtime-only guard and tests.
4. Run full `npm test`.
5. Move plan to completed in delivery commit.

## Verification
- Unit tests:
  - compiler and runtime tests around return args.
- Integration tests:
  - full `npm test`.

## Risks and Mitigations
- Risk:
  - existing scripts relying on runtime failure behavior may change error timing.
  - Mitigation:
    - preserve clear error code and message at compile stage.

## Rollout
- Migration notes:
  - replace `ref:` return args with value expressions or redesign flow with `call`.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
