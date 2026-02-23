# Product Specs Index

- [ScriptLang Syntax Manual](./syntax-manual.md)
- [ScriptLang Player CLI Spec](./player-cli.md)

## Current Scope (Runtime V1 + Syntax V3)
- XML-first branching narrative scripts.
- Implicit group-based execution model.
- Type-checked variables declared via `<script args="...">` and executable `<var .../>`.
- Custom types declared in `*.types.xml` files and resolved per-script include closure.
- Header include graph resolution via `<!-- include: ... -->` (closure starts from `script name="main"`).
- `<code>` node as primary mutation and logic mechanism.
- Ink-style pull runtime API: `next()`, `choose()`, `waitingChoice`, `snapshot()`, `resume()`.

## Current Canonical Behavior
1. Every script compiles to a root implicit group.
2. `if/while/choice/call` execute via implicit child groups.
3. Snapshot persistence is based on current node/group path + ancestor scopes.
4. No language-level random builtin.
5. Host function access is explicit and whitelisted.

## XML Surface (Implemented)
- Allowed roots: `<script>` and `<types>`.
- Script ID is `name`; runtime lookup and `<call script="...">` use this ID.
- Type collection root: `<types name="...">`.
- Header include directives are supported in both roots:
  - `<!-- include: rel/path.xml -->`
  - include traversal starts at the file that declares `<script name="main">`
  - only files reachable from that closure are compiled
  - each script can use only custom types reachable from that script file's own include closure (transitive)
- Optional script params in `args="[ref:]type:name,[ref:]type:name2"`.
- Executable nodes are direct children of `<script>`.
- Supported executable nodes:
  - `<var name="..." type="..." value="..."/>`
  - `<text>...</text>`
  - `<code>...</code>`
  - `<if when="...">...</if>` with optional `<else>`.
  - `<while when="...">...</while>`
  - `<choice><option ...>...</option></choice>`
  - `<option>` supports `text` (required) and `when` (optional); `once` is not supported.
  - `<call script="..." args="[ref:]value,[ref:]value2"/>` (positional; maps to script arg declaration order)
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
  - `<call ... args="...">` arguments are positional and map by target script arg order.
  - `<var>` scope is declaration-point to current block end.
  - Runtime rejects `undefined` and `null` assignments into declared script variables.
  - Runtime enforces declared types on script variables.
  - Supported language types are primitives (`number|string|boolean`), arrays, `Map<string, T>`, and custom object types visible to the current script include closure.
  - Custom object types are strict: missing/extra/wrong-typed fields are rejected.
  - `createEngineFromXml` defaults to `main` when `entryScript` is omitted.
