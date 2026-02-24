# ScriptLang

`ScriptLang` is an XML-first scripting language for branching narrative games with strong data simulation support through TypeScript code nodes.

This repository is initialized for **agent-first harness engineering**:
- specs are explicit,
- architecture boundaries are strict,
- execution is plan-driven,
- quality is measured by repeatable checks.

## Project Status
- Current phase: active development with V2 XML syntax and V1 snapshot schema (`snapshot.v1`).
- Compatibility posture: no backward-compat requirement by default during development; remove legacy syntax/behavior unless a task explicitly requires compatibility.
- Ongoing implementation tasks are tracked in `/docs/exec-plans/active/`.

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
- `npm run player:tui -- --scripts-dir <path>`: run interactive Ink TUI player from build output.
- `npm run player:agent -- <subcommand> ...`: run non-interactive agent mode from build output (`start` requires `--scripts-dir`).

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
```

Play scripts from an external directory (entry is always `<script name="main">`; multi-file dependencies including `.script.xml` / `.types.xml` / `.json` data files must be included from `main` via header `include`):

```bash
npm run player:tui -- --scripts-dir /absolute/path/to/scripts
```

Run to boundary and persist state for agent orchestration:

```bash
npm run player:agent -- start --scripts-dir examples/scripts/06-snapshot-flow --state-out /tmp/sl-state.bin
npm run player:agent -- choose --state-in /tmp/sl-state.bin --choice 0 --state-out /tmp/sl-next.bin
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
    "gamestate.types.xml": `
<types name="gamestate">
  <type name="Actor">
    <field name="hp" type="number"/>
    <field name="name" type="string"/>
  </type>
</types>
`,
    "main.script.xml": `
<!-- include: gamestate.types.xml -->
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
