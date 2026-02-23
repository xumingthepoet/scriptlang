# Fix TUI Scroll Jitter With Bounded Text Viewport

## Objective
- Eliminate visible up/down jump in TUI while text is streaming after content exceeds terminal height.

## Scope
- In scope:
  - keep TUI output height bounded to terminal height.
  - render text in a bounded viewport (show latest lines) instead of unbounded growth.
  - preserve fixed choice area height and divider.
  - update TUI product spec.
- Out of scope:
  - agent mode changes.
  - runtime engine behavior changes.

## Interfaces / Contracts Affected
- Public API changes:
  - none.
- Snapshot/schema changes:
  - none.
- XML syntax/semantic changes:
  - none.
- CLI/TUI interaction changes:
  - TUI text history is displayed in a bounded viewport to avoid terminal scrollbar jitter.

## Implementation Steps
1. Update `/docs/product-specs/player-cli.md` with bounded text viewport behavior.
2. Refactor `/src/cli/commands/tui.tsx` to compute available text rows from terminal size and clip rendered text lines.
3. Keep divider and choice area as stable-height layout blocks.
4. Run full `npm test`.
5. Move plan to completed in delivery commit.

## Verification
- Unit tests:
  - existing suite remains green.
- Integration tests:
  - full `npm test` gate.
- Manual checks:
  - with long-running script, TUI no longer causes terminal-level scrolling/jitter during streaming.

## Risks and Mitigations
- Risk:
  - clipping text viewport may hide older lines.
  - Mitigation:
    - keep “latest lines” behavior explicit in spec and preserve stable interaction.

## Rollout
- Migration notes:
  - none.
- Compatibility notes:
  - TUI visual behavior intentionally changed to stabilize rendering.

## Done Criteria
- [x] Specs updated
- [x] Architecture docs still valid
- [x] Tests added/updated and passing
- [x] Plan moved to completed (in same delivery commit)
