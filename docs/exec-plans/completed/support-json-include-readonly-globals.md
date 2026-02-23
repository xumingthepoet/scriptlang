# Support JSON Include Readonly Globals

## Objective
- Support including `*.json` assets through header include directives and expose them as per-script visible readonly globals (e.g. `x.json -> x`) for game data usage.

## Scope
- In scope:
  - reachable include graph accepts `.json` targets in addition to XML files.
  - compiler parses reachable JSON via `JSON.parse`.
  - compiler validates JSON symbol names and duplicate symbol collisions.
  - per-script JSON visibility follows each script's include closure (transitive).
  - runtime exposes JSON globals as readonly variables and blocks top-level and nested writes.
  - API wires compiled JSON globals into engine construction.
  - CLI scenario loaders include `.json` assets from bundled scenarios and external script dirs.
  - tests and coverage updates.
- Out of scope:
  - include alias syntax (e.g. `include as`).
  - JSON schema validation beyond strict parsing.
  - merging or auto-renaming duplicate JSON symbols.

## Interfaces / Contracts Affected
- Public API changes:
  - `compileProjectFromXmlMap` returns compiled `globalJson` alongside `scripts` and `entryScript`.
  - engine options accept `globalJson`.
- Snapshot/schema changes:
  - none.
- XML syntax/semantic changes:
  - include targets may now be `.json` files.
  - JSON basename symbols become readonly globals visible by include closure.

## Implementation Steps
1. Update product specs (`index.md`, `syntax-manual.md`, `player-cli.md`) to declare JSON include semantics and readonly behavior.
2. Extend compiler project build pipeline:
   - parse reachable JSON files,
   - validate symbol naming/dedup,
   - compute per-script visible JSON names,
   - attach visible symbol list to each `ScriptIR`,
   - return `{ scripts, globalJson }` in a new project compile entrypoint.
3. Preserve backward compatibility:
   - keep `compileProjectScriptsFromXmlMap` returning only scripts.
4. Extend API compile/create/resume paths to consume project bundle output and pass `globalJson` into engine.
5. Extend runtime engine:
   - store global JSON payload,
   - create deep readonly proxy wrappers,
   - surface globals in variable reads by script visibility,
   - reject all writes to global JSON symbols with stable error code.
6. Extend CLI scenario registry to load `.json` assets from scenario directories.
7. Add/update tests across compiler/runtime/api/cli/coverage.
8. Run full gate (`npm test`) and fix any coverage regressions.

## Verification
- Unit tests:
  - compiler errors for malformed JSON / invalid symbol / duplicate symbol.
  - runtime reads deep JSON paths and rejects writes at any depth.
  - API and CLI load/use JSON assets successfully.
- Integration tests:
  - end-to-end `createEngineFromXml` with included JSON + include-closure visibility behavior.
- Manual checks:
  - none required beyond automated tests.

## Risks and Mitigations
- Risk:
  - readonly proxy behavior may miss some mutation paths.
  - Mitigation:
    - trap `set/deleteProperty/defineProperty/setPrototypeOf` and add branch tests.
- Risk:
  - include graph now mixes XML and JSON and may affect existing error branches.
  - Mitigation:
    - preserve existing XML root validation for XML files; add JSON-specific parse branches and tests.

## Rollout
- Migration notes:
  - no migration required; this is additive.
- Compatibility notes:
  - existing projects remain valid.
  - `compileProjectScriptsFromXmlMap` output contract remains unchanged.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
