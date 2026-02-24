# Add Choice Traversal Tool For Comprehensive Smoke Validation

## Objective
- Add a standalone traversal tool that explores all currently visible choice branches in a script scenario and verifies each explored path reaches `END` without runtime errors or timeout/step-limit failures.
- Use this tool as smoke integration coverage for examples.

## Scope
- In scope:
  - Add traversal program under `scripts/` with CLI usage.
  - Traverse by branching on every visible choice option from snapshot boundaries.
  - Enforce deterministic behavior with fixed default random seed behavior (engine default).
  - Enforce per-path choice-step limit and per-scenario runtime limit; exceed => failure.
  - Update smoke test to use this traversal tool for `examples/scripts/01..13`.
  - Add standalone documentation with usage, constraints, and current limitations.
  - Reference the new document from `docs/TEST_WORKFLOW.md`.
- Out of scope:
  - Language/runtime semantic changes.
  - Snapshot format changes.
  - State de-duplication in traversal (explicitly deferred for now).

## Interfaces / Contracts Affected
- Public API changes:
  - None.
- Snapshot/schema changes:
  - None.
- Tooling/test contract changes:
  - New traversal command available for local and CI smoke verification.
  - Smoke expectation shifts from single-path happy flow to multi-branch traversal.

## Implementation Steps
1. Add traversal script (`scripts/choice-traversal.ts`) with:
   - scenario loading,
   - branch DFS traversal from choice boundaries,
   - path trace reporting,
   - limits (`max-choice-steps`, `max-runtime-ms`).
2. Add npm script entry to run the tool via `tsx`.
3. Replace simple smoke case with traversal-based smoke assertion across examples.
4. Add standalone documentation describing:
   - usage examples,
   - options,
   - output interpretation,
   - caveats (no dedup yet).
5. Link new documentation from `docs/TEST_WORKFLOW.md`.
6. Run `npm test` and adjust for deterministic green gate.

## Verification
- Unit/integration tests:
  - Smoke test passes by invoking traversal tool and validating success exit code.
- Manual checks:
  - Tool runs on a single scenario and on full examples root.
  - Failure output includes scenario name + branch trace for reproduction.

## Risks and Mitigations
- Risk:
  - Branch explosion on scripts with high fan-out loops.
  - Mitigation:
    - enforce hard limits and fail-fast with explicit diagnostics.
- Risk:
  - False negatives from too-low limits.
  - Mitigation:
    - defaults documented and configurable by flags.
- Risk:
  - No dedup can revisit identical states repeatedly.
  - Mitigation:
    - explicitly documented as current limitation; keep limits strict.

## Rollout
- Migration notes:
  - Smoke test behavior changes from single default-path check to branch traversal validation.
- Compatibility notes:
  - No product compatibility impact; test/tooling only.

## Done Criteria
- [x] Traversal tool implemented with CLI options and clear failure reporting
- [x] Standalone tool doc added (usage + notes + current approach)
- [x] `docs/TEST_WORKFLOW.md` references new doc
- [x] Smoke test uses traversal tool for examples
- [x] Full gate passes
- [x] Plan moved to completed in same delivery commit
