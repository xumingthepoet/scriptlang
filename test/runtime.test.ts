import assert from "node:assert/strict";
import { test } from "vitest";

import { ScriptLangEngine, compileScript } from "../src/index.js";

test("next/choose and snapshot/resume roundtrip", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="hp" type="number" value="10"/>
  <text value="HP \${hp}"/>
  <choice>
    <option text="Heal">
      <code>hp = hp + 5;</code>
    </option>
    <option text="Hit">
      <code>hp = hp - 3;</code>
    </option>
  </choice>
  <text value="After \${hp}"/>
</script>
`,
    "main.script.xml"
  );

  const engine = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  engine.start("main");

  const first = engine.next();
  assert.deepEqual(first, { kind: "text", text: "HP 10" });
  const choices = engine.next();
  assert.equal(choices.kind, "choices");
  assert.equal(engine.waitingChoice, true);

  const snap = engine.snapshot();
  const restored = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  restored.resume(snap);
  assert.equal(restored.waitingChoice, true);

  restored.choose(0);
  assert.equal(restored.next().kind, "text");
  assert.deepEqual(restored.next(), { kind: "end" });
});

test("choice options remain visible on re-entry and after resume", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="round" type="number" value="0"/>
  <while when="round &lt; 2">
    <choice>
      <option text="Pick A">
        <code>round = round + 1;</code>
        <text value="pick-\${round}"/>
      </option>
      <option text="Pick B">
        <code>round = round + 1;</code>
        <text value="skip-\${round}"/>
      </option>
    </choice>
  </while>
  <text value="done"/>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  engine.start("main");

  const firstChoices = engine.next();
  assert.equal(firstChoices.kind, "choices");
  assert.deepEqual(
    firstChoices.items.map((item) => item.text),
    ["Pick A", "Pick B"]
  );

  engine.choose(0);
  assert.deepEqual(engine.next(), { kind: "text", text: "pick-1" });

  const secondChoices = engine.next();
  assert.equal(secondChoices.kind, "choices");
  assert.deepEqual(
    secondChoices.items.map((item) => item.text),
    ["Pick A", "Pick B"]
  );

  const snap = engine.snapshot();
  const restored = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  restored.resume(snap);
  const resumedChoices = restored.next();
  assert.equal(resumedChoices.kind, "choices");
  assert.deepEqual(
    resumedChoices.items.map((item) => item.text),
    ["Pick A", "Pick B"]
  );

  restored.choose(0);
  assert.deepEqual(restored.next(), { kind: "text", text: "pick-2" });
  assert.deepEqual(restored.next(), { kind: "text", text: "done" });
  assert.deepEqual(restored.next(), { kind: "end" });
});

test("call with ref writes back to caller var", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="hp" type="number" value="1"/>
  <call script="buff" args="amount:3,target:ref:hp"/>
  <text value="HP=\${hp}"/>
</script>
`,
    "main.script.xml"
  );
  const buff = compileScript(
    `
<script name="buff" args="amount:number,target:number:ref">
  <code>target = target + amount;</code>
  <return />
</script>
`,
    "buff.script.xml"
  );

  const engine = new ScriptLangEngine({
    scripts: { main, buff },
    compilerVersion: "dev",
  });
  engine.start("main");
  assert.deepEqual(engine.next(), { kind: "text", text: "HP=4" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("tail-position call compacts stack in waiting-choice snapshot", () => {
  const root = compileScript(
    `
<script name="root">
  <call script="a"/>
  <text value="done"/>
</script>
`,
    "root.script.xml"
  );
  const a = compileScript(
    `
<script name="a">
  <call script="b"/>
</script>
`,
    "a.script.xml"
  );
  const b = compileScript(
    `
<script name="b">
  <choice>
    <option text="ok"><text value="B"/></option>
  </choice>
</script>
`,
    "b.script.xml"
  );

  const engine = new ScriptLangEngine({
    scripts: { root, a, b },
    compilerVersion: "dev",
  });
  engine.start("root");
  const out = engine.next();
  assert.equal(out.kind, "choices");
  const snap = engine.snapshot();
  const groupPath = snap.cursor.groupPath.join(">");
  assert.equal(groupPath.includes("a.script.xml"), false);
  assert.equal(groupPath.includes("b.script.xml"), true);
});

test("snapshot is rejected when not waiting choice", () => {
  const main = compileScript(
    `
<script name="main">
  <text value="x"/>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  engine.start("main");
  assert.throws(() => engine.snapshot());
});

test("default values for arg types are initialized", () => {
  const main = compileScript(
    `
<script name="defaults" args="s:string,b:boolean,n:null,arr:number[],rec:Record&lt;string,number&gt;,m:Map&lt;string,number&gt;">
  <code>
    if (s !== "") throw new Error("s");
    if (b !== false) throw new Error("b");
    if (n !== null) throw new Error("n");
    if (!Array.isArray(arr) || arr.length !== 0) throw new Error("arr");
    if (Object.keys(rec).length !== 0) throw new Error("rec");
    if (!m || typeof m !== "object" || !("size" in m) || Number(m.size) !== 0) {
      throw new Error("m");
    }
  </code>
  <text value="ok"/>
</script>
`,
    "defaults.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { defaults: main },
    compilerVersion: "dev",
  });
  engine.start("defaults");
  assert.deepEqual(engine.next(), { kind: "text", text: "ok" });
});

test("type mismatch in code node fails fast", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="hp" type="number" value="1"/>
  <code>hp = "bad";</code>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  engine.start("main");
  assert.throws(() => engine.next());
});

test("var declaration without value uses type default", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="hp" type="number"/>
  <text value="hp=\${hp}"/>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  engine.start("main");
  assert.deepEqual(engine.next(), { kind: "text", text: "hp=0" });
});
