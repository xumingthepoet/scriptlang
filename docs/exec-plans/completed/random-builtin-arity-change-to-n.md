# Random Builtin Arity Change To `random(n)`

## Objective
- Change ScriptLang builtin random from `random()` to `random(n)`, where `n` is an integer and return value is in `[0, n-1]`.

## Scope
- In scope:
  - Runtime builtin signature change to exactly one argument.
  - Integer/range validation for `n`.
  - Product spec updates for syntax and runtime behavior.
  - Tests updated for deterministic seeded behavior and error branches.
- Out of scope:
  - New overloads like `random(min,max)`.
  - PRNG algorithm replacement.
  - Snapshot schema version bump.

## Interfaces / Contracts Affected
- Public API changes:
  - No API surface change (`randomSeed` remains the same), but expression-level builtin contract changes.
- Snapshot/schema changes:
  - No schema field changes; existing RNG state persistence remains.
- XML syntax/semantic changes:
  - No XML grammar change; expression behavior changes from `random()` to `random(n)`.

## Implementation Steps
1. Update product specs to define `random(n)` contract and validation rules.
2. Update runtime builtin implementation and errors in `src/runtime/engine.ts`.
3. Update runtime/API tests and random-related choice prompt tests to new call form.
4. Run full `npm test` gate.
5. Move plan to completed in the same delivery commit.

## Verification
- Unit tests:
  - deterministic sequence with same seed under `random(n)`.
  - arity/type/range errors for random arg.
- Integration tests:
  - `npm test` full gate.
- Manual checks:
  - none.

## Risks and Mitigations
- Risk:
  - modulo mapping can bias values for some `n`.
  - Mitigation:
    - use rejection sampling for bounded mapping from `uint32`.

## Rollout
- Migration notes:
  - scripts using `random()` must update to `random(n)`.
- Compatibility notes:
  - active development policy: no backward compatibility required for legacy call form.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
