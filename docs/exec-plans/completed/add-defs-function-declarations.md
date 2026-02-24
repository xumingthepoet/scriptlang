# Add `<function>` in `<defs>` + Replace `<types>` With `<defs>`

## Summary
- Add declaration-only `<function>` in `.defs.xml` roots.
- Remove legacy `.types.xml` / `<types>` (no compatibility bridge).
- Make functions callable from expression and `<code>` contexts without ScriptLang node/frame jumps.

## Locked Decisions
1. Functions are declared only in `<defs>`, never in `<script>`.
2. `<defs>` supports sibling `<type>` and `<function>` declarations.
3. Function visibility follows per-script include closure.
4. Function args are value-only typed params (`type:name`), no `ref:`.
5. Function `return="type:name"` is required.
6. Return variable is pre-initialized with type default and write-checked by declared type.
7. Function call arity is exact.
8. Function body scope excludes script vars; locals are args + return var only.
9. Function body can access visible defs functions, visible JSON globals, `random`, `Math`, and host functions.
10. Strict conflicts:
   - duplicate visible function names => compile error
   - function name vs script args/vars => compile error
   - function name vs visible JSON / `random` / `Math` => compile error
   - function name vs host function => runtime init error
11. Recursion is allowed.

## Implementation Steps
1. Spec/docs sync (`product-specs`, `README`, this plan).
2. Extend core IR types for function declarations and script-visible function maps.
3. Refactor compiler defs parsing:
   - accept `<defs>` root
   - parse `<type>` and `<function>`
   - remove `<types>` root support
4. Include-closure resolution:
   - resolve visible types and visible functions per script path
   - attach visible function map to each `ScriptIR`
   - enforce name conflicts.
5. Runtime function execution:
   - expose defs functions in evaluation sandbox
   - implement typed function invocation without runtime frame/node transitions.
6. CLI loader migration to `.defs.xml`.
7. Example migration + new function example.
8. Unit and smoke test updates.
9. Run full gate and ship.

## Done Criteria
- `.types.xml` / `<types>` fully removed from runtime behavior and docs.
- `.defs.xml` / `<defs>` works for both custom types and functions.
- Function calls work in `<code>` and `${...}` with strict type + arity checks.
- No new ScriptLang runtime frame/node transitions for function calls.
- `npm test` passes with existing 100% coverage thresholds.
