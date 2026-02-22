# Player CLI External Scripts Dir

## Objective
- Extend `scriptlang-player` to run external ScriptLang directories, with fixed entry script name `main`.

## Scope
- In scope:
  - `tui` supports `--scripts-dir <path>` as source alternative to `--example`.
  - `agent start` supports `--scripts-dir <path>` as source alternative to `--example`.
  - `agent choose` resumes from saved state for both bundled examples and external sources.
  - source/argument validation and stable error codes.
  - tests and docs updates.
- Out of scope:
  - dynamic entry script flag (`--entry-script`) support.
  - replacing bundled registry with full auto-discovery.

## Interfaces / Contracts Affected
- CLI contract:
  - `scriptlang-player tui (--example <id> | --scripts-dir <path>) [--state-file <path>]`
  - `scriptlang-player agent start (--example <id> | --scripts-dir <path>) --state-out <path>`
- Entry rule:
  - external mode always starts from `<script name="main">`.

## Implementation Steps
1. Add external source loader in CLI core for script directories.
2. Add state-source restoration path so `choose` can resume external runs.
3. Update `agent` source-flag parsing and validation.
4. Update `tui` source-flag parsing and validation.
5. Update usage/help text and product spec docs.
6. Add/adjust tests for source parsing, external loading, and resume.
7. Run `validate:docs`, `typecheck`, `coverage:strict`, `npm test`.

## Verification
- `agent start --scripts-dir <dir> --state-out <path>` reaches boundary.
- `agent choose` works with state produced from external mode.
- `tui --scripts-dir <dir>` boots and runs like example mode.
- Missing `main` in external scripts returns stable error.

## Risks and Mitigations
- Risk: ambiguous source flags (`--example` and `--scripts-dir` both provided).
  - Mitigation: strict mutual exclusion validation.
- Risk: non-deterministic resume if state lacks source context.
  - Mitigation: encode source identity in scenario/state id and resolve by ref.

## Done Criteria
- [x] Specs updated
- [x] Tests added/updated and passing
- [x] Plan moved to completed
