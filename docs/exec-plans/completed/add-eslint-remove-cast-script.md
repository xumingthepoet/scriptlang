# Add ESLint And Remove Cast Script

## Objective
- Introduce a repository linter gate and remove the custom `validate:casts` script.

## Scope
- In scope:
  - add ESLint setup for TypeScript source.
  - add `npm run lint`.
  - replace `validate:casts` in `quality:gate` with `lint`.
  - remove `scripts/validate-no-unsafe-casts.mjs`.
  - sync workflow docs (AGENTS/README/TEST_WORKFLOW).
- Out of scope:
  - broad style formatting policy.
  - linting test files.

## Interfaces / Contracts Affected
- Quality gate contract:
  - `npm test` pretest uses `lint` instead of `validate:casts`.

## Implementation Steps
1. Add ESLint dependencies and config.
2. Add `lint` npm script.
3. Remove `validate:casts` script and obsolete script file.
4. Update docs for the new gate.
5. Run full quality gate and tests.

## Verification
- `npm run lint` passes.
- `npm test` passes with strict gate.

## Risks and Mitigations
- Risk: lint configuration too strict for existing code.
  - Mitigation: start with minimal rules for production source and expand later.

## Done Criteria
- [x] Specs/docs updated
- [x] Tests/quality gate passing
- [x] Plan moved to completed (in same delivery commit)
