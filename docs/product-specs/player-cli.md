# ScriptLang Player CLI Spec (V1)

This document defines the user-facing behavior of `scriptlang-player`.

## 1. Modes

`scriptlang-player` has two modes:

1. `tui`: interactive player built with Ink.
2. `agent`: non-interactive mode for AI agent command orchestration.

Both modes run the same ScriptLang engine semantics.

## 2. Scenario Source

- Source can be either:
  - bundled scenarios from `examples/scripts/`
  - external scripts directory via `--scripts-dir <path>` (entry is fixed to `<script name="main">`)
- Each scenario has:
  - `id`
  - `title`
  - `entryScript`
  - one or more `.script.xml` files.
  - optional `.types.xml` files for global custom type declarations.
  - optional `.json` files for global read-only game data.
- Bundled examples currently include:
  - `01-text-code`
  - `02-if-while`
  - `03-choice-once`
  - `04-call-ref-return`
  - `05-return-transfer`
  - `06-snapshot-flow`
  - `07-battle-duel` (multi-file battle loop demo with custom `<type>` combatants, call/ref, and ending transfer)
  - `08-json-globals` (reads nested fields from included `game.json` as readonly global data)

## 3. TUI Mode

Command:

```bash
scriptlang-player tui (--example <id> | --scripts-dir <path>) [--state-file <path>]
```

Behavior:

- Auto-runs `next()` until a boundary:
  - `choices`
  - `end`
- Displays:
  - current scenario title
  - accumulated text output shown in a bounded terminal-height viewport (latest lines)
  - choices in a fixed-height viewport when waiting choice
  - choice prompt line between text viewport and choices:
    - uses rendered `<choice text="...">` when present
    - falls back to default `choices (up/down + enter):` when absent
    - does not append to the accumulated text history viewport
  - status/help footer
  - text output with typewriter animation at 60 chars/second
  - a visual divider line between text area and choice area

Key bindings:

- `↑` / `↓`: move current choice cursor.
- `enter`: confirm current choice.
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

Text viewport rules:

- text area is clipped to available terminal rows after reserving non-text UI rows.
- only latest visible text lines are rendered when history exceeds viewport capacity.
- this prevents terminal-level scroll growth and avoids streaming-time vertical jitter.

State handling:

- Default state file: `./.scriptlang/save.bin`.
- Save is only valid while waiting for a choice (snapshot constraint).
- For `--scripts-dir`, entry script is fixed to `<script name="main">`.

## 4. Agent Mode

Commands:

```bash
scriptlang-player agent list
scriptlang-player agent start (--example <id> | --scripts-dir <path>) --state-out <path>
scriptlang-player agent choose --state-in <path> --choice <index> --state-out <path>
```

Output protocol (stdout, line-based):

1. `RESULT:OK|ERROR`
2. `EVENT:TEXT|CHOICES|END`
3. `TEXT_JSON:<json-string>` (zero or more lines)
4. `PROMPT_JSON:<json-string>` (zero or one line; only for `EVENT:CHOICES` when choice prompt exists)
5. `CHOICE:<index>|<json-string>` (zero or more lines)
6. `STATE_OUT:<path|NONE>`
7. On error:
   - `ERROR_CODE:<code>`
   - `ERROR_MSG_JSON:<json-string>`

Rules:

- `start` runs from scenario entry until boundary (`choices` or `end`).
- `start` requires exactly one source selector:
  - bundled example: `--example <id>`
  - external scripts directory: `--scripts-dir <path>`
- for `--scripts-dir`, entry script is fixed to script name `main`.
- when multiple script files exist, `main` must include required script/type files via header `include` directives.
- included `.json` assets in the same scenario source are loaded and available to script runtime through include closure rules.
- `choose` resumes from `--state-in`, applies selection, then runs to next boundary.
- `state` is persisted only when output boundary is `CHOICES`.
- if boundary is `END`, output must be `STATE_OUT:NONE`.

## 5. Errors

- Bad arguments return `RESULT:ERROR`.
- Unknown example id returns `RESULT:ERROR`.
- Invalid scripts directory or missing `main` entry script returns `RESULT:ERROR`.
- Invalid choice index returns `RESULT:ERROR`.
- Corrupt/missing state file returns `RESULT:ERROR`.

Error lines must include:

- `ERROR_CODE:<code>`
- `ERROR_MSG_JSON:<json-string>`

## 6. Compatibility

- CLI behavior does not modify ScriptLang language semantics.
- Snapshot compatibility still follows runtime rules:
  - schema must be `snapshot.v1`
  - compiler version must match.
