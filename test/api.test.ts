import assert from "node:assert/strict";
import { test } from "vitest";

import {
  compileProjectFromXmlMap,
  compileScriptsFromXmlMap,
  createEngineFromXml,
  resumeEngineFromXml,
} from "../src/index.js";

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
  <text>HP \${hp}</text>
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
    "main.script.xml": `<!-- include: a.script.xml -->
<script name="main"><text>m</text></script>`,
    "a.script.xml": `<script name="a"><text>a</text></script>`,
    "b.script.xml": `<script name="b"><text>b</text></script>`,
  });
  assert.equal(Object.keys(compiled).length, 2);
  assert.ok(compiled.main);
  assert.ok(compiled.a);
  assert.equal(compiled.b, undefined);
});

test("compileScriptsFromXmlMap rejects duplicate script name", () => {
  assert.throws(() =>
    compileScriptsFromXmlMap({
      "main.script.xml": `<!-- include: a1.script.xml -->
<!-- include: a2.script.xml -->
<script name="main"><text>m</text></script>`,
      "a1.script.xml": `<script name="dup"><text>a</text></script>`,
      "a2.script.xml": `<script name="dup"><text>b</text></script>`,
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
      "main.script.xml": `<script name="main"><text>ok</text></script>`,
    },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "ok" });
});

test("createEngineFromXml forwards randomSeed to builtin random", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main">
  <text>\${random()}</text>
  <text>\${random()}</text>
</script>
`,
  };

  const first = createEngineFromXml({
    scriptsXml,
    randomSeed: 42,
  });
  const second = createEngineFromXml({
    scriptsXml,
    randomSeed: 42,
  });

  const firstA = first.next();
  const firstB = first.next();
  const secondA = second.next();
  const secondB = second.next();

  assert.equal(firstA.kind, "text");
  assert.equal(firstB.kind, "text");
  assert.equal(secondA.kind, "text");
  assert.equal(secondB.kind, "text");
  assert.equal(firstA.text, secondA.text);
  assert.equal(firstB.text, secondB.text);
});

test("resumeEngineFromXml works with default optional options", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main">
  <choice>
    <option text="ok"><text>done</text></option>
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

test("api choices output exposes optional prompt text", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <choice text="Pick one">
    <option text="ok"><text>done</text></option>
  </choice>
</script>
`,
    },
    entryScript: "main",
  });
  const out = engine.next();
  assert.equal(out.kind, "choices");
  assert.equal(out.promptText, "Pick one");
});

test("api create/resume error paths", () => {
  assert.throws(() =>
    createEngineFromXml({
      scriptsXml: {
        "main.script.xml": `<script name="main"><text>x</text></script>`,
      },
      entryScript: "missing",
    })
  );

  const scriptsXml = {
    "main.script.xml": `<script name="main"><choice><option text="x"><text>x</text></option></choice></script>`,
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

test("compile project supports include graph and global custom types", () => {
  const xmlByPath = {
    "main.script.xml": `
<!-- include: gamestate.types.xml -->
<!-- include: game.json -->
<script name="main" args="BattleState:state">
  <text>p=\${state.player.hp},e=\${state.enemy.hp},name=\${game.player.name}</text>
</script>
`,
    "gamestate.types.xml": `
<types name="gamestate">
  <type name="Actor">
    <field name="hp" type="number"/>
    <field name="label" type="string"/>
  </type>
  <type name="BattleState">
    <field name="player" type="Actor"/>
    <field name="enemy" type="Actor"/>
  </type>
</types>
`,
    "unused.types.xml": `
<types name="unused">
  <type name="Ghost">
    <field name="hp" type="number"/>
  </type>
</types>
`,
    "game.json": `{"player":{"name":"Hero"}}`,
  };

  const project = compileProjectFromXmlMap({ xmlByPath });
  assert.equal(project.entryScript, "main");
  assert.ok(project.scripts.main);
  assert.equal(
    (
      project.globalJson.game as {
        player: { name: string };
      }
    ).player.name,
    "Hero"
  );

  const engine = createEngineFromXml({ scriptsXml: xmlByPath });
  assert.deepEqual(engine.next(), { kind: "text", text: "p=0,e=0,name=Hero" });
});

test("project include and type errors are surfaced", () => {
  assert.throws(() =>
    compileScriptsFromXmlMap({
      "main.script.xml": `
<!-- include: missing.types.xml -->
<script name="main"><text>x</text></script>
`,
    })
  );

  assert.throws(() =>
    compileScriptsFromXmlMap({
      "main.script.xml": `
<!-- include: extra.script.xml -->
<script name="main"><text>x</text></script>
`,
      "extra.script.xml": `
<!-- include: main.script.xml -->
<script name="extra"><text>y</text></script>
`,
    })
  );

  assert.throws(() =>
    compileScriptsFromXmlMap({
      "main.script.xml": `
<!-- include: types-a.types.xml -->
<!-- include: types-b.types.xml -->
<script name="main"><text>x</text></script>
`,
      "types-a.types.xml": `
<types name="a"><type name="Dup"><field name="n" type="number"/></type></types>
`,
      "types-b.types.xml": `
<types name="b"><type name="Dup"><field name="s" type="string"/></type></types>
`,
    })
  );
});

test("project compile only includes files reachable from main include closure", () => {
  const compiled = compileScriptsFromXmlMap({
    "main.script.xml": `<!-- include: linked.script.xml -->
<script name="main"><text>m</text></script>`,
    "linked.script.xml": `<script name="linked"><text>l</text></script>`,
    "unused.script.xml": `<script name="unused"><text>u</text></script>`,
  });
  assert.ok(compiled.main);
  assert.ok(compiled.linked);
  assert.equal(compiled.unused, undefined);
});

test("createEngineFromXml requires main script to exist", () => {
  assert.throws(
    () =>
      createEngineFromXml({
        scriptsXml: {
          "other.script.xml": `<script name="other"><text>x</text></script>`,
        },
      }),
    (e: unknown) => {
      assert.ok(e instanceof Error);
      assert.equal((e as { code?: string }).code, "API_ENTRY_MAIN_NOT_FOUND");
      return true;
    }
  );
});
