# Product Specs Index

- [ScriptLang Syntax Manual](./syntax-manual.md)
- [ScriptLang Player CLI Spec](./player-cli.md)

## Current Scope (Runtime V1 + Syntax V3)
- XML-first branching narrative scripts.
- Implicit group-based execution model.
- Type-checked variables declared via `<script args="...">` and executable `<var .../>`.
- Custom types declared in `*.types.xml` files and resolved per-script include closure.
- JSON global data declared in `*.json` files and resolved per-script include closure.
- Header include graph resolution via `<!-- include: ... -->` (closure starts from `script name="main"`).
- `<code>` node as primary mutation and logic mechanism.
- Ink-style pull runtime API: `next()`, `choose()`, `waitingChoice`, `snapshot()`, `resume()`.

## Current Canonical Behavior
1. Every script compiles to a root implicit group.
2. `if/while/choice/call` execute via implicit child groups.
3. Snapshot persistence is based on current node/group path + ancestor scopes.
4. Language-level `random(n)` builtin exists and is deterministic when seeded.
5. Host function access is explicit and whitelisted.

## XML Surface (Implemented)
- Allowed roots: `<script>` and `<types>`.
- Script ID is `name`; runtime lookup and `<call script="...">` use this ID.
- Type collection root: `<types name="...">`.
- Header include directives are supported in script/type XML roots:
  - `<!-- include: rel/path.ext -->`
  - include traversal starts at the file that declares `<script name="main">`
  - only files reachable from that closure are compiled
  - each script can use only custom types and JSON globals reachable from that script file's own include closure (transitive)
- Reachable `.json` assets are compiled into global read-only symbols:
  - symbol name is file basename without `.json`
  - invalid symbol names and duplicate symbols are compile errors
- Optional script params in `args="[ref:]type:name,[ref:]type:name2"`.
- Executable nodes are direct children of `<script>`.
- Supported executable nodes:
  - `<var name="..." type="..." value="..."/>`
  - `<text>...</text>`
  - `<code>...</code>`
  - `<if when="...">...</if>` with optional `<else>`.
  - `<while when="...">...</while>`
  - `<choice text="..."><option ...>...</option></choice>`
  - `<choice>` supports optional `text` (non-empty when present) as choice prompt text for host display.
  - `<option>` supports `text` (required) and `when` (optional); `once` is not supported.
  - `<call script="..." args="[ref:]value,[ref:]value2"/>` (positional; maps to script arg declaration order)
  - `<return/>` and `<return script="..." args="[value,value2,...]"/>`
- Explicitly removed nodes: `<vars>`, `<step>`, `<set>`, `<push>`, `<remove>`.

## Runtime Behavior (Implemented)
- Ink-like API:
  - `next()` returns `text`, `choices`, or `end`.
  - `choose(index)` consumes current pending choice.
  - `waitingChoice` indicates whether a choice is pending.
  - `choices` output may include optional rendered `promptText` from `<choice text="...">`.
- Snapshot:
  - Only allowed when `waitingChoice === true`.
  - Resume requires same compiler version string.
  - Snapshot payload includes runtime RNG state, rendered pending choice items, and rendered pending choice prompt text for deterministic resume.
- Type behavior:
  - Script parameters come from `<script args="...">`.
  - `<call ... args="...">` arguments are positional and map by target script arg order.
  - `<return script="..." args="...">` arguments are positional and value-only.
  - Compiler rejects any `ref:` segment in return args.
  - `<var>` scope is declaration-point to current block end.
  - Runtime rejects `undefined` and `null` assignments into declared script variables.
  - Runtime enforces declared types on script variables.
  - Supported language types are primitives (`number|string|boolean`), arrays, `Map<string, T>`, and custom object types visible to the current script include closure.
  - Custom object types are strict: missing/extra/wrong-typed fields are rejected.
  - Reachable included `.json` files are exposed as read-only globals by symbol name (`file.json -> file`) and are visible only in the including script's include closure.
  - Any write to JSON globals (top-level or nested) is a runtime error.
  - `createEngineFromXml` defaults to `main` when `entryScript` is omitted.
- Builtins:
  - `random(n)` is available in script expressions and `<code>` blocks without `hostFunctions`.
  - `n` must be a positive integer.
  - `random(n)` returns an integer in `[0, n-1]` using deterministic seeded PRNG state.
  - `Math.random` remains available and is not overridden by ScriptLang.
