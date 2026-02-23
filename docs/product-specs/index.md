# Product Specs Index

- [ScriptLang Syntax Manual](./syntax-manual.md)
- [ScriptLang Player CLI Spec](./player-cli.md)

## ScriptLang V1 Scope
- XML-first branching narrative scripts.
- Implicit group-based execution model.
- Type-checked variables declared via `<script args="...">` and executable `<var .../>`.
- `<code>` node as primary mutation and logic mechanism.
- Ink-style pull runtime API: `next()`, `choose()`, `waitingChoice`, `snapshot()`, `resume()`.

## Current Canonical Behavior
1. Every script compiles to a root implicit group.
2. `if/while/choice/call` execute via implicit child groups.
3. Snapshot persistence is based on current node/group path + ancestor scopes.
4. No language-level random builtin.
5. Host function access is explicit and whitelisted.

## XML Surface (Implemented)
- Required root: `<script>`.
- Script ID is `name`; runtime lookup and `<call script="...">` use this ID.
- Optional script params in `args="name:type,name2:type:ref"`.
- Executable nodes are direct children of `<script>`.
- Supported executable nodes:
  - `<var name="..." type="..." value="..."/>`
  - `<text>...</text>`
  - `<code>...</code>`
  - `<if when="...">...</if>` with optional `<else>`.
  - `<while when="...">...</while>`
  - `<choice><option ...>...</option></choice>`
  - `<option>` supports `text` (required) and `when` (optional); `once` is not supported.
  - `<call script="..." args="name:value,name2:ref:path"/>`
  - `<return/>` and `<return script="..."/>`
- Explicitly removed nodes: `<vars>`, `<step>`, `<set>`, `<push>`, `<remove>`.

## Runtime Behavior (Implemented)
- Ink-like API:
  - `next()` returns `text`, `choices`, or `end`.
  - `choose(index)` consumes current pending choice.
  - `waitingChoice` indicates whether a choice is pending.
- Snapshot:
  - Only allowed when `waitingChoice === true`.
  - Resume requires same compiler version string.
- Type behavior:
  - Script parameters come from `<script args="...">`.
  - `<var>` scope is declaration-point to current block end.
  - Runtime rejects `undefined` and `null` assignments into declared script variables.
  - Runtime enforces declared types on script variables.
  - Supported language types are primitives (`number|string|boolean`), arrays, and `Map<string, T>` (no `null` type and no `Record<string, T>`).
