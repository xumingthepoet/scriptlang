import assert from "node:assert/strict";
import { test } from "vitest";

import {
  compileProjectFromXmlMap,
  compileScriptsFromXmlMap,
  createEngineFromXml,
  resumeEngineFromXml,
} from "../../src/index.js";

test("createEngineFromXml and resumeEngineFromXml", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main">
  <var name="hp" type="number" value="2"/>
  <choice text="Choose">
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

test("compileScriptsFromXmlMap compiles all provided scripts", () => {
  const compiled = compileScriptsFromXmlMap({
    "main.script.xml": `<!-- include: a.script.xml -->
<script name="main"><text>m</text></script>`,
    "a.script.xml": `<script name="a"><text>a</text></script>`,
    "b.script.xml": `<script name="b"><text>b</text></script>`,
  });
  assert.equal(Object.keys(compiled).length, 3);
  assert.ok(compiled.main);
  assert.ok(compiled.a);
  assert.ok(compiled.b);
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
  <text>\${random(100)}</text>
  <text>\${random(100)}</text>
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
  <choice text="Choose">
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

test("api choices output exposes required choice prompt text", () => {
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
    "main.script.xml": `<script name="main"><choice text="Choose"><option text="x"><text>x</text></option></choice></script>`,
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
<!-- include: gamestate.defs.xml -->
<!-- include: game.json -->
<script name="main" args="BattleState:state">
  <text>p=\${state.player.hp},e=\${state.enemy.hp},name=\${game.player.name}</text>
</script>
`,
    "gamestate.defs.xml": `
<defs name="gamestate">
  <type name="Actor">
    <field name="hp" type="number"/>
    <field name="label" type="string"/>
  </type>
  <type name="BattleState">
    <field name="player" type="Actor"/>
    <field name="enemy" type="Actor"/>
  </type>
</defs>
`,
    "unused.defs.xml": `
<defs name="unused">
  <type name="Ghost">
    <field name="hp" type="number"/>
  </type>
</defs>
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
<!-- include: missing.defs.xml -->
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
<!-- include: types-a.defs.xml -->
<!-- include: types-b.defs.xml -->
<script name="main"><text>x</text></script>
`,
      "types-a.defs.xml": `
<defs name="a"><type name="Dup"><field name="n" type="number"/></type></defs>
`,
      "types-b.defs.xml": `
<defs name="b"><type name="Dup"><field name="s" type="string"/></type></defs>
`,
    })
  );
});

test("project compile validates non-main scripts as well", () => {
  assert.throws(
    () =>
      compileScriptsFromXmlMap({
        "main.script.xml": `<script name="main"><text>m</text></script>`,
        "unused.script.xml": `<script name="unused"><var name="hp" type="Missing"/></script>`,
      }),
    (e: unknown) => {
      assert.ok(e instanceof Error);
      assert.equal((e as { code?: string }).code, "TYPE_UNKNOWN");
      return true;
    }
  );
});

test("project compile includes all scripts regardless of main include closure", () => {
  const compiled = compileScriptsFromXmlMap({
    "main.script.xml": `<!-- include: linked.script.xml -->
<script name="main"><text>m</text></script>`,
    "linked.script.xml": `<script name="linked"><text>l</text></script>`,
    "unused.script.xml": `<script name="unused"><text>u</text></script>`,
  });
  assert.ok(compiled.main);
  assert.ok(compiled.linked);
  assert.ok(compiled.unused);
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

test("createEngineFromXml supports explicit non-main entry without main", () => {
  const engine = createEngineFromXml({
    entryScript: "alt",
    scriptsXml: {
      "alt.script.xml": `<script name="alt"><text>alt-ok</text></script>`,
    },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "alt-ok" });
});

test("createEngineFromXml explicit missing entry returns API_ENTRY_SCRIPT_NOT_FOUND", () => {
  assert.throws(
    () =>
      createEngineFromXml({
        entryScript: "alt",
        scriptsXml: {
          "main.script.xml": `<script name="main"><text>x</text></script>`,
        },
      }),
    (e: unknown) => {
      assert.ok(e instanceof Error);
      assert.equal((e as { code?: string }).code, "API_ENTRY_SCRIPT_NOT_FOUND");
      return true;
    }
  );
});

test("createEngineFromXml forwards entryArgs and validates entry arg paths", () => {
  const scriptsXml = {
    "alt.script.xml": `<script name="alt" args="number:hp,string:name"><text>\${name}:\${hp}</text></script>`,
  };
  const engine = createEngineFromXml({
    scriptsXml,
    entryScript: "alt",
    entryArgs: { hp: 7, name: "Rin" },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "Rin:7" });

  assert.throws(
    () =>
      createEngineFromXml({
        scriptsXml,
        entryScript: "alt",
        entryArgs: { hp: "7", name: "Rin" },
      }),
    (e: unknown) => {
      assert.ok(e instanceof Error);
      assert.equal((e as { code?: string }).code, "ENGINE_TYPE_MISMATCH");
      return true;
    }
  );

  assert.throws(
    () =>
      createEngineFromXml({
        scriptsXml,
        entryScript: "alt",
        entryArgs: { hp: 7, name: "Rin", extra: 1 },
      }),
    (e: unknown) => {
      assert.ok(e instanceof Error);
      assert.equal((e as { code?: string }).code, "ENGINE_CALL_ARG_UNKNOWN");
      return true;
    }
  );
});

test("api createEngineFromXml supports host functions", () => {
  const engine = createEngineFromXml({
    entryScript: "main",
    compilerVersion: "dev",
    hostFunctions: {
      add: (...args: unknown[]) => Number(args[0]) + Number(args[1]),
    },
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <var name="hp" type="number" value="1"/>
  <code>hp = add(hp, 2);</code>
  <text>v=\${hp}</text>
</script>
`,
    },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "v=3" });
});
