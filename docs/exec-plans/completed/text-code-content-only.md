# Text And Code Content-Only Syntax

## Objective
- Make `<text>` and `<code>` content-only nodes.
- Remove `value` attribute support and enforce non-empty inline content.

## Scope
- In scope:
  - compiler rejects `value` attribute on `<text>` and `<code>`.
  - compiler rejects empty/whitespace-only inline content on `<text>` and `<code>`.
  - docs, README, examples, and tests migrated to inline content syntax.
- Out of scope:
  - changes to behavior of interpolation, VM execution, or other XML nodes.
  - compatibility layer, warnings, or migration flags.

## Interfaces / Contracts Affected
- `<text>`:
  - allowed: `<text>...</text>`
  - disallowed: `<text>` with `value` attribute
- `<code>`:
  - allowed: `<code>...</code>`
  - disallowed: `<code>` with `value` attribute
- New compiler errors:
  - `XML_ATTR_NOT_ALLOWED`
  - `XML_EMPTY_NODE_CONTENT`

## Implementation Steps
1. Update product specs and README to inline-only examples.
2. Add active plan for this migration.
3. Implement compiler validation for disallowed `value` and required non-empty inline content.
4. Migrate examples and tests from `value` form to inline content form.
5. Add compiler negative tests for `value` usage and empty inline content.
6. Run quality gate (`validate:docs`, `lint`, `typecheck`, `npm test`) with 100% coverage.

## Verification
- No `value`-attribute form remains for `<text>/<code>` in active docs/examples/tests/README.
- `<text>`/`<code>` with `value` attribute fail with `XML_ATTR_NOT_ALLOWED`.
- `<text></text>` and `<code>   </code>` fail with `XML_EMPTY_NODE_CONTENT`.
- Test and coverage gates remain fully green.

## Risks and Mitigations
- Risk: broad fixture migration introduces brittle string mismatches.
  - Mitigation: run global grep after migration and full test suite.
- Risk: empty inline nodes in hidden fixtures now fail compile.
  - Mitigation: add explicit negative tests and clear error messages.

## Done Criteria
- [x] Specs/docs updated
- [x] Compiler behavior updated
- [x] Fixtures/examples migrated
- [x] Tests and quality gate passing
- [x] Plan moved to completed in delivery commit
