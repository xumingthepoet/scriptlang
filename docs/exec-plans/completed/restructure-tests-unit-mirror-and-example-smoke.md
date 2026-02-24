# Restructure Tests To Src-Mirrored Unit Layout + Example Smoke

## Objective
- Reorganize tests into a defensive, file-mirrored unit structure aligned one-to-one with non-`.d.ts` source files under `src/`.
- Redefine smoke tests as integration coverage focused on runnable examples under `examples/scripts/`.

## Scope
- In scope:
  - Introduce `test/unit/**` mirrored topology and migrate existing tests into mapped files.
  - Introduce `test/smoke/**` example-driven integration tests.
  - Remove old root-level aggregated test files after migration.
  - Fix `examples/scripts/04-call-ref-return` and `examples/scripts/05-return-transfer` include headers so they are runnable in smoke flows.
  - Update docs to define test topology and responsibilities.
- Out of scope:
  - Runtime/compiler API behavior changes.
  - Coverage threshold changes.
  - Adding automated structure-lint scripts.

## Interfaces / Contracts Affected
- Public API changes:
  - None.
- Snapshot/schema changes:
  - None.
- XML syntax/semantic changes:
  - None.
- Engineering/test contract changes:
  - Unit tests must mirror source layout one-to-one.
  - Smoke tests primarily validate integrated example execution.

## Implementation Steps
1. Update docs (`docs/TEST_WORKFLOW.md`, `README.md`) to define unit/smoke topology.
2. Create `test/unit/**` mirrored directories and mapped test files for each non-`.d.ts` source file.
3. Migrate existing tests from old aggregated files into mapped files and align test ordering to source declaration order.
4. Split former cross-cutting defensive branch tests into corresponding unit files (primarily `runtime/engine`, `compiler/compiler`, `compiler/xml`, `api`).
5. Add `test/smoke/examples-agent.test.ts` to run examples `01..13` via `agent start/choose` until `END`.
6. Add include headers in example `04` and `05` mains to make called/returned scripts reachable.
7. Remove obsolete root-level test files and de-duplicate overlapping assertions.
8. Run full gate (`npm test`) and adjust migrated tests until coverage remains 100%.

## Verification
- Unit tests:
  - All mapped unit test files exist and pass.
  - Defensive/error-path assertions preserved after migration.
- Integration tests:
  - Example smoke test passes across all `examples/scripts/01..13`.
- Manual checks:
  - Validate test file tree mirrors source tree for non-`.d.ts` files.

## Risks and Mitigations
- Risk:
  - Coverage drops due to migration gaps.
  - Mitigation:
    - Move branch-defensive cases with source ownership and verify using strict coverage gate.
- Risk:
  - Path/import errors after directory move.
  - Mitigation:
    - Batch-update imports per folder depth and run quick targeted `vitest` before full gate.
- Risk:
  - Example smoke flakiness.
  - Mitigation:
    - Use deterministic `agent` protocol with max-step guard.

## Rollout
- Migration notes:
  - Old root-level test files are intentionally removed and replaced by `test/unit/**` and `test/smoke/**`.
- Compatibility notes:
  - No product/runtime compatibility impact; test organization only.

## Done Criteria
- [x] Docs updated with topology responsibilities
- [x] Unit tests mirror non-`.d.ts` source files one-to-one
- [x] Example smoke integration tests added and passing
- [x] Full gate passes with 100% coverage
- [x] Plan moved to completed in delivery commit
