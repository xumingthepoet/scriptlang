import assert from "node:assert/strict";
import { test } from "vitest";

import {
  ScriptLangEngine,
  ScriptLangError,
  compileScript,
  createEngineFromXml,
} from "../src/index.js";

const expectCode = (fn: () => unknown, code: string): void => {
  assert.throws(fn, (error: unknown) => {
    assert.ok(error instanceof ScriptLangError);
    assert.equal(error.code, code);
    return true;
  });
};

test("next/choose and snapshot/resume roundtrip", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="hp" type="number" value="10"/>
  <text>HP \${hp}</text>
  <choice>
    <option text="Heal">
      <code>hp = hp + 5;</code>
    </option>
    <option text="Hit">
      <code>hp = hp - 3;</code>
    </option>
  </choice>
  <text>After \${hp}</text>
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
        <text>pick-\${round}</text>
      </option>
      <option text="Pick B">
        <code>round = round + 1;</code>
        <text>skip-\${round}</text>
      </option>
    </choice>
  </while>
  <text>done</text>
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
  <call script="buff" args="3,ref:hp"/>
  <text>HP=\${hp}</text>
</script>
`,
    "main.script.xml"
  );
  const buff = compileScript(
    `
<script name="buff" args="number:amount,ref:number:target">
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

test("return transfer supports positional value args", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="hp" type="number" value="2"/>
  <return script="next" args="hp + 3"/>
</script>
`,
    "main.script.xml"
  );
  const next = compileScript(
    `
<script name="next" args="number:value">
  <text>v=\${value}</text>
</script>
`,
    "next.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { main, next },
    compilerVersion: "dev",
  });
  engine.start("main");
  assert.deepEqual(engine.next(), { kind: "text", text: "v=5" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("return transfer rejects ref args at compile time", () => {
  expectCode(
    () =>
      compileScript(
        `
<script name="main">
  <var name="hp" type="number" value="2"/>
  <return script="next" args="ref:hp"/>
</script>
`,
        "main.script.xml"
      ),
    "XML_RETURN_REF_UNSUPPORTED"
  );
});

test("return transfer flushes inherited ref writes before switching script", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="hp" type="number" value="1"/>
  <call script="mid" args="ref:hp"/>
  <text>hp=\${hp}</text>
</script>
`,
    "main.script.xml"
  );
  const mid = compileScript(
    `
<script name="mid" args="ref:number:x">
  <code>x = x + 1;</code>
  <return script="tail" args="x + 1"/>
</script>
`,
    "mid.script.xml"
  );
  const tail = compileScript(
    `
<script name="tail" args="number:y">
  <text>tail=\${y}</text>
  <return/>
</script>
`,
    "tail.script.xml"
  );

  const engine = new ScriptLangEngine({
    scripts: { main, mid, tail },
    compilerVersion: "dev",
  });
  engine.start("main");
  assert.deepEqual(engine.next(), { kind: "text", text: "tail=3" });
  assert.deepEqual(engine.next(), { kind: "text", text: "hp=2" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("tail-position call compacts stack in waiting-choice snapshot", () => {
  const root = compileScript(
    `
<script name="root">
  <call script="a"/>
  <text>done</text>
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
    <option text="ok"><text>B</text></option>
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
  <text>x</text>
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
<script name="defaults" args="string:s,boolean:b,number[]:arr,Map&lt;string,number&gt;:m">
  <code>
    if (s !== "") throw new Error("s");
    if (b !== false) throw new Error("b");
    if (!Array.isArray(arr) || arr.length !== 0) throw new Error("arr");
    if (!m || typeof m !== "object" || !("size" in m) || Number(m.size) !== 0) {
      throw new Error("m");
    }
  </code>
  <text>ok</text>
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

test("null values are rejected for declared variables", () => {
  const numberScript = compileScript(
    `
<script name="number-null">
  <var name="hp" type="number" value="1"/>
  <code>hp = null;</code>
</script>
`,
    "number-null.script.xml"
  );
  const arrayScript = compileScript(
    `
<script name="array-null">
  <var name="values" type="number[]" value="[1,2]"/>
  <code>values = [1, null];</code>
</script>
`,
    "array-null.script.xml"
  );
  const mapScript = compileScript(
    `
<script name="map-null">
  <var name="scores" type="Map&lt;string,number&gt;" value="new Map([['a', 1]])"/>
  <code>scores = new Map([['a', null]]);</code>
</script>
`,
    "map-null.script.xml"
  );

  const numberEngine = new ScriptLangEngine({
    scripts: { "number-null": numberScript },
    compilerVersion: "dev",
  });
  numberEngine.start("number-null");
  expectCode(() => numberEngine.next(), "ENGINE_TYPE_MISMATCH");

  const arrayEngine = new ScriptLangEngine({
    scripts: { "array-null": arrayScript },
    compilerVersion: "dev",
  });
  arrayEngine.start("array-null");
  expectCode(() => arrayEngine.next(), "ENGINE_TYPE_MISMATCH");

  const mapEngine = new ScriptLangEngine({
    scripts: { "map-null": mapScript },
    compilerVersion: "dev",
  });
  mapEngine.start("map-null");
  expectCode(() => mapEngine.next(), "ENGINE_TYPE_MISMATCH");
});

test("resume rejects snapshot frames with legacy null primitive metadata", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="hp" type="number" value="1"/>
  <choice>
    <option text="ok"><text>x</text></option>
  </choice>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  engine.start("main");
  const out = engine.next();
  assert.equal(out.kind, "choices");
  const snapshot = engine.snapshot();
  const primitiveNull = JSON.parse('{"kind":"primitive","name":"null"}');
  const arrayOfNull = JSON.parse('{"kind":"array","elementType":{"kind":"primitive","name":"null"}}');
  const mapOfNull = JSON.parse('{"kind":"map","keyType":"string","valueType":{"kind":"primitive","name":"null"}}');

  const primitiveMutated = structuredClone(snapshot);
  primitiveMutated.runtimeFrames[0].varTypes = { hp: primitiveNull };
  const primitiveRestored = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  expectCode(() => primitiveRestored.resume(primitiveMutated), "SNAPSHOT_TYPE_UNSUPPORTED");

  const arrayMutated = structuredClone(snapshot);
  arrayMutated.runtimeFrames[0].varTypes = { hp: arrayOfNull };
  const arrayRestored = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  expectCode(() => arrayRestored.resume(arrayMutated), "SNAPSHOT_TYPE_UNSUPPORTED");

  const mapMutated = structuredClone(snapshot);
  mapMutated.runtimeFrames[0].varTypes = { hp: mapOfNull };
  const mapRestored = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  expectCode(() => mapRestored.resume(mapMutated), "SNAPSHOT_TYPE_UNSUPPORTED");
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
  <text>hp=\${hp}</text>
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

test("custom object types default recursively and enforce strict fields", () => {
  const typesXml = `
<types name="gamestate">
  <type name="Actor">
    <field name="hp" type="number"/>
    <field name="name" type="string"/>
  </type>
  <type name="BattleState">
    <field name="player" type="Actor"/>
    <field name="enemy" type="Actor"/>
  </type>
</types>
`;

  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: gamestate.types.xml -->
<script name="main">
  <var name="state" type="BattleState"/>
  <text>\${state.player.hp}:\${state.enemy.hp}</text>
</script>
`,
      "gamestate.types.xml": typesXml,
    },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "0:0" });

  const missingFieldEngine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: gamestate.types.xml -->
<script name="main">
  <var name="state" type="BattleState" value="{ player: { hp: 1, name: 'a' } }"/>
  <text>bad</text>
</script>
`,
      "gamestate.types.xml": typesXml,
    },
  });
  expectCode(() => missingFieldEngine.next(), "ENGINE_TYPE_MISMATCH");

  const extraFieldEngine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: gamestate.types.xml -->
<script name="main">
  <var name="state" type="BattleState" value="{ player: { hp: 1, name: 'a', extra: 1 }, enemy: { hp: 2, name: 'b' } }"/>
  <text>bad</text>
</script>
`,
      "gamestate.types.xml": typesXml,
    },
  });
  expectCode(() => extraFieldEngine.next(), "ENGINE_TYPE_MISMATCH");
});

test("custom object fields support nested array and map typing", () => {
  const ok = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: complex.types.xml -->
<script name="main">
  <var name="bag" type="Complex" value="{ items: [1,2], scores: new Map([['a', 1]]) }"/>
  <text>ok</text>
</script>
`,
      "complex.types.xml": `
<types name="complex">
  <type name="Complex">
    <field name="items" type="number[]"/>
    <field name="scores" type="Map&lt;string,number&gt;"/>
  </type>
</types>
`,
    },
  });
  assert.deepEqual(ok.next(), { kind: "text", text: "ok" });

  const badNested = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: complex.types.xml -->
<script name="main">
  <var name="bag" type="Complex" value="{ items: [1], scores: new Map([['a', 'bad']]) }"/>
  <text>x</text>
</script>
`,
      "complex.types.xml": `
<types name="complex">
  <type name="Complex">
    <field name="items" type="number[]"/>
    <field name="scores" type="Map&lt;string,number&gt;"/>
  </type>
</types>
`,
    },
  });
  expectCode(() => badNested.next(), "ENGINE_TYPE_MISMATCH");
});

test("object type rejects null/array and missing expected key with same key count", () => {
  const typesXml = `
<types name="pair">
  <type name="Pair">
    <field name="a" type="number"/>
    <field name="b" type="number"/>
  </type>
</types>
`;

  const nullValue = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: pair.types.xml -->
<script name="main">
  <var name="p" type="Pair" value="null"/>
</script>
`,
      "pair.types.xml": typesXml,
    },
  });
  expectCode(() => nullValue.next(), "ENGINE_TYPE_MISMATCH");

  const arrayValue = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: pair.types.xml -->
<script name="main">
  <var name="p" type="Pair" value="[1,2]"/>
</script>
`,
      "pair.types.xml": typesXml,
    },
  });
  expectCode(() => arrayValue.next(), "ENGINE_TYPE_MISMATCH");

  const wrongKeySet = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: pair.types.xml -->
<script name="main">
  <var name="p" type="Pair" value="{ a: 1, c: 2 }"/>
</script>
`,
      "pair.types.xml": typesXml,
    },
  });
  expectCode(() => wrongKeySet.next(), "ENGINE_TYPE_MISMATCH");
});

test("resume rejects snapshot object varTypes with nested unsupported primitive", () => {
  const typesXml = `
<types name="holder">
  <type name="Holder">
    <field name="hp" type="number"/>
  </type>
</types>
`;
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: holder.types.xml -->
<script name="main">
  <var name="h" type="Holder"/>
  <choice>
    <option text="ok"><text>x</text></option>
  </choice>
</script>
`,
      "holder.types.xml": typesXml,
    },
  });
  const out = engine.next();
  assert.equal(out.kind, "choices");
  const snapshot = engine.snapshot();
  const objectWithNull = JSON.parse(
    '{"kind":"object","typeName":"Holder","fields":{"hp":{"kind":"primitive","name":"null"}}}'
  );
  const mutated = structuredClone(snapshot);
  mutated.runtimeFrames[0].varTypes = { h: objectWithNull };

  const restored = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: holder.types.xml -->
<script name="main">
  <var name="h" type="Holder"/>
  <choice>
    <option text="ok"><text>x</text></option>
  </choice>
</script>
`,
      "holder.types.xml": typesXml,
    },
  });
  expectCode(() => restored.resume(mutated), "SNAPSHOT_TYPE_UNSUPPORTED");
});
