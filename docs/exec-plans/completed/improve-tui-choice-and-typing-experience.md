# Improve TUI Choice And Typing Experience

## Objective
- Improve TUI usability by switching choice interaction to arrow/enter navigation, stabilizing choice list layout, and adding a typing animation for text output.

## Scope
- In scope:
  - TUI choice selection with Up/Down + Enter.
  - Choice list viewport fixed to 5 rows with scroll for overflow.
  - Text area typewriter effect at 5 chars/second.
  - TUI product spec sync.
- Out of scope:
  - Agent mode behavior changes.
  - Runtime engine output semantics.
  - New CLI arguments/configuration.

## Interfaces / Contracts Affected
- Public API changes:
  - none.
- Snapshot/schema changes:
  - none.
- XML syntax/semantic changes:
  - none.
- CLI/TUI interaction changes:
  - TUI selection no longer uses `1..9` keys; uses Up/Down + Enter.
  - TUI choice panel keeps a fixed 5-row height and scroll window.
  - TUI text rendering is animated at 5 chars/second.

## Implementation Steps
1. Update `/docs/product-specs/player-cli.md` with new TUI interaction and rendering rules.
2. Refactor `src/cli/commands/tui.tsx` input handling and render layout for selection cursor + scroll window.
3. Add typewriter state machine for text rendering at 200ms per character.
4. Run full `npm test` quality gate and verify no regression.
5. Move plan to completed in the same delivery commit.

## Verification
- Unit tests:
  - existing CLI/core unit suite remains green.
- Integration tests:
  - full `npm test`.
- Manual checks:
  - run TUI and verify Up/Down/Enter, fixed 5-row list behavior, and typing animation speed.

## Risks and Mitigations
- Risk:
  - text animation may temporarily desync with boundary updates.
  - Mitigation:
    - queue incoming lines and render sequentially in a single state flow.
- Risk:
  - scrolling math may select out-of-range options.
  - Mitigation:
    - centralize index clamp and visible-window adjustment.

## Rollout
- Migration notes:
  - none.
- Compatibility notes:
  - keybinding behavior intentionally changed for TUI mode.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
