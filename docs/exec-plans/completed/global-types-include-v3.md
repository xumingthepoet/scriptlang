# Global Types And Include Graph V3

## Objective
- Add global custom type declarations through `*.types.xml`.
- Add header `include` graph resolution across `*.script.xml` and `*.types.xml`.
- Compile all reachable scripts and run with `script name="main"` as default entry.

## Scope
- In scope:
  - `<types name=\"...\">` root support with `<type>/<field>` declarations.
  - Custom type usage in script args and var declarations.
  - Header include syntax: `<!-- include: rel/path.xml -->`.
  - Include traversal from all `.script.xml` roots.
  - Include cycle/missing include errors.
  - Runtime strict object type defaults and validations.
  - CLI loader support for both `.script.xml` and `.types.xml`.
- Out of scope:
  - Namespace-qualified type lookup.
  - Compatibility flags and legacy syntax shims.

## Interfaces / Contracts Affected
- New root:
  - `<types name=\"gamestate\"> ... </types>`
- Type syntax:
  - keep `number|string|boolean`, `T[]`, `Map<string, T>`
  - add global custom type names (bare identifiers)
- Include:
  - header comment lines only
  - relative path resolution
  - cycle -> compile error
  - missing include target -> compile error
- Entry:
  - default entry is `main` when `entryScript` is not provided.

## Implementation Steps
1. Update product specs for global type roots and include directives.
2. Extend core type model with object/custom type branch.
3. Add compiler project graph traversal and `<types>` declaration resolver.
4. Wire API compile/create/resume through project compile path.
5. Extend runtime type default/check logic for object types.
6. Update CLI scenario loading to include `.types.xml`.
7. Add/adjust unit tests and branch coverage tests.
8. Run full quality gate and keep 100% coverage.

## Verification
- Scripts can declare variables/args using custom types defined in included `.types.xml`.
- Duplicate custom type names fail compile.
- Recursive custom type references fail compile.
- Include cycles and missing include targets fail compile.
- Default create path works with implicit `main` entry.
- CLI `--scripts-dir` supports mixed script/types files.

## Risks and Mitigations
- Risk: include traversal introduces new branch complexity.
  - Mitigation: explicit branch tests for missing/cycle/header parsing.
- Risk: strict object checks break loose fixtures.
  - Mitigation: migrate fixtures and add precise error assertions.

## Done Criteria
- [x] Specs/docs updated
- [x] Compiler/API/runtime/CLI updates landed
- [x] Tests updated and passing
- [x] 100% coverage preserved
- [ ] Plan moved to completed in delivery commit
