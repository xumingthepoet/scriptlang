# Main-Anchored Include Closure

## Objective
- Make project compilation traverse includes only from the file that declares `<script name="main">`.
- Ensure non-included `.script.xml` files are not compiled/reachable.

## Scope
- In scope:
  - Compiler include root selection logic.
  - Example updates for multi-file scripts to include dependencies explicitly.
  - Spec/docs updates that define main-anchored closure behavior.
  - Test updates for include root behavior.
- Out of scope:
  - Runtime `call` semantics changes.
  - Include syntax format changes.

## Interfaces / Contracts Affected
- Public API changes:
  - No function signature change.
  - Behavioral change: `compileScriptsFromXmlMap` compiles only files reachable from the main script file include closure.
- Snapshot/schema changes:
  - None.
- XML syntax/semantic changes:
  - Multi-file script projects must include dependent script/type files from main closure.

## Implementation Steps
1. Update product specs and README to state main-anchored include closure.
2. Update compiler reachability root discovery to start from `script name="main"` file.
3. Update examples (07 battle duel) to declare script includes explicitly.
4. Update tests for compile map behavior and include reachability.
5. Run `npm run validate:docs`, `npm run lint`, `npm run typecheck`, `npm test`.

## Verification
- Unit tests:
  - `test/api.test.ts`
  - `test/compiler.test.ts`
  - `test/cli-agent.test.ts` (example scenario still runs)
- Integration tests:
  - `npm test`
- Manual checks:
  - `npm run player:agent -- start --example 07-battle-duel --state-out ...`

## Risks and Mitigations
- Risk: existing multi-file examples break due to missing include declarations.
  - Mitigation: add explicit include lines to main script files and tests.

## Rollout
- Migration notes:
  - For multi-file projects, main script header must include every required script/type dependency directly or transitively.
- Compatibility notes:
  - Breaking behavior for projects that relied on implicit “all scripts are roots”.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [ ] Plan moved to completed (in same delivery commit)
