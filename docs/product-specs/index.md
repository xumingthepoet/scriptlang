# Product Specs Index

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

