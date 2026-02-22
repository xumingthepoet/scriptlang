# Remove Record Type Support

## Objective
- Remove language-level `Record<string, T>` type support from ScriptLang.

## Scope
- In scope:
  - compiler type parser no longer accepts `Record<string, T>`.
  - runtime type system no longer has `record` branch.
  - docs and tests updated to reflect supported types.
- Out of scope:
  - changes to TS utility type usage (`Record<...>` in implementation code).
  - new replacement type syntax.

## Interfaces / Contracts Affected
- XML type syntax:
  - remove `Record<string, T>` support.
  - keep `number|string|boolean|null`, `T[]`, `Map<string, T>`.
- Runtime type checks/defaults:
  - no `record` kind handling.

## Implementation Steps
1. Update product specs for type syntax.
2. Remove `record` from core type unions and parser/runtime branches.
3. Migrate tests that rely on `Record` language type.
4. Run full gate (`validate:docs`, `validate:casts`, `typecheck`, `coverage:strict`, `npm test`).

## Verification
- `compileScript` rejects `Record<string, T>` with `TYPE_PARSE_ERROR`.
- Full quality gate remains green at 100% coverage.

## Risks and Mitigations
- Risk: tests using `Record` as language syntax fail broadly.
  - Mitigation: update fixtures to `Map` or explicit object injection in branch tests.

## Done Criteria
- [x] Specs/docs updated
- [x] Tests/quality gate passing
- [x] Plan moved to completed (in same delivery commit)
