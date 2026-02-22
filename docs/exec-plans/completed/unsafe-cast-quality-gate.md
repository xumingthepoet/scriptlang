# Unsafe Cast Quality Gate

## Objective
- Add a strict harness gate that rejects unsafe cast patterns in production source code.

## Scope
- In scope:
  - add a script that scans `src/**/*.ts(x)` for forbidden patterns:
    - `as any`
    - `as unknown as`
  - wire the check into `quality:gate`.
  - update workflow docs to include the new gate.
- Out of scope:
  - exception/allowlist mechanisms.
  - lint framework adoption.

## Interfaces / Contracts Affected
- Quality gate contract:
  - `npm test` pretest now includes `validate:casts`.

## Implementation Steps
1. Add `scripts/validate-no-unsafe-casts.mjs`.
2. Add `npm run validate:casts` and include it in `quality:gate`.
3. Update AGENTS/README/test workflow docs.
4. Run validation and test gate.

## Verification
- `npm run validate:casts` succeeds on current source tree.
- `npm test` still passes.

## Risks and Mitigations
- Risk: false positives from test-only cast usage.
  - Mitigation: scan `src/` only.
- Risk: policy drift between docs and scripts.
  - Mitigation: update all gate docs in the same change.

## Done Criteria
- [x] Specs/docs updated
- [x] Tests/quality gate passing
- [x] Plan moved to completed (in same delivery commit)
