# Enhance `<choice>` with Option `<continue/>` and `fall_over`

## Objective
- Add two focused choice capabilities:
- Allow direct child `<continue/>` inside `<option>` to return to the same choice and reselect.
- Add `<option fall_over="true">` as a hidden fallback option that appears only when all non-fall-over options are unavailable.

## Scope
- In scope:
- Compiler support and validation for `fall_over` on `<option>`.
- Compiler support for option-direct `<continue/>` targeting the parent choice.
- Runtime visibility logic for fall-over option.
- Runtime re-entry behavior for choice-continue.
- Docs/tests/example updates.
- Out of scope:
- `once`基础能力本身（由 Plan 1 提供），这里只做与 choice 的交互验证。
- `while` 的 break/continue（由 Plan 3 提供）。

## Interfaces / Contracts Affected
- Public API changes:
- No host API signature change.
- Choice runtime output may include fallback option only under trigger condition.
- Snapshot/schema changes:
- No schema field addition required by this plan.
- XML syntax/semantic changes:
- `<option fall_over="true|false">`
- A `<choice>` can have at most one `fall_over="true"` option.
- Fall-over option must be the last `<option>`.
- Fall-over option cannot declare `when`.
- `<continue/>` is allowed only as direct child of `<option>` and means “return to current `<choice>`”.

## Implementation Steps
1. Spec first: update `docs/product-specs/index.md` and `docs/product-specs/syntax-manual.md` with `fall_over` and option-direct `<continue/>` semantics and constraints.
2. Compiler: parse `fall_over`; enforce single/last/no-when rules; compile option-direct `<continue/>` as choice-target continue node.
3. Runtime (choice visibility):
   - Build visible non-fall-over options first.
   - Show fall-over option only when visible non-fall-over count is `0`.
4. Runtime (choice continue):
   - Selecting an option path that executes direct `<continue/>` returns execution pointer to the current choice node for re-prompt.
5. Tests: add compile errors for invalid fall-over authoring; add runtime cases for fallback visibility and repeat selection flow.
6. Example: add `examples/scripts/11-choice-fallover-continue/main.script.xml`.

## Verification
- Unit tests:
- Invalid fall-over authoring emits expected compile errors.
- Choice continue re-prompts same choice and supports immediate reselection.
- Fallback option stays hidden while any non-fall-over option is visible.
- Integration tests:
- `npm test` full gate.
- Manual checks:
- Run `agent start/choose` on example and confirm fallback appears only after regular options are exhausted/hidden.

## Example (Required)
- Target file: `examples/scripts/11-choice-fallover-continue/main.script.xml`
- Draft snippet:
```xml
<script name="main">
  <var name="hasKey" type="boolean" value="false"/>
  <choice text="Open the gate">
    <option text="Search nearby" when="!hasKey">
      <text>You keep searching...</text>
      <continue/>
    </option>
    <option text="Use key" when="hasKey">
      <text>The gate opens.</text>
    </option>
    <option text="Leave for now" fall_over="true">
      <text>Nothing else can be done here.</text>
    </option>
  </choice>
</script>
```

## Risks and Mitigations
- Risk:
- Choice control-flow becomes harder to reason about with nested structures.
- Mitigation:
- Restrict choice-continue to option direct child only; compile-time reject all other placements.

## Rollout
- Migration notes:
- Existing choices remain valid unless they newly adopt `fall_over`/`continue`.
- Compatibility notes:
- No snapshot schema bump required by this plan.

## Done Criteria
- [ ] Specs updated
- [ ] Architecture docs still valid
- [ ] Tests added/updated and passing
- [ ] Plan moved to completed (in same delivery commit)

