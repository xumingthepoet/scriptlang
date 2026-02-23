# Test and Coverage Workflow

This repository uses Vitest and a strict quality gate wired into `npm test`.
This document owns test and coverage mechanics only.
For end-to-end delivery sequence (doc sync, plan movement, commit policy), use `/docs/HARNESS.md`.

## Mandatory Flow

1. Run `npm test`.
2. `npm test` runs gate commands before unit tests:
   - `npm run validate:docs`
   - `npm run lint`
   - `npm run typecheck`
   - `npm run coverage:strict` (`vitest run --coverage`)
3. Only if all gates pass does `test` continue.

## Coverage Rule

- Coverage thresholds are fixed at 100% for:
  - lines
  - branches
  - functions
  - statements
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
