# Choice Traversal Tool

This document defines the current branch-traversal smoke tool for ScriptLang scenarios.

## Purpose
- Explore all currently visible `<choice>` options from each waiting-choice boundary.
- Validate every explored path reaches `END` without runtime errors.
- Fail fast if traversal exceeds configured choice-step/runtime/path limits.

## Command

```bash
npm run traverse:choices -- --examples-root examples/scripts
```

Or target a single scenario directory:

```bash
npm run traverse:choices -- --scripts-dir examples/scripts/07-battle-duel
```

## Options
- `--scripts-dir <path>`
  - Traverse one scenario directory.
- `--examples-root <path>`
  - Traverse each child directory under root that contains `main.script.xml`.
  - Default: `examples/scripts`
- `--max-choice-steps <n>`
  - Max `choose` depth per path.
  - Default: `100`
- `--max-runtime-ms <ms>`
  - Max runtime per scenario traversal.
  - Default: `30000`
- `--max-paths <n>`
  - Max completed paths per scenario before fail-fast.
  - Default: `20000`
- `--verbose`
  - Emit total summary across scenarios.

## Output
- Success line per scenario:
  - `[PASS] <scenario> paths=<n> transitions=<n> maxDepth=<n> elapsed=<ms>`
- Failure line:
  - `[FAIL] <scenario> code=<CODE> msg=<message>`
  - `trace: <choice-trace>`
- Optional summary:
  - `[SUMMARY] scenarios=<n> paths=<n> transitions=<n> maxDepth=<n> elapsed=<ms>`

## Current Approach
- Traversal mode: depth-first search over visible choice items.
- Branch source: engine choice boundary (`choices.items`), not static XML parsing.
- Seed behavior: uses engine default deterministic seed behavior (current default seed = `1`).
- Snapshot branching: each branch resumes from captured waiting-choice snapshot.
- Timeout/limit policy: hitting any limit is a hard failure.

## Important Notes
- No state dedup is used in current version.
  - Repeated equivalent states are not collapsed.
  - For high-fanout loops, increase limits carefully or refactor scenario structure.
- This tool validates runtime safety and termination for explored branches only under current limits/seed.
- Failure traces are intended to be directly reproducible via scenario + sequence of chosen indices.
