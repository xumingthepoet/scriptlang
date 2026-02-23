# Harness Responsibility Split

## Objective
- Remove workflow-rule duplication across harness docs and establish a single source of truth for agent delivery process.

## Scope
- In scope:
  - add one canonical harness workflow document.
  - re-scope existing docs so each owns a distinct responsibility.
  - update docs validation to require the canonical harness workflow doc.
- Out of scope:
  - runtime/compiler/CLI behavior changes.
  - test framework behavior changes.

## Interfaces / Contracts Affected
- Public API changes:
  - none.
- Snapshot/schema changes:
  - none.
- XML syntax/semantic changes:
  - none.

## Implementation Steps
1. Define responsibility boundaries and workflow ownership in a canonical harness doc.
2. Trim duplicate workflow instructions from AGENTS/README/TEST_WORKFLOW/exec-plan docs and replace with links.
3. Update docs index and validation checks to enforce the new canonical doc.
4. Run full quality gate and move this plan to completed in the same delivery commit.

## Verification
- Unit tests:
  - `npm test` passes.
- Integration tests:
  - n/a.
- Manual checks:
  - verify each harness rule appears in exactly one authority document.

## Risks and Mitigations
- Risk:
  - references break if canonical doc path changes.
  - Mitigation:
    - enforce path in `scripts/validate-docs.mjs`.

## Rollout
- Migration notes:
  - existing readers should use `/docs/HARNESS.md` as process authority.
- Compatibility notes:
  - no runtime compatibility impact.

## Done Criteria
- [x] Canonical harness workflow doc added
- [x] Duplicate process rules removed from non-authority docs
- [x] Docs validation updated for new authority doc
- [x] Full quality gate passing
- [x] Plan moved to completed in same delivery commit
