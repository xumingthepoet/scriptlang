# ScriptLang Player CLI Spec (V1)

This document defines the user-facing behavior of `scriptlang-player`.

## 1. Modes

`scriptlang-player` has two modes:

1. `tui`: interactive player built with Ink.
2. `agent`: non-interactive mode for AI agent command orchestration.

Both modes run the same ScriptLang engine semantics.

## 2. Scenario Source

- Scenarios are loaded from `examples/scripts/`.
- Each scenario has:
  - `id`
  - `title`
  - `entryScript`
  - one or more `.script.xml` files.

## 3. TUI Mode

Command:

```bash
scriptlang-player tui --example <id> [--state-file <path>]
```

Behavior:

- Auto-runs `next()` until a boundary:
  - `choices`
  - `end`
- Displays:
  - current scenario title
  - accumulated text output
  - choices (with numeric index) when waiting choice
  - status/help footer

Key bindings:

- `1..9`: choose option index.
- `s`: save state.
- `l`: load state.
- `r`: restart scenario from entry script.
- `h`: toggle help.
- `q`: quit.

State handling:

- Default state file: `./.scriptlang/save.bin`.
- Save is only valid while waiting for a choice (snapshot constraint).

## 4. Agent Mode

Commands:

```bash
scriptlang-player agent list
scriptlang-player agent start --example <id> --state-out <path>
scriptlang-player agent choose --state-in <path> --choice <index> --state-out <path>
```

Output protocol (stdout, line-based):

1. `RESULT:OK|ERROR`
2. `EVENT:TEXT|CHOICES|END`
3. `TEXT_JSON:<json-string>` (zero or more lines)
4. `CHOICE:<index>|<json-string>` (zero or more lines)
5. `STATE_OUT:<path|NONE>`
6. On error:
   - `ERROR_CODE:<code>`
   - `ERROR_MSG_JSON:<json-string>`

Rules:

- `start` runs from scenario entry until boundary (`choices` or `end`).
- `choose` resumes from `--state-in`, applies selection, then runs to next boundary.
- `state` is persisted only when output boundary is `CHOICES`.
- if boundary is `END`, output must be `STATE_OUT:NONE`.

## 5. Errors

- Bad arguments return `RESULT:ERROR`.
- Unknown example id returns `RESULT:ERROR`.
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
