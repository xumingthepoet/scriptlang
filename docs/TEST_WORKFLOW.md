# Test and Coverage Workflow

This repository uses a strict pre-test quality gate.

## Mandatory Flow

1. Run `npm test`.
2. `pretest` runs automatically before tests:
   - `npm run validate:docs`
   - `npm run typecheck`
   - `npm run coverage:strict`
3. Only if all gates pass does `test` continue.

## Coverage Rule

- Coverage thresholds are fixed at 100% for:
  - lines
  - branches
  - functions
- Scope: `src/**/*.ts`

If coverage is below 100%:

1. Read the uncovered file/line report from the coverage output.
2. Add or update tests to cover the missing paths.
3. Re-run `npm test`.
4. Repeat until thresholds are fully satisfied.

## Failure Handling

- If tests fail, fix code or tests, then rerun `npm test`.
- If coverage fails, do not bypass or lower thresholds.
- Every change to fix failing tests/coverage must be committed.

