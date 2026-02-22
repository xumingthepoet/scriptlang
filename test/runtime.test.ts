import assert from "node:assert/strict";
import test from "node:test";

import { ScriptLangEngine, compileScript } from "../src";

test("next/choose and snapshot/resume roundtrip", () => {
  const main = compileScript(
    `
<script name="main.script.xml">
  <vars>
    <var name="hp" type="number" value="10"/>
  </vars>
  <step>
    <text value="HP \${hp}"/>
    <choice>
      <option text="Heal">
        <code>hp = hp + 5;</code>
      </option>
      <option text="Hit" once="true">
        <code>hp = hp - 3;</code>
      </option>
    </choice>
    <text value="After \${hp}"/>
  </step>
</script>
`,
    "main.script.xml"
  );

  const engine = new ScriptLangEngine({
    scripts: { "main.script.xml": main },
    compilerVersion: "dev",
  });
  engine.start("main.script.xml");

  const first = engine.next();
  assert.deepEqual(first, { kind: "text", text: "HP 10" });
  const choices = engine.next();
  assert.equal(choices.kind, "choices");
  assert.equal(engine.waitingChoice, true);

  const snap = engine.snapshot();
  const restored = new ScriptLangEngine({
    scripts: { "main.script.xml": main },
    compilerVersion: "dev",
  });
  restored.resume(snap);
  assert.equal(restored.waitingChoice, true);

  restored.choose(0);
  assert.equal(restored.next().kind, "text");
  assert.deepEqual(restored.next(), { kind: "end" });
});

test("call with ref writes back to caller var", () => {
  const main = compileScript(
    `
<script name="main.script.xml">
  <vars>
    <var name="hp" type="number" value="1"/>
  </vars>
  <step>
    <call script="buff.script.xml" args="amount:3,target:ref:hp"/>
    <text value="HP=\${hp}"/>
  </step>
</script>
`,
    "main.script.xml"
  );
  const buff = compileScript(
    `
<script name="buff.script.xml">
  <vars>
    <var name="amount" type="number" value="0"/>
    <var name="target" type="number" value="0"/>
  </vars>
  <step>
    <code>target = target + amount;</code>
    <return />
  </step>
</script>
`,
    "buff.script.xml"
  );

  const engine = new ScriptLangEngine({
    scripts: {
      "main.script.xml": main,
      "buff.script.xml": buff,
    },
    compilerVersion: "dev",
  });
  engine.start("main.script.xml");
  assert.deepEqual(engine.next(), { kind: "text", text: "HP=4" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("tail-position call compacts stack in waiting-choice snapshot", () => {
  const root = compileScript(
    `
<script name="root.script.xml">
  <vars/>
  <step>
    <call script="a.script.xml"/>
    <text value="done"/>
  </step>
</script>
`,
    "root.script.xml"
  );
  const a = compileScript(
    `
<script name="a.script.xml">
  <vars/>
  <step>
    <call script="b.script.xml"/>
  </step>
</script>
`,
    "a.script.xml"
  );
  const b = compileScript(
    `
<script name="b.script.xml">
  <vars/>
  <step>
    <choice>
      <option text="ok"><text value="B"/></option>
    </choice>
  </step>
</script>
`,
    "b.script.xml"
  );

  const engine = new ScriptLangEngine({
    scripts: {
      "root.script.xml": root,
      "a.script.xml": a,
      "b.script.xml": b,
    },
    compilerVersion: "dev",
  });
  engine.start("root.script.xml");
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
<script name="main.script.xml">
  <vars/>
  <step>
    <text value="x"/>
  </step>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { "main.script.xml": main },
    compilerVersion: "dev",
  });
  engine.start("main.script.xml");
  assert.throws(() => engine.snapshot());
});

test("default values for var types are initialized", () => {
  const main = compileScript(
    `
<script name="defaults.script.xml">
  <vars>
    <var name="s" type="string"/>
    <var name="b" type="boolean"/>
    <var name="n" type="null"/>
    <var name="arr" type="number[]"/>
    <var name="rec" type="Record&lt;string,number&gt;"/>
    <var name="m" type="Map&lt;string,number&gt;"/>
  </vars>
  <step>
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
  </step>
</script>
`,
    "defaults.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { "defaults.script.xml": main },
    compilerVersion: "dev",
  });
  engine.start("defaults.script.xml");
  assert.deepEqual(engine.next(), { kind: "text", text: "ok" });
});

test("type mismatch in code node fails fast", () => {
  const main = compileScript(
    `
<script name="main.script.xml">
  <vars>
    <var name="hp" type="number" value="1"/>
  </vars>
  <step>
    <code>hp = "bad";</code>
  </step>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { "main.script.xml": main },
    compilerVersion: "dev",
  });
  engine.start("main.script.xml");
  assert.throws(() => engine.next());
});
