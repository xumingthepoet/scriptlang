# Reintroduce `once` for `<text>` and `<option>` (Script-Private Static State)

## Objective
- Re-add `once` as a language feature with strict scope: only `<text once="...">` and `<option once="...">`.
- Make `once` behave like script-private static state: persists for the engine instance and does not reset on `start()`.

## Scope
- In scope:
- Compiler support for `once` on `<text>` and `<option>` only.
- Runtime once-state tracking and filtering.
- Snapshot/resume persistence for once-state.
- Docs and tests for the new semantics.
- One dedicated example under `examples/scripts/`.
- Out of scope:
- `once` support on any other node.
- Any change to `choice` fallback or loop control semantics (handled by separate plans).

## Interfaces / Contracts Affected
- Public API changes:
- No new public methods.
- Runtime behavior changes for `next()/choose()` when nodes/options are marked `once`.
- Snapshot/schema changes:
- `SnapshotV1` adds optional `onceStateByScript` payload.
- XML syntax/semantic changes:
- `<text once="true|false">...`
- `<option once="true|false">...`
- `once` default is `false`.
- Invalid boolean literal is compile error.

## Implementation Steps
1. Spec first: update `docs/product-specs/index.md` and `docs/product-specs/syntax-manual.md` to define `once` syntax and persistence semantics.
2. Compiler: parse boolean `once` on `text`/`option`; reject unsupported placement with compile error.
3. Core types: extend `TextNode` and `ChoiceOption` with `once: boolean`; extend `SnapshotV1` with optional once-state field.
4. Runtime: add once-state map keyed by current script + node/option ID; skip already-fired text; hide already-fired once options; mark option as fired on `choose`.
5. Snapshot: serialize and restore once-state; validate malformed payload.
6. Example: add `examples/scripts/10-once-static/main.script.xml` demonstrating one-time intro text and one-time dialog option.
7. Tests: add compiler/runtime/coverage tests for once semantics and snapshot roundtrip.

## Verification
- Unit tests:
- Compiler accepts/rejects `once` attributes correctly.
- Runtime emits once text only once and once options disappear after selection.
- Snapshot resume preserves once-state.
- Integration tests:
- `npm test` full gate.
- Manual checks:
- `npm run player:dev -- tui --scripts-dir examples/scripts/10-once-static` and verify repeated entry does not replay once text/options within same engine lifecycle.

## Example (Required)
- Target file: `examples/scripts/10-once-static/main.script.xml`
- Draft snippet:
```xml
<script name="main">
  <text once="true">You step into the tavern for the first time.</text>
  <choice text="Talk to the bartender">
    <option text="Ask for rumors" once="true">
      <text>You hear a secret route to the castle.</text>
    </option>
    <option text="Order water">
      <text>A calm break.</text>
    </option>
  </choice>
</script>
```

## Risks and Mitigations
- Risk:
- Existing snapshot fixtures may fail if payload shape is invalid.
- Mitigation:
- Keep `onceStateByScript` optional; strict runtime validation only when field exists.

## Rollout
- Migration notes:
- This reintroduces `once`; scripts that do not use `once` are unaffected.
- Compatibility notes:
- Snapshot payload extends V1 with optional field; old snapshots remain acceptable.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)

