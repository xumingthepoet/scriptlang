import assert from "node:assert/strict";
import { test } from "vitest";

import {
  ScriptLangEngine,
  ScriptLangError,
  compileScript,
  createEngineFromXml,
  resumeEngineFromXml,
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
  <choice text="Choose">
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
    <choice text="Choose">
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

test("choice prompt text is host-facing and not emitted as text output", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <text>before</text>
  <choice text="pick now">
    <option text="A"><text>after</text></option>
  </choice>
</script>
`,
    },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "before" });
  const choices = engine.next();
  assert.equal(choices.kind, "choices");
  assert.equal(choices.promptText, "pick now");

  engine.choose(0);
  assert.deepEqual(engine.next(), { kind: "text", text: "after" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("text once is emitted only once and survives restart", () => {
  const main = compileScript(
    `
<script name="main">
  <text once="true">intro</text>
  <text>always</text>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  engine.start("main");
  assert.deepEqual(engine.next(), { kind: "text", text: "intro" });
  assert.deepEqual(engine.next(), { kind: "text", text: "always" });
  assert.deepEqual(engine.next(), { kind: "end" });

  engine.start("main");
  assert.deepEqual(engine.next(), { kind: "text", text: "always" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("choice option once is hidden after selection and preserved across resume", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="round" type="number" value="0"/>
  <while when="round &lt; 2">
    <choice text="Pick">
      <option text="A" once="true">
        <code>round = round + 1;</code>
        <text>a-\${round}</text>
      </option>
      <option text="B">
        <code>round = round + 1;</code>
        <text>b-\${round}</text>
      </option>
    </choice>
  </while>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({ scripts: { main }, compilerVersion: "dev" });
  engine.start("main");
  const first = engine.next();
  assert.equal(first.kind, "choices");
  engine.choose(0);
  assert.deepEqual(engine.next(), { kind: "text", text: "a-1" });

  const second = engine.next();
  assert.equal(second.kind, "choices");
  assert.deepEqual(
    second.items.map((item) => item.text),
    ["B"]
  );

  const snapshot = engine.snapshot();
  const restored = new ScriptLangEngine({ scripts: { main }, compilerVersion: "dev" });
  restored.resume(snapshot);
  const resumed = restored.next();
  assert.equal(resumed.kind, "choices");
  assert.deepEqual(
    resumed.items.map((item) => item.text),
    ["B"]
  );
});

test("option direct continue returns to current choice and respects once", () => {
  const main = compileScript(
    `
<script name="main">
  <choice text="Talk">
    <option text="Ask once" once="true">
      <text>asked</text>
      <continue/>
    </option>
    <option text="Leave">
      <text>left</text>
    </option>
  </choice>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({ scripts: { main }, compilerVersion: "dev" });
  engine.start("main");
  const first = engine.next();
  assert.equal(first.kind, "choices");
  engine.choose(0);
  assert.deepEqual(engine.next(), { kind: "text", text: "asked" });

  const second = engine.next();
  assert.equal(second.kind, "choices");
  assert.deepEqual(
    second.items.map((item) => item.text),
    ["Leave"]
  );
});

test("choice fall_over option appears only when regular options are unavailable", () => {
  const main = compileScript(
    `
<script name="main">
  <choice text="Door">
    <option text="Open" when="false"><text>open</text></option>
    <option text="Use key" once="true"><text>key</text><continue/></option>
    <option text="Leave" fall_over="true"><text>leave</text></option>
  </choice>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({ scripts: { main }, compilerVersion: "dev" });
  engine.start("main");
  const first = engine.next();
  assert.equal(first.kind, "choices");
  assert.deepEqual(
    first.items.map((item) => item.text),
    ["Use key"]
  );
  engine.choose(0);
  assert.deepEqual(engine.next(), { kind: "text", text: "key" });
  const second = engine.next();
  assert.equal(second.kind, "choices");
  assert.deepEqual(
    second.items.map((item) => item.text),
    ["Leave"]
  );
});

test("while break and continue follow nearest loop semantics", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="i" type="number" value="0"/>
  <while when="i &lt; 6">
    <code>i = i + 1;</code>
    <if when="i == 2">
      <continue/>
    </if>
    <if when="i == 5">
      <break/>
    </if>
    <text>tick-\${i}</text>
  </while>
  <text>done-\${i}</text>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({ scripts: { main }, compilerVersion: "dev" });
  engine.start("main");
  assert.deepEqual(engine.next(), { kind: "text", text: "tick-1" });
  assert.deepEqual(engine.next(), { kind: "text", text: "tick-3" });
  assert.deepEqual(engine.next(), { kind: "text", text: "tick-4" });
  assert.deepEqual(engine.next(), { kind: "text", text: "done-5" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("loop times authoring sugar runs fixed and expression counts", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="times" type="number" value="2"/>
  <loop times="3">
    <text>a</text>
  </loop>
  <loop times="times + 1">
    <text>b</text>
  </loop>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({ scripts: { main }, compilerVersion: "dev" });
  engine.start("main");
  assert.deepEqual(engine.next(), { kind: "text", text: "a" });
  assert.deepEqual(engine.next(), { kind: "text", text: "a" });
  assert.deepEqual(engine.next(), { kind: "text", text: "a" });
  assert.deepEqual(engine.next(), { kind: "text", text: "b" });
  assert.deepEqual(engine.next(), { kind: "text", text: "b" });
  assert.deepEqual(engine.next(), { kind: "text", text: "b" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("loop body supports break and continue through while-backed expansion", () => {
  const main = compileScript(
    `
<script name="main">
  <var name="i" type="number" value="0"/>
  <loop times="6">
    <code>i = i + 1;</code>
    <if when="i == 2"><continue/></if>
    <if when="i == 5"><break/></if>
    <text>tick-\${i}</text>
  </loop>
  <text>done-\${i}</text>
</script>
`,
    "main.script.xml"
  );
  const engine = new ScriptLangEngine({ scripts: { main }, compilerVersion: "dev" });
  engine.start("main");
  assert.deepEqual(engine.next(), { kind: "text", text: "tick-1" });
  assert.deepEqual(engine.next(), { kind: "text", text: "tick-3" });
  assert.deepEqual(engine.next(), { kind: "text", text: "tick-4" });
  assert.deepEqual(engine.next(), { kind: "text", text: "done-5" });
  assert.deepEqual(engine.next(), { kind: "end" });
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

test("json include globals support deep reads and reject mutations", () => {
  const deepRead = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: x.json -->
<script name="main">
  <text>\${x.a.b.c[123].e.f.g}</text>
</script>
`,
      "x.json": `{"a":{"b":{"c":{"123":{"e":{"f":{"g":"ok"}}}}}}}`,
    },
  });
  assert.deepEqual(deepRead.next(), { kind: "text", text: "ok" });

  const topWrite = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: x.json -->
<script name="main">
  <code>x = { a: 1 };</code>
</script>
`,
      "x.json": `{"a":{"b":1}}`,
    },
  });
  expectCode(() => topWrite.next(), "ENGINE_GLOBAL_READONLY");

  const nestedWrite = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: x.json -->
<script name="main">
  <code>x.a.b = 2;</code>
</script>
`,
      "x.json": `{"a":{"b":1}}`,
    },
  });
  expectCode(() => nestedWrite.next(), "ENGINE_GLOBAL_READONLY");
});

test("json globals follow per-script include visibility", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: x.json -->
<!-- include: child.script.xml -->
<script name="main">
  <call script="child"/>
</script>
`,
      "child.script.xml": `
<script name="child">
  <text>\${x.a}</text>
</script>
`,
      "x.json": `{"a":1}`,
    },
  });

  assert.throws(() => engine.next());
});

test("builtin random returns deterministic bounded integer sequence with seed", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main">
  <text>\${random(10)}</text>
  <text>\${random(10)}</text>
  <text>\${random(10)}</text>
</script>
`,
  };

  const runWithSeed = (seed: number): number[] => {
    const engine = createEngineFromXml({ scriptsXml, randomSeed: seed });
    const values: number[] = [];
    for (let i = 0; i < 3; i += 1) {
      const out = engine.next();
      assert.equal(out.kind, "text");
      const value = Number(out.text);
      assert.equal(Number.isInteger(value), true);
      assert.equal(value >= 0 && value <= 9, true);
      values.push(value);
    }
    assert.deepEqual(engine.next(), { kind: "end" });
    return values;
  };

  const seed42 = runWithSeed(42);
  assert.deepEqual(seed42, [6, 0, 4]);
  assert.deepEqual(runWithSeed(42), seed42);
  assert.notDeepEqual(runWithSeed(43), seed42);
});

test("builtin random uses rejection sampling for large bounds", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <text>\${random(2147483649)}</text>
</script>
`,
    },
    randomSeed: 42,
  });
  const out = engine.next();
  assert.equal(out.kind, "text");
  assert.equal(Number(out.text), 1925393290);
});

test("builtin random validates arity and integer argument", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <text>\${random()}</text>
</script>
`,
    },
  });
  expectCode(() => engine.next(), "ENGINE_RANDOM_ARITY");

  const badArgEngine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <text>\${random(1.5)}</text>
</script>
`,
    },
  });
  expectCode(() => badArgEngine.next(), "ENGINE_RANDOM_ARG");

  const badArityEngine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <text>\${random(1,2)}</text>
</script>
`,
    },
  });
  expectCode(() => badArityEngine.next(), "ENGINE_RANDOM_ARITY");

  const badRangeEngine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <text>\${random(0)}</text>
</script>
`,
    },
  });
  expectCode(() => badRangeEngine.next(), "ENGINE_RANDOM_ARG");

  const badUpperBoundEngine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <text>\${random(4294967297)}</text>
</script>
`,
    },
  });
  expectCode(() => badUpperBoundEngine.next(), "ENGINE_RANDOM_ARG");
});

test("engine rejects invalid random seed and reserved host function name", () => {
  const main = compileScript(
    `
<script name="main">
  <text>x</text>
</script>
`,
    "main.script.xml"
  );

  expectCode(
    () =>
      new ScriptLangEngine({
        scripts: { main },
        randomSeed: -1,
        compilerVersion: "dev",
      }),
    "ENGINE_RANDOM_SEED_INVALID"
  );

  expectCode(
    () =>
      new ScriptLangEngine({
        scripts: { main },
        hostFunctions: {
          random: () => 1,
        },
        compilerVersion: "dev",
      }),
    "ENGINE_HOST_FUNCTION_RESERVED"
  );
});

test("snapshot resume reuses pending choice items and prompt text for random-rendered choices", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main">
  <choice text="prompt-\${random(1000)}">
    <option text="roll-\${random(1000)}">
      <text>ok</text>
    </option>
  </choice>
</script>
`,
  };

  const engine = createEngineFromXml({ scriptsXml, randomSeed: 7, compilerVersion: "dev" });
  const first = engine.next();
  assert.equal(first.kind, "choices");
  const expectedText = first.items[0].text;
  const expectedPrompt = first.promptText;
  const snapshot = engine.snapshot();

  const resumed = resumeEngineFromXml({
    scriptsXml,
    snapshot,
    compilerVersion: "dev",
  });
  const resumedChoices = resumed.next();
  assert.equal(resumedChoices.kind, "choices");
  assert.equal(resumedChoices.items[0].text, expectedText);
  assert.equal(resumedChoices.promptText, expectedPrompt);
});

test("resume rejects snapshots missing random state or pending choice items", () => {
  const main = compileScript(
    `
<script name="main">
  <choice text="Choose">
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

  const missingRng = structuredClone(snapshot) as unknown as Record<string, unknown>;
  delete missingRng.rngState;
  const restoredMissingRng = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  expectCode(() => restoredMissingRng.resume(missingRng as unknown as typeof snapshot), "SNAPSHOT_RNG_STATE");

  const missingItems = structuredClone(snapshot) as unknown as Record<string, unknown>;
  delete missingItems.pendingChoiceItems;
  const restoredMissingItems = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  expectCode(
    () => restoredMissingItems.resume(missingItems as unknown as typeof snapshot),
    "SNAPSHOT_PENDING_CHOICE_ITEMS"
  );

  const badItemShape = structuredClone(snapshot) as unknown as Record<string, unknown>;
  badItemShape.pendingChoiceItems = [{ index: 0, id: 1, text: "ok" }];
  const restoredBadItemShape = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  expectCode(
    () => restoredBadItemShape.resume(badItemShape as unknown as typeof snapshot),
    "SNAPSHOT_PENDING_CHOICE_ITEMS"
  );

  const badPromptType = structuredClone(snapshot) as unknown as Record<string, unknown>;
  badPromptType.pendingChoicePromptText = 1;
  const restoredBadPromptType = new ScriptLangEngine({
    scripts: { main },
    compilerVersion: "dev",
  });
  expectCode(
    () => restoredBadPromptType.resume(badPromptType as unknown as typeof snapshot),
    "SNAPSHOT_PENDING_CHOICE_PROMPT_TEXT"
  );
});

test("resume accepts snapshots without pending choice prompt text", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main">
  <choice text="pick">
    <option text="ok"><text>x</text></option>
  </choice>
</script>
`,
  };
  const engine = createEngineFromXml({ scriptsXml, compilerVersion: "dev" });
  const choices = engine.next();
  assert.equal(choices.kind, "choices");
  assert.equal(choices.promptText, "pick");
  const snapshot = engine.snapshot();
  const snapshotWithoutPrompt = structuredClone(snapshot) as unknown as Record<string, unknown>;
  delete snapshotWithoutPrompt.pendingChoicePromptText;

  const resumed = resumeEngineFromXml({
    scriptsXml,
    snapshot: snapshotWithoutPrompt as unknown as typeof snapshot,
    compilerVersion: "dev",
  });
  const resumedChoices = resumed.next();
  assert.equal(resumedChoices.kind, "choices");
  assert.equal(resumedChoices.promptText, undefined);
});

test("script variable named random shadows builtin random symbol", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<script name="main">
  <var name="random" type="number" value="7"/>
  <text>\${random}</text>
</script>
`,
    },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "7" });
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
  <choice text="Choose">
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
  <choice text="Choose">
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
  <choice text="Choose">
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
  <choice text="Choose">
    <option text="ok"><text>x</text></option>
  </choice>
</script>
`,
      "holder.types.xml": typesXml,
    },
  });
  expectCode(() => restored.resume(mutated), "SNAPSHOT_TYPE_UNSUPPORTED");
});
