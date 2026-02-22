# Add Battle Duel Example

## Objective
- Add a new complex playable example that demonstrates a two-character battle with player choices, looped combat in a called script, and winner-specific ending output.

## Scope
- In scope:
  - new scenario `07-battle-duel` under `examples/scripts/`.
  - multi-file composition with `main` calling battle loop script.
  - syntax coverage across existing V2 surface (`var`, `text`, `code`, `if/else`, `while`, `choice/option`, `call`, `return`, `return script`, script args with ref/value).
  - scenario registry and tests/docs updates.
- Out of scope:
  - runtime/grammar behavior changes.
  - new built-in random functions.

## Interfaces / Contracts Affected
- CLI scenario list gains one new scenario id:
  - `07-battle-duel`
- No runtime API or syntax contract changes.

## Implementation Steps
1. Update player CLI spec to include bundled scenario entry.
2. Add `examples/scripts/07-battle-duel/` XML files:
   - `main.script.xml`
   - `battle-loop.script.xml`
   - `victory.script.xml`
   - `defeat.script.xml`
3. Register scenario in `src/cli/core/scenario-registry.ts`.
4. Update CLI tests that assert scenario count/list.
5. Run docs/typecheck/tests/coverage and full `npm test`.

## Verification
- `agent list` includes `07-battle-duel`.
- `agent start --example 07-battle-duel` reaches combat choice boundary.
- Main script calls battle-loop script and branches to winner ending output.

## Risks and Mitigations
- Risk: deterministic enemy behavior feels too static.
  - Mitigation: vary enemy move by round/state-derived expression to emulate unpredictability.
- Risk: example accidentally relies on unsupported syntax.
  - Mitigation: keep only existing V2 nodes/attributes and run full suite.

## Done Criteria
- [x] Specs updated
- [x] Tests added/updated and passing
- [ ] Plan moved to completed
