# Add Input Waiting Node + Interactive Protocol

## Summary
- Add executable `<input var="..." text="..."/>` to collect player text input.
- Input is a first-class interactive boundary (same level as choice waiting).
- Runtime/output protocol extends with `input` boundary and submit API.
- Snapshot upgrades to `snapshot.v2` with pending interactive boundary union.

## Locked Decisions
1. `<input>` syntax only supports `var` + `text`.
2. `default` attribute is unsupported.
3. Input empty/whitespace submit falls back to target variable current value.
4. Target variable must be compile-time visible and `string` typed.
5. Runtime host path is `next()->input` + `submitInput(text)`.
6. CLI adds `agent input`.
7. Snapshot `v1` is intentionally dropped; resume requires `snapshot.v2`.

## Interfaces
- `EngineOutput` adds `kind: "input"` payload.
- `ScriptLangEngine` adds `submitInput(text: string)`.
- Agent protocol adds `EVENT:INPUT` + `INPUT_DEFAULT_JSON`.
- Snapshot type upgrades to V2 with pending boundary union (`choice` or `input`).

## Work Breakdown
1. Spec/doc sync (`index.md`, `syntax-manual.md`, `player-cli.md`, `README.md`).
2. Compiler support for `<input>` parsing + strong var/type validation.
3. Runtime pending-boundary refactor + submitInput + snapshot.v2.
4. API/CLI core type migration from snapshot.v1 to v2.
5. Agent/TUI command support for input boundary.
6. Choice traversal tool support for input auto-submit.
7. New runnable example `examples/scripts/16-input-name`.
8. Unit/smoke updates and full `npm test`.

## Acceptance
1. `<input>` can block and resume correctly from engine/API/CLI/TUI.
2. Empty submit falls back to current variable value.
3. `snapshot.v1` resume fails with schema error; `snapshot.v2` resumes.
4. Traversal and smoke still terminate.
5. `npm test` + strict coverage pass.
