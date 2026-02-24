# ScriptLang

`ScriptLang` is an XML-first scripting language for branching narrative games with strong data simulation support through TypeScript code nodes.

This repository is initialized for **agent-first harness engineering**:
- specs are explicit,
- architecture boundaries are strict,
- execution is plan-driven,
- quality is measured by repeatable checks.

## Project Status
- Current phase: active development with V3 XML syntax and V2 snapshot schema (`snapshot.v2`).
- Compatibility posture: no backward-compat requirement by default during development; remove legacy syntax/behavior unless a task explicitly requires compatibility.
- Ongoing implementation tasks are tracked in `/docs/exec-plans/active/`.
- Internal reserved prefix: user-authored names starting with `__` are compile-time errors.

## Repo Map
- `/AGENTS.md`: required workflow for engineers and coding agents.
- `/docs/HARNESS.md`: canonical harness workflow authority.
- `/ARCHITECTURE.md`: module boundaries and long-term structure.
- `/PLANS.md`: execution plan template.
- `/docs/product-specs/`: product behavior contracts.
- `/docs/design-docs/`: design principles and engineering decisions.
- `/docs/exec-plans/`: active/completed implementation plans.
- `/docs/references/`: external references that influenced this setup.

## Commands
- `npm run validate:docs`: check required docs and project scaffold integrity.
- `npm run lint`: run ESLint for `src/**/*.ts(x)`.
- `npm run typecheck`: run TypeScript checks.
- `npm run build`: compile TypeScript output into `dist/`.
- `npm test`: strict gate (`validate:docs` + `lint` + `typecheck` + `coverage:strict`) then unit tests.
- `npm run player:dev -- <mode> ...`: run player from source via `tsx`.
- `npm run player:tui -- --scripts-dir <path> [--entry-script <name>]`: run interactive Ink TUI player from build output.
- `npm run player:agent -- <subcommand> ...`: run non-interactive agent mode from build output (`start` requires `--scripts-dir`, optional `--entry-script`).
- `npm run traverse:choices -- --examples-root examples/scripts`: traverse visible choice branches and ensure all explored paths reach `END`.

## Test Layout
- `test/unit/**`: unit tests mirrored to non-`.d.ts` files in `src/` with defensive edge-path coverage.
- `test/smoke/**`: integration smoke tests focused on runnable examples under `examples/scripts/`.

## Harness Workflow Notes
- Process authority lives in `/docs/HARNESS.md`.
- Use `/AGENTS.md` for startup checklist and `/docs/TEST_WORKFLOW.md` for test/coverage mechanics.

## Script Player

Build first:

```bash
npm run build
```

Play bundled examples (treated as ordinary script directories):

```bash
npm run player:tui -- --scripts-dir examples/scripts/06-snapshot-flow
npm run player:tui -- --scripts-dir examples/scripts/07-battle-duel
npm run player:tui -- --scripts-dir examples/scripts/08-json-globals
npm run player:tui -- --scripts-dir examples/scripts/09-random
npm run player:tui -- --scripts-dir examples/scripts/10-once-static
npm run player:tui -- --scripts-dir examples/scripts/11-choice-fallover-continue
npm run player:tui -- --scripts-dir examples/scripts/12-while-break-continue
npm run player:tui -- --scripts-dir examples/scripts/13-loop-times
npm run player:tui -- --scripts-dir examples/scripts/14-defs-functions
npm run player:tui -- --scripts-dir examples/scripts/15-entry-override-recursive --entry-script alt
npm run player:tui -- --scripts-dir examples/scripts/16-input-name
```

Play scripts from an external directory (default entry is `<script name="main">`; override with `--entry-script <name>` when needed. `--scripts-dir` is scanned recursively for `.script.xml` / `.defs.xml` / `.json` files):

```bash
npm run player:tui -- --scripts-dir /absolute/path/to/scripts
npm run player:tui -- --scripts-dir /absolute/path/to/scripts --entry-script alt
```

Run to boundary and persist state for agent orchestration:

```bash
npm run player:agent -- start --scripts-dir examples/scripts/06-snapshot-flow --state-out /tmp/sl-state.bin
npm run player:agent -- start --scripts-dir examples/scripts/15-entry-override-recursive --entry-script alt --state-out /tmp/sl-alt-state.bin
npm run player:agent -- choose --state-in /tmp/sl-state.bin --choice 0 --state-out /tmp/sl-next.bin
npm run player:agent -- input --state-in /tmp/sl-next.bin --text "Rin" --state-out /tmp/sl-next-2.bin
```

Agent mode can also start from an external scripts directory:

```bash
npm run player:agent -- start --scripts-dir /absolute/path/to/scripts --state-out /tmp/sl-state.bin
```

## Quick Start

```ts
import { createEngineFromXml } from "script-lang";

const engine = createEngineFromXml({
  entryScript: "main",
  compilerVersion: "dev",
  scriptsXml: {
    "gamestate.defs.xml": `
<defs name="gamestate">
  <type name="Actor">
    <field name="hp" type="number"/>
    <field name="name" type="string"/>
  </type>
</defs>
`,
    "main.script.xml": `
<!-- include: gamestate.defs.xml -->
<!-- include: game.json -->
<script name="main">
  <var name="hero" type="Actor" value="{ hp: 10, name: 'Rin' }"/>
  <text>\${game.title} HP \${hero.hp}</text>
  <choice text="Choose action">
    <option text="Heal"><code>hero.hp = hero.hp + 5;</code></option>
  </choice>
  <text>After \${hero.hp}</text>
</script>
`,
    "game.json": `{"title":"Demo"}`,
  },
});

const first = engine.next(); // text
const second = engine.next(); // choices
engine.choose(0);
const third = engine.next(); // text
```
