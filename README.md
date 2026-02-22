# ScriptLang

`ScriptLang` is an XML-first scripting language for branching narrative games with strong data simulation support through TypeScript code nodes.

This repository is initialized for **agent-first harness engineering**:
- specs are explicit,
- architecture boundaries are strict,
- execution is plan-driven,
- quality is measured by repeatable checks.

## Project Status
- Current phase: ScriptLang V1 core compiler/runtime available.
- Ongoing implementation tasks are tracked in `/docs/exec-plans/active/`.

## Repo Map
- `/AGENTS.md`: required workflow for engineers and coding agents.
- `/ARCHITECTURE.md`: module boundaries and long-term structure.
- `/PLANS.md`: execution plan template.
- `/docs/product-specs/`: product behavior contracts.
- `/docs/design-docs/`: design principles and engineering decisions.
- `/docs/exec-plans/`: active/completed implementation plans.
- `/docs/references/`: external references that influenced this setup.

## Commands
- `npm run validate:docs`: check required docs and project scaffold integrity.
- `npm run typecheck`: run TypeScript checks.
- `npm run build`: compile TypeScript output into `dist/`.
- `npm test`: docs validation + unit tests.
- `npm run player:dev -- <mode> ...`: run player from source via `tsx`.
- `npm run player:tui -- --example <id>`: run interactive Ink TUI player from build output.
- `npm run player:agent -- <subcommand> ...`: run non-interactive agent mode from build output.

## Script Player

Build first:

```bash
npm run build
```

Play in interactive mode:

```bash
npm run player:tui -- --example 06-snapshot-flow
```

List available examples in agent mode:

```bash
npm run player:agent -- list
```

Run to boundary and persist state for agent orchestration:

```bash
npm run player:agent -- start --example 06-snapshot-flow --state-out /tmp/sl-state.bin
npm run player:agent -- choose --state-in /tmp/sl-state.bin --choice 0 --state-out /tmp/sl-next.bin
```

## Quick Start

```ts
import { createEngineFromXml } from "script-lang";

const engine = createEngineFromXml({
  entryScript: "main",
  compilerVersion: "dev",
  scriptsXml: {
    "main.script.xml": `
<script name="main">
  <var name="hp" type="number" value="10"/>
  <text value="HP \${hp}"/>
  <choice>
    <option text="Heal"><code>hp = hp + 5;</code></option>
  </choice>
  <text value="After \${hp}"/>
</script>
`,
  },
});

const first = engine.next(); // text
const second = engine.next(); // choices
engine.choose(0);
const third = engine.next(); // text
```
