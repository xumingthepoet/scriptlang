# Test and Coverage Workflow

This repository uses Vitest and a strict pre-test quality gate.

## Mandatory Flow

1. Sync docs before gates:
   - `/README.md`
   - `/ARCHITECTURE.md`
   - `/docs/` (all impacted specs/plans/workflow docs)
   - audit `/docs/exec-plans/active/`; move only truly completed plans to `/docs/exec-plans/completed/`
   - if gate has not been executed successfully, do not mark plan completion or move plans
2. Run `npm test`.
3. `pretest` runs automatically before tests:
   - `npm run validate:docs`
   - `npm run lint`
   - `npm run typecheck`
   - `npm run coverage:strict` (`vitest run --coverage`)
4. Only if all gates pass does `test` continue.
5. If `npm test` passes, create a `git commit` before ending the delivery conversation (no extra approval ping required).

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
