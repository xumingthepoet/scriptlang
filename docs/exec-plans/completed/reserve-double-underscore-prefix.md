# Reserve `__` Prefix For Internal Macro Names

## Objective
- Prevent collisions between compiler-generated macro variables and user-authored names by reserving the `__` prefix for ScriptLang internals.

## Scope
- In scope:
  - Compile-time validation for user-authored names that start with `__`.
  - Clear compile errors and spec/manual documentation updates.
  - Unit test coverage for accepted/rejected naming cases.
- Out of scope:
  - Runtime behavior changes.
  - Snapshot/schema changes.
  - Macro expansion strategy changes beyond prefix reservation.

## Interfaces / Contracts Affected
- Public API changes:
  - None.
- Snapshot/schema changes:
  - None.
- XML syntax/semantic changes:
  - User-defined names with `__` prefix are invalid across named entities (script name, args, var/types-collection/type/field names, JSON global symbol names).

## Implementation Steps
1. Spec-first updates in product spec docs to define the reserved prefix rule and the error contract.
2. Add compiler-level centralized reserved-prefix validation and apply it at all relevant name parse points.
3. Extend compiler tests to cover each affected name category and verify the new error code.
4. Run full gate and then move this plan to completed in the same delivery commit.

## Verification
- Unit tests:
  - `test/compiler.test.ts` new coverage for `NAME_RESERVED_PREFIX`.
- Integration tests:
  - Covered indirectly via `npm test` full suite.
- Manual checks:
  - Confirm existing loop macro temp variable prefix remains internal-only and conflict-free.

## Risks and Mitigations
- Risk:
  - Overly broad validation could reject previously accepted scripts.
  - Mitigation:
    - Restrict behavior to explicit `__`-prefix cases only.
- Risk:
  - Inconsistent enforcement across naming entry points.
  - Mitigation:
    - Use a shared validator helper and add per-entity tests.

## Rollout
- Migration notes:
  - Rename any user-authored identifiers starting with `__`.
- Compatibility notes:
  - This is a compile-time breaking change by design during active development.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
