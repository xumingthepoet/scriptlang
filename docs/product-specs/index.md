# Product Specs Index

- [ScriptLang Syntax Manual](./syntax-manual.md)
- [ScriptLang Player CLI Spec](./player-cli.md)

## ScriptLang V1 Scope
- XML-first branching narrative scripts.
- Implicit group-based execution model.
- Type-checked variables declared in `<vars>`.
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
- Variable declarations in `<vars><var .../></vars>`.
- Executable container: `<step>...</step>`.
- Supported executable nodes:
  - `<text value="..."/>`
  - `<code>...</code>`
  - `<if when="...">...</if>` with optional `<else>`.
  - `<while when="...">...</while>`
  - `<choice><option ...>...</option></choice>`
  - `<call script="..." args="name:value,name2:ref:path"/>`
  - `<return/>` and `<return script="..."/>`
- Explicitly removed nodes: `<set>`, `<push>`, `<remove>`.

## Runtime Behavior (Implemented)
- Ink-like API:
  - `next()` returns `text`, `choices`, or `end`.
  - `choose(index)` consumes current pending choice.
  - `waitingChoice` indicates whether a choice is pending.
- Snapshot:
  - Only allowed when `waitingChoice === true`.
  - Resume requires same compiler version string.
- Type behavior:
  - Vars must be declared in `<vars>`.
  - Runtime rejects `undefined` assignments.
  - Runtime enforces declared types on script variables.
