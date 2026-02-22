# Remove Option Once

## Objective
- Remove `once` support from `<option>` so options are gated only by `when` and current scene state.

## Scope
- In scope:
  - Compiler no longer reads `option.once`.
  - Runtime no longer tracks chosen-option IDs for once filtering.
  - Snapshot payload no longer carries once-tracking state.
  - Docs/examples/tests updated to remove `once` usage.
- Out of scope:
  - Any new option attributes.
  - Broader choice-system redesign.

## Interfaces / Contracts Affected
- XML syntax/semantic changes:
  - `<option once="...">` is removed.
  - Choosing an option never permanently hides sibling options by built-in `once` behavior.
- Snapshot/schema changes:
  - remove `selectedChoices` from `SnapshotV1`.

## Implementation Steps
1. Update product specs and syntax manual to remove `once`.
2. Update core types (`ChoiceOption`, `SnapshotV1`) to drop once fields.
3. Update compiler option parsing to stop reading `once`.
4. Update runtime choice filtering and snapshot/resume logic.
5. Migrate examples and tests.
6. Run docs/typecheck/tests/coverage strict gate.

## Verification
- Unit tests:
  - choice options remain available after resume/choose cycles unless gated by `when`.
  - snapshot/resume works without once state.
- Regression:
  - `npm run validate:docs`
  - `npm run typecheck`
  - `npm test`

## Risks and Mitigations
- Risk: snapshot compatibility for old stored files.
  - Mitigation: this is a breaking behavior update; keep schema version but accept missing field path only.
- Risk: hidden references to `once` remain in examples/docs.
  - Mitigation: `rg once` sweep before final checks.

## Done Criteria
- [x] Specs updated
- [x] Tests added/updated and passing
- [x] Plan moved to completed
