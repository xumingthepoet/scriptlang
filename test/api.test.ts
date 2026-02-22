import assert from "node:assert/strict";
import { test } from "vitest";

import { compileScriptsFromXmlMap, createEngineFromXml, resumeEngineFromXml } from "../src/index.js";

test("createEngineFromXml and resumeEngineFromXml", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main">
  <var name="hp" type="number" value="2"/>
  <choice>
    <option text="up">
      <code>hp = hp + 1;</code>
    </option>
  </choice>
  <text value="HP \${hp}"/>
</script>
`,
  };

  const engine = createEngineFromXml({
    scriptsXml,
    entryScript: "main",
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

test("compileScriptsFromXmlMap returns compiled map keyed by script name", () => {
  const compiled = compileScriptsFromXmlMap({
    "a.script.xml": `<script name="a"><text value="a"/></script>`,
    "b.script.xml": `<script name="b"><text value="b"/></script>`,
  });
  assert.equal(Object.keys(compiled).length, 2);
  assert.ok(compiled.a);
  assert.ok(compiled.b);
});

test("compileScriptsFromXmlMap rejects duplicate script name", () => {
  assert.throws(() =>
    compileScriptsFromXmlMap({
      "a1.script.xml": `<script name="dup"><text value="a"/></script>`,
      "a2.script.xml": `<script name="dup"><text value="b"/></script>`,
    })
  );
});

test("compileScriptsFromXmlMap handles empty input", () => {
  const compiled = compileScriptsFromXmlMap({});
  assert.deepEqual(compiled, {});
});

test("createEngineFromXml works with default optional options", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `<script name="main"><text value="ok"/></script>`,
    },
    entryScript: "main",
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "ok" });
});

test("resumeEngineFromXml works with default optional options", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main">
  <choice>
    <option text="ok"><text value="done"/></option>
  </choice>
</script>
`,
  };
  const engine = createEngineFromXml({
    scriptsXml,
    entryScript: "main",
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
        "main.script.xml": `<script name="main"><text value="x"/></script>`,
      },
      entryScript: "missing",
    })
  );

  const scriptsXml = {
    "main.script.xml": `<script name="main"><choice><option text="x"><text value="x"/></option></choice></script>`,
  };
  const engine = createEngineFromXml({
    scriptsXml,
    entryScript: "main",
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
