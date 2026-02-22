import assert from "node:assert/strict";
import { test } from "vitest";

import { compileScriptsFromXmlMap, createEngineFromXml, resumeEngineFromXml } from "../src/index.js";

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

test("compileScriptsFromXmlMap returns compiled map", () => {
  const compiled = compileScriptsFromXmlMap({
    "a.script.xml": `<script name="a.script.xml"><vars/><step><text value="a"/></step></script>`,
    "b.script.xml": `<script name="b.script.xml"><vars/><step><text value="b"/></step></script>`,
  });
  assert.equal(Object.keys(compiled).length, 2);
  assert.ok(compiled["a.script.xml"]);
  assert.ok(compiled["b.script.xml"]);
});

test("compileScriptsFromXmlMap handles empty input", () => {
  const compiled = compileScriptsFromXmlMap({});
  assert.deepEqual(compiled, {});
});

test("createEngineFromXml works with default optional options", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `<script name="main.script.xml"><vars/><step><text value="ok"/></step></script>`,
    },
    entryScript: "main.script.xml",
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "ok" });
});

test("resumeEngineFromXml works with default optional options", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main.script.xml">
  <vars/>
  <step>
    <choice>
      <option text="ok"><text value="done"/></option>
    </choice>
  </step>
</script>
`,
  };
  const engine = createEngineFromXml({
    scriptsXml,
    entryScript: "main.script.xml",
  });
  const out = engine.next();
  assert.equal(out.kind, "choices");
  const resumed = resumeEngineFromXml({
    scriptsXml,
    snapshot: engine.snapshot(),
  });
  assert.equal(resumed.waitingChoice, true);
});

test("api create/resume error paths", () => {
  assert.throws(() =>
    createEngineFromXml({
      scriptsXml: {
        "main.script.xml": `<script name="main.script.xml"><vars/><step><text value="x"/></step></script>`,
      },
      entryScript: "missing.script.xml",
    })
  );

  const scriptsXml = {
    "main.script.xml": `<script name="main.script.xml"><vars/><step><choice><option text="x"><text value="x"/></option></choice></step></script>`,
  };
  const engine = createEngineFromXml({
    scriptsXml,
    entryScript: "main.script.xml",
    compilerVersion: "dev",
  });
  engine.next();
  assert.throws(() =>
    resumeEngineFromXml({
      scriptsXml,
      snapshot: engine.snapshot(),
      compilerVersion: "not-dev",
    })
  );
});
