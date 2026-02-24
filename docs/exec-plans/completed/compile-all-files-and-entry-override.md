# Compile All Files + Entry Override

## Summary
- Switch compiler scope from `main` include-closure compilation to whole-project compilation for loaded source files.
- Keep visibility discipline unchanged: each script still resolves defs/json/functions through its own include closure.
- Add optional entry override for CLI `tui`/`agent start` and API entry start.
- Keep snapshot/state schema unchanged.

## Locked Product Decisions
1. Entry support surface: CLI + API both support explicit entry script.
2. Main rule: `main` required only when entry is omitted.
3. Compile scope: all loaded `.script.xml`, `.defs.xml`, `.json` files must compile.
4. CLI scripts-dir load mode: recursive directory scan.
5. Entry args: API only.

## Public Interface Changes
- API:
  - `createEngineFromXml` adds optional `entryArgs?: Record<string, unknown>`.
  - `entryScript` resolution:
    - explicit entry -> must exist (`API_ENTRY_SCRIPT_NOT_FOUND` when missing)
    - omitted entry -> default `main` (`API_ENTRY_MAIN_NOT_FOUND` when missing)
- Runtime:
  - `ScriptLangEngine.start(entryScriptName, entryArgs?)`
- CLI:
  - `tui --scripts-dir <path> [--entry-script <name>] [--state-file <path>]`
  - `agent start --scripts-dir <path> [--entry-script <name>] --state-out <path>`
  - `agent choose` unchanged.

## Implementation Plan
1. Spec/doc sync:
   - update product specs and README for full-compile + entry override semantics.
2. Compiler:
   - replace main-rooted path collection with all-loaded path compilation.
   - validate include graph missing/cycle globally.
   - compile every script file IR, regardless of reachability from `main`.
3. API/Runtime:
   - add explicit-entry resolution and `entryArgs` plumbing to engine start.
4. CLI/source loader:
   - recursive scripts-dir scan with POSIX relative keys.
   - expose and parse `--entry-script` for `tui` and `agent start`.
   - map missing explicit entry to a dedicated CLI error code.
5. Example:
   - add `examples/scripts/15-entry-override-recursive`.
6. Tests:
   - update unit + smoke expectations.
7. Gate:
   - run `npm test` and keep strict coverage passing.

## Acceptance Criteria
1. All loaded source files compile/validate; unreachable script errors are no longer ignored.
2. CLI and API can start from non-main entry when provided.
3. Omitted entry still defaults to `main`.
4. Recursive scripts-dir loading works for nested source files.
5. `npm test` passes with strict gate.
