# Support `<break/>` and `<continue/>` inside `<while>`

## Objective
- Add structured loop control for `<while>` body:
- `<break/>` exits the nearest while loop.
- `<continue/>` skips current iteration and re-checks nearest while condition.

## Scope
- In scope:
- Compiler recognition and validation of `<break/>` and `<continue/>`.
- Runtime execution semantics for nearest-while targeting.
- Nested while behavior coverage.
- Docs/tests/example updates.
- Out of scope:
- Choice-specific continue behavior (covered by separate choice plan).
- Non-while loop constructs.

## Interfaces / Contracts Affected
- Public API changes:
- No API signature changes.
- Runtime progression can now short-circuit loop bodies via break/continue.
- Snapshot/schema changes:
- No snapshot schema changes required.
- XML syntax/semantic changes:
- `<break/>` valid only inside `<while>` body.
- `<continue/>` valid inside `<while>` body (non-while placement is compile error unless explicitly handled by choice plan).

## Implementation Steps
1. Spec first: update `docs/product-specs/index.md` and `docs/product-specs/syntax-manual.md` for while-control nodes and placement rules.
2. Compiler: add node kinds for `break`/`continue`; track loop depth during group compilation; reject illegal placement.
3. Runtime:
   - On `break`: unwind to nearest while owner frame and advance past that while node.
   - On `continue`: unwind to nearest while owner frame and re-evaluate loop condition.
4. Nested-loop correctness: ensure nearest while wins for both operations.
5. Tests: compile-time illegal-placement tests and runtime behavioral tests for single/nested loops.
6. Example: add `examples/scripts/12-while-break-continue/main.script.xml`.

## Verification
- Unit tests:
- `break` exits only current nearest while.
- `continue` re-checks condition without executing remaining statements in current iteration.
- Illegal placement returns compile error.
- Integration tests:
- `npm test` full gate.
- Manual checks:
- Execute example and verify emitted text sequence matches expected loop control flow.

## Example (Required)
- Target file: `examples/scripts/12-while-break-continue/main.script.xml`
- Draft snippet:
```xml
<script name="main">
  <var name="i" type="number" value="0"/>
  <while when="i &lt; 6">
    <code>i = i + 1;</code>
    <if when="i == 2">
      <continue/>
    </if>
    <if when="i == 5">
      <break/>
    </if>
    <text>tick-${i}</text>
  </while>
  <text>done-${i}</text>
</script>
```

## Risks and Mitigations
- Risk:
- Incorrect frame unwinding can corrupt control flow in nested loops.
- Mitigation:
- Add targeted nested-loop tests and explicit runtime invariant checks for missing loop targets.

## Rollout
- Migration notes:
- Existing scripts unaffected unless they add new loop-control nodes.
- Compatibility notes:
- No snapshot schema migration needed for this isolated feature.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)

