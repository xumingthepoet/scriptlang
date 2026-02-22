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
- `npm test`: docs validation + unit tests.

## Quick Start

```ts
import { createEngineFromXml } from "script-lang";

const engine = createEngineFromXml({
  entryScript: "main.script.xml",
  compilerVersion: "dev",
  scriptsXml: {
    "main.script.xml": `
<script name="main.script.xml">
  <vars>
    <var name="hp" type="number" value="10"/>
  </vars>
  <step>
    <text value="HP \${hp}"/>
    <choice>
      <option text="Heal"><code>hp = hp + 5;</code></option>
    </choice>
    <text value="After \${hp}"/>
  </step>
</script>
`,
  },
});

const first = engine.next(); // text
const second = engine.next(); // choices
engine.choose(0);
const third = engine.next(); // text
```
