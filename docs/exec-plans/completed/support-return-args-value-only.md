# Support Return Transfer Args (Value-Only)

## Objective
- Add positional args support to `<return script="..."/>` so transfer returns can pass values into target script parameters.

## Scope
- In scope:
  - parser support for `<return script="..." args="..."/>`.
  - runtime binding of return transfer args by target script param order.
  - explicit runtime rejection of `ref:` in return args (V1).
  - tests and docs updates.
- Out of scope:
  - `ref` binding semantics across transfer return chains.
  - changes to snapshot schema.

## Interfaces / Contracts Affected
- Public API changes:
  - none.
- Snapshot/schema changes:
  - none.
- XML syntax/semantic changes:
  - `<return script="..." args="..."/>` is supported.
  - return args are positional and value-only in V1.

## Implementation Steps
1. Update product spec docs for return-transfer args and value-only rule.
2. Extend `ReturnNode` and compiler return parsing to carry `args`.
3. Update runtime return-transfer execution to evaluate and bind args to target params.
4. Add runtime guard for `ref:` return args (`ENGINE_RETURN_REF_UNSUPPORTED`).
5. Add/adjust tests and run full `npm test`.
6. Move plan to completed in delivery commit.

## Verification
- Unit tests:
  - `test/runtime.test.ts`, `test/coverage-branches.test.ts`, `test/compiler.test.ts`.
- Integration tests:
  - full `npm test`.
- Manual checks:
  - run scenario with `<return script="..." args="..."/>` and verify target receives values.

## Risks and Mitigations
- Risk:
  - interaction with existing continuation ref write-back could become inconsistent.
  - Mitigation:
    - transfer path flushes inherited ref write-back before switching root and clears inherited ref bindings for the transferred script.

## Rollout
- Migration notes:
  - none required; old `<return script="..."/>` remains valid.
- Compatibility notes:
  - return `ref:` args intentionally unsupported in V1.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
