import assert from "node:assert/strict";
import test from "node:test";

import { createEngineFromXml, resumeEngineFromXml } from "../src";

test("createEngineFromXml and resumeEngineFromXml", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main.script.xml">
  <vars>
    <var name="hp" type="number" value="2"/>
  </vars>
  <step>
    <choice>
      <option text="up">
        <code>hp = hp + 1;</code>
      </option>
    </choice>
    <text value="HP \${hp}"/>
  </step>
</script>
`,
  };

  const engine = createEngineFromXml({
    scriptsXml,
    entryScript: "main.script.xml",
    compilerVersion: "dev",
  });
  const first = engine.next();
  assert.equal(first.kind, "choices");
  const snap = engine.snapshot();

  const resumed = resumeEngineFromXml({
    scriptsXml,
    snapshot: snap,
    compilerVersion: "dev",
  });
  resumed.choose(0);
  assert.deepEqual(resumed.next(), { kind: "text", text: "HP 3" });
});

