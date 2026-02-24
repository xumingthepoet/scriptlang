# Choice Prompt Text For TUI

## Objective
- Add a choice-level prompt string via `<choice text="...">` so authors can show non-history guidance text above options in TUI.

## Scope
- In scope:
  - XML syntax extension: optional `text` attribute on `<choice>`.
  - Runtime choice boundary carries rendered choice prompt text.
  - Snapshot/resume preserves rendered prompt text for deterministic resume.
  - TUI replaces the fixed choice header line with the choice prompt when present.
  - Product spec updates for syntax and player behavior.
- Out of scope:
  - Changing `<option text="...">` semantics.
  - Agent mode output format redesign beyond compatibility updates required by shared contracts.
  - New CLI flags or user configuration.

## Interfaces / Contracts Affected
- Public API changes:
  - `EngineOutput` choices payload gains optional prompt text field (for host integrations such as TUI).
  - CLI boundary model in `src/cli/core/engine-runner.ts` follows the same field.
- Snapshot/schema changes:
  - Keep `snapshot.v1`; add optional `pendingChoicePromptText` to persisted waiting-choice payload for deterministic resume.
  - Resume validation accepts missing `pendingChoicePromptText` as `null` for older snapshots.
- XML syntax/semantic changes:
  - `<choice text="...">` becomes valid.
  - Choice-level `text` is guidance text only: it is not emitted as `<text>` output and must not be appended to the text history area.

## Implementation Steps
1. Update `/docs/product-specs/index.md`, `/docs/product-specs/syntax-manual.md`, and `/docs/product-specs/player-cli.md` to define `<choice text="...">` semantics and TUI rendering rules.
2. Extend core types in `/src/core/types.d.ts` to represent choice prompt text in IR, runtime output, and snapshot payload.
3. Update compiler parsing in `/src/compiler/compiler.ts` to read optional `<choice text>` and store it on the compiled choice node.
4. Update runtime in `/src/runtime/engine.ts` to render/store pending choice prompt text, return it in `next()` choices output, and persist/restore it through snapshot flow.
5. Update CLI boundary plumbing (`/src/cli/core/engine-runner.ts`, `/src/cli/commands/agent.ts` as needed) so shared types remain consistent.
6. Update TUI rendering in `/src/cli/commands/tui.tsx` to display choice prompt text between text viewport and option list, with fallback to the current default header when absent.
7. Add or update tests in compiler/runtime/CLI suites to cover parsing, runtime output, snapshot determinism, and TUI display behavior; then run full `npm test`.

## Verification
- Unit tests:
  - compiler test: `<choice text="...">` compiles and remains optional.
  - runtime test: `next()` choices include prompt text and do not emit it as `text` output.
  - runtime snapshot test: rendered prompt text round-trips across `snapshot()` and `resume()`.
- Integration tests:
  - `npm test` full gate passes.
- Manual checks:
  - Run TUI with a scenario using `<choice text="...">` and verify the prompt appears above options while text history remains unchanged.
  - Verify fallback header remains for `<choice>` without `text`.

## Risks and Mitigations
- Risk:
  - Prompt interpolation plus `random()` can drift across resume if not persisted as rendered text.
  - Mitigation:
    - persist rendered prompt text in pending-choice snapshot payload and reuse it on resume.
- Risk:
  - Shared output-type change can break CLI compilation/tests.
  - Mitigation:
    - update boundary/agent typing in one pass and add regression tests around choice boundary serialization.

## Rollout
- Migration notes:
  - Existing scripts remain valid; `text` on `<choice>` is optional.
- Compatibility notes:
  - No backward-compat guarantee is required in development phase; resume should still tolerate snapshots without prompt text by defaulting to `null`.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
