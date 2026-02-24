# Add `<loop times="...">` via Compiler Macro Layer

## Objective
- Make count-based loops easy to author with `<loop times="expr">...</loop>`.
- Introduce a dedicated compiler macro-expansion layer in a new module so future macros can be added without bloating `compiler.ts`.

## Scope
- In scope:
- Add macro-expansion module under `src/compiler/` and route script roots through it before IR compilation.
- Implement `<loop>` macro expansion into existing primitives (`<var>` + `<while>` + `<code>`).
- Enforce that `times` uses expression syntax and reject `${...}` wrapper form.
- Update docs, tests, and add one example.
- Out of scope:
- Runtime `LoopNode` support.
- `index` variable support.
- New loop runtime error contracts beyond compile-time wrapper rejection.

## Interfaces / Contracts Affected
- Public API changes:
- XML syntax adds `<loop times="...">...</loop>`.
- Snapshot/schema changes:
- None.
- XML syntax/semantic changes:
- `times` required on `<loop>`.
- `<loop times="${x}">` is compile-time error (`XML_LOOP_TIMES_TEMPLATE_UNSUPPORTED`).
- `<loop>` is compile-time sugar and not a runtime primitive.

## Implementation Steps
1. Spec first updates in `docs/product-specs/index.md` and `docs/product-specs/syntax-manual.md`.
2. Create `src/compiler/macros.ts` with extensible expansion structure (`handler map + recursive expander`).
3. Implement loop macro:
   - generate collision-safe hidden temp variable name per script.
   - expand loop node into:
     - hidden `<var ... type="number" value="timesExpr"/>`
     - `<while when="temp > 0">`
       - `<code>temp = temp - 1;</code>`
       - original loop body
4. Integrate `expandScriptMacros(...)` into `compileScript(...)` before `compileGroup(...)`.
5. Add tests for:
   - successful loop expansion behavior,
   - nested loop expansion,
   - `${...}` rejection,
   - generated temp name collision handling.
6. Add example `examples/scripts/13-loop-times/main.script.xml`.

## Verification
- Unit tests:
- `test/compiler.test.ts` and `test/runtime.test.ts` cover loop compile + execution path.
- `test/coverage-branches.test.ts` covers loop macro error/edge branches.
- Integration tests:
- `npm test`.
- Manual checks:
- `npm run player:tui -- --scripts-dir examples/scripts/13-loop-times` displays expected repeated text flow.

## Risks and Mitigations
- Risk:
- Generated temp var name collides with author variables/args.
- Mitigation:
- Pre-scan declared names and allocate unique hidden names.
- Risk:
- Macro layer changes AST locations and hurts error readability.
- Mitigation:
- Reuse source `location` from original `<loop>` node for synthetic nodes.

## Rollout
- Migration notes:
- Existing scripts are unaffected unless they adopt `<loop>`.
- Compatibility notes:
- `<loop>` is additive and compiler-only.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)

