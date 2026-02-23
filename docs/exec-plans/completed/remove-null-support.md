# Remove Null Type And Null Values

## Objective
- Remove language-level `null` support from ScriptLang types.
- Enforce that declared ScriptLang variables never hold `null` values at runtime.

## Scope
- In scope:
  - compiler no longer accepts `null` in `<var type="...">` and `<script args="...">`.
  - runtime type compatibility rejects `null` for all declared types, including nested array/map values.
  - snapshot restore rejects legacy runtime frame type metadata containing `null` primitive kinds.
  - docs and tests updated to the new contract.
- Out of scope:
  - introducing compatibility flags or migration shims.
  - changing non-type uses of `null` in implementation internals (for optional fields, sentinels, etc.).

## Interfaces / Contracts Affected
- XML type syntax:
  - remove `null` type.
  - keep `number|string|boolean`, `T[]`, and `Map<string, T>`.
- Runtime state rules:
  - declared script variables cannot contain `null` values.
- Snapshot restore:
  - legacy `varTypes` with primitive `null` are rejected with a stable runtime error.

## Implementation Steps
1. Update product specs (`index`, `syntax-manual`) for removed `null` support.
2. Remove `null` from core type unions and compiler primitive parser list.
3. Refactor runtime defaults/type checks to eliminate `null` branch and add defensive primitive-kind validation.
4. Add snapshot restore validation for unsupported primitive type names in runtime frame `varTypes`.
5. Update unit tests (compiler/runtime/coverage branches) for new compile/runtime errors.
6. Run full gate (`validate:docs`, `lint`, `typecheck`, `coverage:strict`, `npm test`) at 100% coverage.

## Verification
- `compileScript` rejects `type="null"` and `args="x:null"` with `TYPE_PARSE_ERROR`.
- Runtime throws `ENGINE_TYPE_MISMATCH` when declared values become `null` (including nested collection values).
- Resume rejects snapshots with legacy `null` type metadata.

## Risks and Mitigations
- Risk: legacy snapshots fail to resume after upgrade.
  - Mitigation: return explicit error code for unsupported snapshot type metadata.
- Risk: coverage drops due to removed branches.
  - Mitigation: rebalance coverage tests to hit all remaining/added branches.

## Done Criteria
- [x] Specs/docs updated
- [x] Tests added/updated and passing
- [x] Full quality gate passing at 100% coverage
- [x] Plan moved to completed in delivery commit
