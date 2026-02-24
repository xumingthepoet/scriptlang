# ScriptLang Player CLI Spec (V2)

This document defines the user-facing behavior of `scriptlang-player`.

## 1. Modes

`scriptlang-player` has two modes:

1. `tui`: interactive player built with Ink.
2. `agent`: non-interactive mode for AI agent command orchestration.

Both modes run the same ScriptLang engine semantics.

## 2. Scenario Source

- Source is always provided via `--scripts-dir <path>`.
- `--scripts-dir` can point to any script project directory, including directories under `examples/scripts/`.
- `--scripts-dir` is scanned recursively for supported files (`.script.xml`, `.defs.xml`, `.json`).
- Entry script defaults to `<script name="main">`.
- `--entry-script <name>` may override the default entry for `tui` and `agent start`.
- A source directory may include:
  - one or more `.script.xml` files
  - optional `.defs.xml` files for global declarations (`<type>`, `<function>`)
  - optional `.json` files for global read-only game data

## 3. TUI Mode

Command:

```bash
scriptlang-player tui --scripts-dir <path> [--entry-script <name>] [--state-file <path>]
```

Behavior:

- Auto-runs `next()` until a boundary:
  - `choices`
  - `input`
  - `end`
- Displays:
  - current scenario title
  - accumulated text output shown in a bounded terminal-height viewport (latest lines)
  - choices in a fixed-height viewport when waiting choice
  - choice prompt line between text viewport and choices:
    - uses rendered `<choice text="...">`
    - does not append to the accumulated text history viewport
  - status/help footer
  - text output with typewriter animation at 60 chars/second
  - a visual divider line between text area and choice area

Key bindings:

- `↑` / `↓`: move current choice cursor.
- `enter`: confirm current choice or submit current input line.
- `s`: save state.
- `l`: load state.
- `r`: restart scenario from entry script.
- `h`: toggle help.
- `q`: quit.

Choice viewport rules:

- choice list height is fixed to 5 rows.
- when choices exceed 5, viewport scrolls with cursor movement.
- selected row is visually highlighted.
- when new boundary text is still animating, choices must stay hidden and cannot be selected until text animation completes.
- choice area keeps its reserved layout height even when choices are hidden, to avoid vertical layout jumps.
- input mode uses the same interaction area to show prompt/default/current typed buffer.

Text viewport rules:

- text area is clipped to available terminal rows after reserving non-text UI rows.
- only latest visible text lines are rendered when history exceeds viewport capacity.
- this prevents terminal-level scroll growth and avoids streaming-time vertical jitter.

State handling:

- Default state file: `./.scriptlang/save.bin`.
- Save is only valid while waiting for a choice (snapshot constraint).
- Save is valid while waiting for `choice` or `input`.

## 4. Agent Mode

Commands:

```bash
scriptlang-player agent start --scripts-dir <path> [--entry-script <name>] --state-out <path>
scriptlang-player agent choose --state-in <path> --choice <index> --state-out <path>
scriptlang-player agent input --state-in <path> --text <value> --state-out <path>
```

Output protocol (stdout, line-based):

1. `RESULT:OK|ERROR`
2. `EVENT:CHOICES|INPUT|END`
3. `TEXT_JSON:<json-string>` (zero or more lines)
4. `PROMPT_JSON:<json-string>` (exactly one line for `EVENT:CHOICES|INPUT`)
5. `CHOICE:<index>|<json-string>` (zero or more lines, `EVENT:CHOICES` only)
6. `INPUT_DEFAULT_JSON:<json-string>` (exactly one line, `EVENT:INPUT` only)
7. `STATE_OUT:<path|NONE>`
8. On error:
   - `ERROR_CODE:<code>`
   - `ERROR_MSG_JSON:<json-string>`

Rules:

- `start` runs from source entry until boundary (`choices`, `input`, or `end`).
- `start` requires `--scripts-dir <path>` and `--state-out <path>`.
- `start` optionally accepts `--entry-script <name>`; when omitted, entry defaults to `main`.
- loaded source files are recursive from `--scripts-dir` and all supported files are compiled/validated.
- include closure still controls per-script visibility of defs/json at compile/runtime.
- `choose` resumes from `--state-in`, applies selection, then runs to next boundary.
- `input` resumes from `--state-in`, applies submitted text, then runs to next boundary.
- `state` is persisted when boundary is `CHOICES` or `INPUT`.
- `INPUT_DEFAULT_JSON` is the target variable current value captured when engine reached the `<input>` boundary.
- if boundary is `END`, output must be `STATE_OUT:NONE`.

## 5. Errors

- Bad arguments return `RESULT:ERROR`.
- Unknown/unsupported subcommand returns `RESULT:ERROR`.
- Invalid scripts directory returns `RESULT:ERROR`.
- Missing default `main` entry (when `--entry-script` is omitted) returns `RESULT:ERROR`.
- Explicit `--entry-script` not found returns `RESULT:ERROR`.
- Invalid choice index returns `RESULT:ERROR`.
- Invalid `input` operation (wrong boundary) returns `RESULT:ERROR`.
- Corrupt/missing state file returns `RESULT:ERROR`.
- State `scenarioId` that is not `scripts-dir:<absolute-path>` returns `RESULT:ERROR`.

Error lines must include:

- `ERROR_CODE:<code>`
- `ERROR_MSG_JSON:<json-string>`

## 6. Compatibility

- CLI behavior does not modify ScriptLang language semantics.
- Snapshot compatibility still follows runtime rules:
  - schema must be `snapshot.v2`
  - compiler version must match.
