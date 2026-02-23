# Harness Doc Sync And Drift Guard

## Objective
- Align harness docs with current repository workflow and remove ambiguous wording that can cause process drift.

## Scope
- In scope:
  - sync `README.md`, `AGENTS.md`, `ARCHITECTURE.md`, and workflow docs with current gate/process behavior.
  - clarify development-phase compatibility stance.
  - define explicit completion criteria for moving plans from `active` to `completed`.
- Out of scope:
  - runtime/compiler behavior changes.
  - new CLI or language features.

## Interfaces / Contracts Affected
- Public API changes:
  - none.
- Snapshot/schema changes:
  - none.
- XML syntax/semantic changes:
  - none.

## Implementation Steps
1. Audit harness docs for wording conflicts and drift risks.
2. Update key docs with aligned gate/compatibility/plan-move wording.
3. Run docs validation and full test gate to verify no process regressions.
4. Move this plan to completed in the same delivery commit context.

## Verification
- Unit tests:
  - `npm test` passes.
- Integration tests:
  - n/a.
- Manual checks:
  - cross-check key statements in `README.md`, `AGENTS.md`, `ARCHITECTURE.md`, `docs/TEST_WORKFLOW.md`, `docs/exec-plans/README.md`.

## Risks and Mitigations
- Risk:
  - process wording diverges again across docs.
  - Mitigation:
    - keep gate/process wording centralized and updated together in doc-sync step.

## Rollout
- Migration notes:
  - none.
- Compatibility notes:
  - development phase defaults to no backward-compat requirement unless explicitly requested by task.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
