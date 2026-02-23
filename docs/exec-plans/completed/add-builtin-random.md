# Add Builtin Random (Deterministic, Non-Host)

## Objective
- Add a language-level `random()` builtin that does not rely on `hostFunctions`, supports deterministic seeded behavior, and remains replayable across snapshot/resume.

## Scope
- In scope:
  - Runtime builtin `random()` with strict zero-arg signature.
  - Seeded PRNG state in engine and snapshot.
  - Pending choice snapshot stabilization to avoid re-evaluating random-dependent choice rendering on resume.
  - API passthrough of seed for create flow.
  - CLI snapshot validation update.
  - Spec/doc updates and tests.
- Out of scope:
  - Overriding or disabling `Math.random`.
  - Additional random overloads (`random(max)`, `random(min,max)`).
  - Exposing PRNG algorithm selection.

## Interfaces / Contracts Affected
- Public API changes:
  - `ScriptLangEngineOptions.randomSeed?: number`
  - `CreateEngineFromXmlOptions.randomSeed?: number`
- Snapshot/schema changes:
  - `SnapshotV1.rngState: number`
  - `SnapshotV1.pendingChoiceItems: ChoiceItem[]`
  - Missing these fields is a hard failure.
- XML syntax/semantic changes:
  - No XML grammar changes.
  - Runtime expression environment adds builtin `random()`.

## Implementation Steps
1. Update product/design/architecture docs to define deterministic builtin random behavior and snapshot contract.
2. Extend runtime engine with seeded PRNG state, builtin injection, reserved-name checks, and snapshot/resume handling.
3. Extend API options to pass `randomSeed` into engine creation.
4. Tighten CLI state snapshot validation for new required snapshot fields.
5. Add/adjust tests for runtime/API/CLI/coverage branches to maintain strict 100% gate.

## Verification
- Unit tests:
  - `npm test`
- Integration tests:
  - Included in existing Vitest suites via API and CLI core tests.
- Manual checks:
  - Verify deterministic output consistency with same seed.

## Risks and Mitigations
- Risk: Resume drift when random is used in choice filters/rendering.
  - Mitigation: Persist rendered pending choices and reuse on resume.
- Risk: Host function collisions with builtin name.
  - Mitigation: Hard fail constructor on `hostFunctions.random`.
- Risk: Snapshot shape tightening breaks old serialized states.
  - Mitigation: explicit runtime/CLI validation errors with dedicated codes.

## Rollout
- Migration notes:
  - Old snapshots without `rngState` and `pendingChoiceItems` are rejected.
- Compatibility notes:
  - Active development policy applies; no backward compatibility retained for prior snapshot payload shape.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
