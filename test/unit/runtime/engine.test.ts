import assert from "node:assert/strict";
import { test } from "vitest";

import {
  compileProjectScriptsFromXmlMap,
  ScriptLangEngine,
  ScriptLangError,
  compileScript,
  createEngineFromXml,
  resumeEngineFromXml,
} from "../../../src/index.js";

const expectCode = (fn: () => unknown, code: string): void => {
  assert.throws(fn, (error: unknown) => {
    assert.ok(error instanceof ScriptLangError);
    assert.equal(error.code, code);
    return true;
  });
};

const asV2Body = (body: string): string => {
  let next = body;
  next = next.replace(/<vars\s*\/>/g, "");
  next = next.replace(/<vars>([\s\S]*?)<\/vars>/g, "$1");
  next = next.replace(/<step\s*\/>/g, "");
  next = next.replace(/<step>([\s\S]*?)<\/step>/g, "$1");
  return next;
};

const compile = (scriptPath: string, body: string): ReturnType<typeof compileScript> =>
  compileScript(
    `
<script name="${scriptPath}">
  ${asV2Body(body)}
</script>
`,
    scriptPath
  );

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

test("engine start supports entry args and validates type/unknown paths", () => {
  const alt = compileScript(
    `
<script name="alt" args="number:hp,string:name">
  <text>\${name}:\${hp}</text>
</script>
`,
    "alt.script.xml"
  );

  const okEngine = new ScriptLangEngine({
    scripts: { alt },
    compilerVersion: "dev",
  });
  okEngine.start("alt", { hp: 9, name: "N" });
  assert.deepEqual(okEngine.next(), { kind: "text", text: "N:9" });

  const typeEngine = new ScriptLangEngine({
    scripts: { alt },
    compilerVersion: "dev",
  });
  expectCode(() => typeEngine.start("alt", { hp: "9", name: "N" }), "ENGINE_TYPE_MISMATCH");

  const unknownEngine = new ScriptLangEngine({
    scripts: { alt },
    compilerVersion: "dev",
  });
  expectCode(() => unknownEngine.start("alt", { hp: 9, name: "N", extra: true }), "ENGINE_CALL_ARG_UNKNOWN");
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
<defs name="gamestate">
  <type name="Actor">
    <field name="hp" type="number"/>
    <field name="name" type="string"/>
  </type>
  <type name="BattleState">
    <field name="player" type="Actor"/>
    <field name="enemy" type="Actor"/>
  </type>
</defs>
`;

  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: gamestate.defs.xml -->
<script name="main">
  <var name="state" type="BattleState"/>
  <text>\${state.player.hp}:\${state.enemy.hp}</text>
</script>
`,
      "gamestate.defs.xml": typesXml,
    },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "0:0" });

  const missingFieldEngine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: gamestate.defs.xml -->
<script name="main">
  <var name="state" type="BattleState" value="{ player: { hp: 1, name: 'a' } }"/>
  <text>bad</text>
</script>
`,
      "gamestate.defs.xml": typesXml,
    },
  });
  expectCode(() => missingFieldEngine.next(), "ENGINE_TYPE_MISMATCH");

  const extraFieldEngine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: gamestate.defs.xml -->
<script name="main">
  <var name="state" type="BattleState" value="{ player: { hp: 1, name: 'a', extra: 1 }, enemy: { hp: 2, name: 'b' } }"/>
  <text>bad</text>
</script>
`,
      "gamestate.defs.xml": typesXml,
    },
  });
  expectCode(() => extraFieldEngine.next(), "ENGINE_TYPE_MISMATCH");
});

test("custom object fields support nested array and map typing", () => {
  const ok = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: complex.defs.xml -->
<script name="main">
  <var name="bag" type="Complex" value="{ items: [1,2], scores: new Map([['a', 1]]) }"/>
  <text>ok</text>
</script>
`,
      "complex.defs.xml": `
<defs name="complex">
  <type name="Complex">
    <field name="items" type="number[]"/>
    <field name="scores" type="Map&lt;string,number&gt;"/>
  </type>
</defs>
`,
    },
  });
  assert.deepEqual(ok.next(), { kind: "text", text: "ok" });

  const badNested = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: complex.defs.xml -->
<script name="main">
  <var name="bag" type="Complex" value="{ items: [1], scores: new Map([['a', 'bad']]) }"/>
  <text>x</text>
</script>
`,
      "complex.defs.xml": `
<defs name="complex">
  <type name="Complex">
    <field name="items" type="number[]"/>
    <field name="scores" type="Map&lt;string,number&gt;"/>
  </type>
</defs>
`,
    },
  });
  expectCode(() => badNested.next(), "ENGINE_TYPE_MISMATCH");
});

test("object type rejects null/array and missing expected key with same key count", () => {
  const typesXml = `
<defs name="pair">
  <type name="Pair">
    <field name="a" type="number"/>
    <field name="b" type="number"/>
  </type>
</defs>
`;

  const nullValue = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: pair.defs.xml -->
<script name="main">
  <var name="p" type="Pair" value="null"/>
</script>
`,
      "pair.defs.xml": typesXml,
    },
  });
  expectCode(() => nullValue.next(), "ENGINE_TYPE_MISMATCH");

  const arrayValue = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: pair.defs.xml -->
<script name="main">
  <var name="p" type="Pair" value="[1,2]"/>
</script>
`,
      "pair.defs.xml": typesXml,
    },
  });
  expectCode(() => arrayValue.next(), "ENGINE_TYPE_MISMATCH");

  const wrongKeySet = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: pair.defs.xml -->
<script name="main">
  <var name="p" type="Pair" value="{ a: 1, c: 2 }"/>
</script>
`,
      "pair.defs.xml": typesXml,
    },
  });
  expectCode(() => wrongKeySet.next(), "ENGINE_TYPE_MISMATCH");
});

test("resume rejects snapshot object varTypes with nested unsupported primitive", () => {
  const typesXml = `
<defs name="holder">
  <type name="Holder">
    <field name="hp" type="number"/>
  </type>
</defs>
`;
  const engine = createEngineFromXml({
    scriptsXml: {
      "main.script.xml": `
<!-- include: holder.defs.xml -->
<script name="main">
  <var name="h" type="Holder"/>
  <choice text="Choose">
    <option text="ok"><text>x</text></option>
  </choice>
</script>
`,
      "holder.defs.xml": typesXml,
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
<!-- include: holder.defs.xml -->
<script name="main">
  <var name="h" type="Holder"/>
  <choice text="Choose">
    <option text="ok"><text>x</text></option>
  </choice>
</script>
`,
      "holder.defs.xml": typesXml,
    },
  });
  expectCode(() => restored.resume(mutated), "SNAPSHOT_TYPE_UNSUPPORTED");
});
test("engine start and next defensive branches", () => {
  const s = compile("main.script.xml", `<vars/><step><text>x</text></step>`);
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  expectCode(() => engine.start("missing.script.xml"), "ENGINE_SCRIPT_NOT_FOUND");
  assert.deepEqual(engine.next(), { kind: "end" });
  engine.start("main.script.xml");
  assert.deepEqual(engine.next(), { kind: "text", text: "x" });
  assert.deepEqual(engine.next(), { kind: "end" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("engine constructor empty scripts and waitingChoice getter", () => {
  const engine = new ScriptLangEngine({ scripts: {}, compilerVersion: "dev" });
  assert.equal(engine.waitingChoice, false);
  expectCode(() => engine.start("missing.script.xml"), "ENGINE_SCRIPT_NOT_FOUND");
});

test("engine choice error branches", () => {
  const s = compile(
    "main.script.xml",
    `<vars/><step><choice text="Choose"><option text="a"><text>ok</text></option></choice></step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  expectCode(() => engine.choose(0), "ENGINE_NO_PENDING_CHOICE");
  engine.start("main.script.xml");
  const out = engine.next();
  assert.equal(out.kind, "choices");

  expectCode(() => engine.choose(9), "ENGINE_CHOICE_INDEX");

  const anyEngine = engine as any;
  anyEngine.pendingChoice.options[0].id = "not-exists";
  expectCode(() => engine.choose(0), "ENGINE_CHOICE_NOT_FOUND");
});

test("choice frame and node missing branches", () => {
  const s = compile(
    "main.script.xml",
    `<vars/><step><choice text="Choose"><option text="a"><text>ok</text></option></choice></step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  engine.start("main.script.xml");
  engine.next();
  const anyEngine = engine as any;

  const savedPending = anyEngine.pendingChoice;
  anyEngine.pendingChoice = { ...savedPending, frameId: 99999 };
  expectCode(() => engine.choose(0), "ENGINE_CHOICE_FRAME_MISSING");
  anyEngine.pendingChoice = savedPending;

  anyEngine.frames[0].nodeIndex = 999;
  expectCode(() => engine.choose(0), "ENGINE_CHOICE_NODE_MISSING");
});

test("while guard exceeded branch", () => {
  const s = compile(
    "loop.script.xml",
    `<vars/><step><while when="true"><code>/* no-op */</code></while></step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "loop.script.xml": s }, compilerVersion: "dev" });
  engine.start("loop.script.xml");
  expectCode(() => engine.next(), "ENGINE_GUARD_EXCEEDED");
});

test("engine snapshot resume error branches", () => {
  const s = compile(
    "main.script.xml",
    `<vars/><step><choice text="Choose"><option text="a"><text>ok</text></option></choice></step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "v1" });
  engine.start("main.script.xml");
  engine.next();
  const snap = engine.snapshot();

  const e2 = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "v2" });
  expectCode(() => e2.resume(snap), "SNAPSHOT_COMPILER_VERSION");

  expectCode(
    () => engine.resume({ ...snap, schemaVersion: "x" as never }),
    "SNAPSHOT_SCHEMA"
  );
  expectCode(
    () => engine.resume({ ...snap, waitingChoice: false }),
    "SNAPSHOT_WAITING_CHOICE"
  );
  expectCode(
    () => engine.resume({ ...snap, runtimeFrames: [{ ...snap.runtimeFrames[0], groupId: "ghost" }] }),
    "SNAPSHOT_GROUP_MISSING"
  );
  expectCode(
    () => engine.resume({ ...snap, runtimeFrames: [], cursor: { groupPath: [], nodeIndex: 0 } }),
    "SNAPSHOT_EMPTY"
  );
  expectCode(
    () => engine.resume({ ...snap, pendingChoiceNodeId: "wrong" }),
    "SNAPSHOT_PENDING_CHOICE"
  );
});

test("snapshot empty-frame defensive branch", () => {
  const s = compile(
    "main.script.xml",
    `<vars/><step><choice text="Choose"><option text="x"><text>x</text></option></choice></step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  engine.start("main.script.xml");
  engine.next();
  const anyEngine = engine as any;
  anyEngine.frames = [];
  expectCode(() => engine.snapshot(), "SNAPSHOT_EMPTY");
});

test("resume handles nested runtime frames", () => {
  const s = compileScript(
    `
<script name="nested.script.xml">
  <var name="x" type="number" value="1"/>
  <if when="true">
    <choice text="Choose">
      <option text="ok"><text>done</text></option>
    </choice>
  </if>
</script>
`,
    "nested.script.xml"
  );
  const engine = new ScriptLangEngine({ scripts: { "nested.script.xml": s }, compilerVersion: "dev" });
  engine.start("nested.script.xml");
  const out = engine.next();
  assert.equal(out.kind, "choices");
  const snap = engine.snapshot();
  const e2 = new ScriptLangEngine({ scripts: { "nested.script.xml": s }, compilerVersion: "dev" });
  e2.resume(snap);
  assert.equal(e2.waitingChoice, true);
});

test("call and return error branches", () => {
  const main = compile(
    "main.script.xml",
    `<vars/><step><call script="missing.script.xml"/></step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": main }, compilerVersion: "dev" });
  engine.start("main.script.xml");
  expectCode(() => engine.next(), "ENGINE_CALL_TARGET");

  const badReturn = compile(
    "ret.script.xml",
    `<vars/><step><return script="missing.script.xml"/></step>`
  );
  const e2 = new ScriptLangEngine({ scripts: { "ret.script.xml": badReturn }, compilerVersion: "dev" });
  e2.start("ret.script.xml");
  expectCode(() => e2.next(), "ENGINE_RETURN_TARGET");

  const returnArgTarget = compileScript(
    `<script name="ret-target.script.xml" args="number:v"><text>x</text></script>`,
    "ret-target.script.xml"
  );
  const badReturnArgs = compile(
    "ret-args.script.xml",
    `<vars/><step><return script="ret-target.script.xml" args="1,2"/></step>`
  );
  const e2b = new ScriptLangEngine({
    scripts: {
      "ret-args.script.xml": badReturnArgs,
      "ret-target.script.xml": returnArgTarget,
    },
    compilerVersion: "dev",
  });
  e2b.start("ret-args.script.xml");
  expectCode(() => e2b.next(), "ENGINE_RETURN_ARG_UNKNOWN");

  const refTarget = compileScript(
    `<script name="ref-target.script.xml" args="ref:number:x"><return/></script>`,
    "ref-target.script.xml"
  );
  const refCaller = compile(
    "ref-caller.script.xml",
    `<vars><var name="hp" type="number" value="1"/></vars><step><call script="ref-target.script.xml" args="1"/></step>`
  );
  const e3 = new ScriptLangEngine({
    scripts: {
      "ref-caller.script.xml": refCaller,
      "ref-target.script.xml": refTarget,
    },
    compilerVersion: "dev",
  });
  e3.start("ref-caller.script.xml");
  expectCode(() => e3.next(), "ENGINE_CALL_REF_MISMATCH");

  const valueTarget = compileScript(
    `<script name="value-target.script.xml" args="number:x"><return/></script>`,
    "value-target.script.xml"
  );
  const valueCaller = compile(
    "value-caller.script.xml",
    `<vars><var name="hp" type="number" value="1"/></vars><step><call script="value-target.script.xml" args="ref:hp"/></step>`
  );
  const e4 = new ScriptLangEngine({
    scripts: {
      "value-caller.script.xml": valueCaller,
      "value-target.script.xml": valueTarget,
    },
    compilerVersion: "dev",
  });
  e4.start("value-caller.script.xml");
  expectCode(() => e4.next(), "ENGINE_CALL_REF_MISMATCH");
});

test("return script valid path", () => {
  const a = compile("a.script.xml", `<vars/><step><return script="b.script.xml"/></step>`);
  const b = compile("b.script.xml", `<vars/><step><text>B</text></step>`);
  const engine = new ScriptLangEngine({
    scripts: { "a.script.xml": a, "b.script.xml": b },
    compilerVersion: "dev",
  });
  engine.start("a.script.xml");
  assert.deepEqual(engine.next(), { kind: "text", text: "B" });
});

test("tail call with ref unsupported branch", () => {
  const root = compile(
    "root.script.xml",
    `<vars><var name="hp" type="number" value="1"/></vars><step><call script="child.script.xml" args="ref:hp"/></step>`
  );
  const child = compileScript(
    `<script name="child.script.xml" args="ref:number:hp"><return/></script>`,
    "child.script.xml"
  );
  const parent = compile("parent.script.xml", `<vars/><step><call script="root.script.xml"/><text>x</text></step>`);
  const engine = new ScriptLangEngine({
    scripts: { "root.script.xml": root, "child.script.xml": child, "parent.script.xml": parent },
    compilerVersion: "dev",
  });
  engine.start("parent.script.xml");
  expectCode(() => engine.next(), "ENGINE_TAIL_REF_UNSUPPORTED");
});

test("return continuation missing and root frame missing branches", () => {
  const s = compile("main.script.xml", `<vars/><step><text>x</text></step>`);
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  const anyEngine = engine as any;

  expectCode(
    () =>
      anyEngine.executeReturn({
        kind: "return",
        targetScript: null,
        args: [],
        location: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
      }),
    "ENGINE_ROOT_FRAME"
  );
  expectCode(
    () =>
      anyEngine.executeCall({
        kind: "call",
        targetScript: "x",
        args: [],
        location: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
      }),
    "ENGINE_CALL_NO_FRAME"
  );
});

test("group and variable path error branches", () => {
  const s = compile("main.script.xml", `<vars><var name="hp" type="number" value="1"/></vars><step><text>x</text></step>`);
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  engine.start("main.script.xml");
  engine.next();
  const anyEngine = engine as any;

  expectCode(() => anyEngine.pushGroupFrame("ghost", "none"), "ENGINE_GROUP_NOT_FOUND");
  expectCode(() => anyEngine.readPath(""), "ENGINE_REF_PATH");
  expectCode(() => anyEngine.readPath("hp.a"), "ENGINE_REF_PATH_READ");
  expectCode(() => anyEngine.writePath("", 1), "ENGINE_REF_PATH");
  expectCode(() => anyEngine.writePath("hp", undefined), "ENGINE_UNDEFINED_WRITE");
  expectCode(() => anyEngine.writePath("hp.a", 1), "ENGINE_REF_PATH_WRITE");
  expectCode(() => anyEngine.readVariable("ghost"), "ENGINE_VAR_READ");
  expectCode(() => anyEngine.writeVariable("ghost", 1), "ENGINE_VAR_WRITE");
});

test("boolean, type map, and arg validation error branches", () => {
  const a = compile(
    "a.script.xml",
    `<vars><var name="v" type="number" value="1"/></vars><step><if when="1"><text>x</text></if></step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "a.script.xml": a }, compilerVersion: "dev" });
  engine.start("a.script.xml");
  expectCode(() => engine.next(), "ENGINE_BOOLEAN_EXPECTED");

  const anyEngine = engine as any;
  expectCode(() => anyEngine.buildParamTypeMap("missing.script.xml"), "ENGINE_SCRIPT_NOT_FOUND");
  expectCode(() => anyEngine.createScriptRootScope("missing.script.xml", {}), "ENGINE_SCRIPT_NOT_FOUND");

  // cover map type compatibility branches
  const mapType = { kind: "map", keyType: "string", valueType: { kind: "primitive", name: "number" } } as const;
  expectCode(() => anyEngine.assertType("m", mapType, {}), "ENGINE_TYPE_MISMATCH");
  expectCode(
    () => anyEngine.assertType("m", mapType, new Map([[1 as unknown as string, 2]])),
    "ENGINE_TYPE_MISMATCH"
  );
  anyEngine.assertType("m", mapType, new Map([["k", 2]]));
});

test("undefined assignment and type mismatch branches", () => {
  const s = compile(
    "main.script.xml",
    `<vars>
      <var name="n" type="number" value="1"/>
    </vars>
    <step>
      <code>n = undefined;</code>
    </step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  engine.start("main.script.xml");
  expectCode(() => engine.next(), "ENGINE_UNDEFINED_ASSIGN");

  const badInit = compileScript(
    `<script name="bad.script.xml"><var name="n" type="number" value="undefined"/></script>`,
    "bad.script.xml"
  );
  const e2 = new ScriptLangEngine({ scripts: { "bad.script.xml": badInit }, compilerVersion: "dev" });
  e2.start("bad.script.xml");
  expectCode(() => e2.next(), "ENGINE_VAR_UNDEFINED");

  const target = compile(
    "target.script.xml",
    `<vars><var name="n" type="number" value="1"/></vars><step><return/></step>`
  );
  const caller = compile(
    "caller.script.xml",
    `<vars/><step><call script="target.script.xml" args="1,2"/></step>`
  );
  const e3 = new ScriptLangEngine({
    scripts: { "caller.script.xml": caller, "target.script.xml": target },
    compilerVersion: "dev",
  });
  e3.start("caller.script.xml");
  expectCode(() => e3.next(), "ENGINE_CALL_ARG_UNKNOWN");
});

test("direct next with corrupted node kind hits unknown node branch", () => {
  const s = compile("main.script.xml", `<vars/><step><text>x</text></step>`);
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  engine.start("main.script.xml");
  const anyEngine = engine as any;
  const rootId = anyEngine.frames[0].groupId as string;
  anyEngine.groupLookup[rootId].group.nodes[0] = {
    kind: "mystery",
    id: "x",
    location: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
  };
  expectCode(() => engine.next(), "ENGINE_NODE_UNKNOWN");
});

test("next throws when runtime frame points to unknown group", () => {
  const s = compile("main.script.xml", `<vars/><step><text>x</text></step>`);
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  engine.start("main.script.xml");
  const anyEngine = engine as any;
  anyEngine.frames[0].groupId = "ghost.group";
  expectCode(() => engine.next(), "ENGINE_GROUP_NOT_FOUND");
});

test("engine start/reset, empty-step completion, and direct return target path", () => {
  const main = compile("main.script.xml", `<vars/><step/>`);
  const target = compile("target.script.xml", `<vars/><step><text>T</text></step>`);
  const engine = new ScriptLangEngine({
    scripts: { "main.script.xml": main, "target.script.xml": target },
    compilerVersion: "dev",
  });
  const anyEngine = engine as any;

  anyEngine.frames = [
    {
      frameId: 99,
      groupId: "ghost",
      nodeIndex: 5,
      scope: { x: 1 },
      completion: "none",
      scriptRoot: true,
      returnContinuation: null,
      varTypes: {},
    },
  ];
  anyEngine.pendingChoice = { frameId: 99, nodeId: "x", options: [] };
  anyEngine.ended = true;
  anyEngine.frameCounter = 123;

  engine.start("main.script.xml");
  assert.equal(anyEngine.pendingChoice, null);
  assert.equal(anyEngine.ended, false);
  assert.equal(anyEngine.frames.length, 1);
  assert.equal(anyEngine.frameCounter, 2);

  assert.deepEqual(engine.next(), { kind: "end" });

  engine.start("main.script.xml");
  anyEngine.executeReturn({
    kind: "return",
    targetScript: "target.script.xml",
    args: [],
    location: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
  });
  assert.equal(anyEngine.frames[0].groupId, target.rootGroupId);
  assert.deepEqual(engine.next(), { kind: "text", text: "T" });

  const built = anyEngine.createScriptRootScope("target.script.xml", {});
  assert.equal(typeof built, "object");
  assert.ok("scope" in built);
});

test("direct executeReturn missing target and createScriptRootScope var loop path", () => {
  const script = compile(
    "vars.script.xml",
    `<vars><var name="hp" type="number" value="3"/></vars><step><text>ok</text></step>`
  );
  const engine = new ScriptLangEngine({
    scripts: { "vars.script.xml": script },
    compilerVersion: "dev",
  });
  const anyEngine = engine as any;
  engine.start("vars.script.xml");
  expectCode(
    () =>
      anyEngine.executeReturn({
        kind: "return",
        targetScript: "missing.script.xml",
        args: [],
        location: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
      }),
    "ENGINE_RETURN_TARGET"
  );
  const built = anyEngine.createScriptRootScope("vars.script.xml", {});
  assert.equal((built.scope as Record<string, unknown>).hp, undefined);
});

test("resume reconstructs continuation-bearing runtime frames", () => {
  const main = compile(
    "main.script.xml",
    `<vars/><step><call script="child.script.xml"/><text>done</text></step>`
  );
  const child = compile(
    "child.script.xml",
    `<vars/><step><choice text="Choose"><option text="go"><text>ok</text></option></choice></step>`
  );
  const engine = new ScriptLangEngine({
    scripts: { "main.script.xml": main, "child.script.xml": child },
    compilerVersion: "dev",
  });
  engine.start("main.script.xml");
  const out = engine.next();
  assert.equal(out.kind, "choices");
  const snapshot = engine.snapshot();
  const resumed = new ScriptLangEngine({
    scripts: { "main.script.xml": main, "child.script.xml": child },
    compilerVersion: "dev",
  });
  resumed.resume(snapshot);
  assert.equal(resumed.waitingChoice, true);
});

test("engine helper paths for return target and root scope arg assignment", () => {
  const waiting = compile(
    "waiting.script.xml",
    `<vars/><step><choice text="Choose"><option text="ok"><text>ok</text></option></choice></step>`
  );
  const target = compileScript(
    `<script name="target.script.xml" args="number:n"><text>ok</text></script>`,
    "target.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: {
      "waiting.script.xml": waiting,
      "target.script.xml": target,
    },
    compilerVersion: "dev",
  });
  const anyEngine = engine as any;

  engine.start("waiting.script.xml");
  engine.next();
  const snap = engine.snapshot();
  engine.resume(snap);
  assert.equal(engine.waitingChoice, true);

  const resolved = anyEngine.requireReturnTargetScript("target.script.xml");
  assert.equal(resolved.rootGroupId, target.rootGroupId);
  expectCode(() => anyEngine.requireReturnTargetScript("missing.script.xml"), "ENGINE_RETURN_TARGET");

  const withArg = anyEngine.createScriptRootScope("target.script.xml", { n: 9 });
  assert.equal((withArg.scope as Record<string, unknown>).n, 9);
  expectCode(
    () => anyEngine.createScriptRootScope("target.script.xml", { ghost: 1 }),
    "ENGINE_CALL_ARG_UNKNOWN"
  );
});

test("engine control-flow branches for pending choices and hidden options", () => {
  const script = compileScript(
    `
<script name="control.script.xml">
  <var name="n" type="number" value="1"/>
  <while when="false">
    <text>never</text>
  </while>
  <choice text="Choose">
    <option text="hidden" when="false"><text>nope</text></option>
    <option text="visible"><text>ok</text></option>
  </choice>
  <choice text="Choose">
    <option text="all-hidden" when="false"><text>x</text></option>
  </choice>
  <if when="false">
    <text>then</text>
    <else><text>else</text></else>
  </if>
</script>
`,
    "control.script.xml"
  );
  const engine = new ScriptLangEngine({ scripts: { "control.script.xml": script }, compilerVersion: "dev" });
  engine.start("control.script.xml");

  const firstChoices = engine.next();
  assert.equal(firstChoices.kind, "choices");
  assert.equal(engine.waitingChoice, true);
  const secondChoices = engine.next();
  assert.equal(secondChoices.kind, "choices");

  engine.choose(0);
  assert.deepEqual(engine.next(), { kind: "text", text: "ok" });
  assert.deepEqual(engine.next(), { kind: "text", text: "else" });
});

test("engine finishFrame and executeReturn continuation branches", () => {
  const script = compile("main.script.xml", `<vars><var name="x" type="number" value="1"/></vars><step><text>x</text></step>`);
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": script }, compilerVersion: "dev" });
  engine.start("main.script.xml");
  const anyEngine = engine as any;

  const resumeFrame = {
    frameId: 11,
    groupId: script.rootGroupId,
    nodeIndex: 1,
    scope: { x: 1 },
      completion: "none",
      scriptRoot: false,
      returnContinuation: null,
      varTypes: {},
  };
  const calleeFrame = {
    frameId: 22,
    groupId: script.rootGroupId,
    nodeIndex: 0,
    scope: { v: 8 },
      completion: "none",
      scriptRoot: true,
      returnContinuation: { resumeFrameId: 11, nextNodeIndex: 7, refBindings: { v: "x" } },
      varTypes: {},
  };

  anyEngine.frames = [resumeFrame, calleeFrame];
  anyEngine.finishFrame(calleeFrame);
  assert.equal(anyEngine.frames.length, 1);
  assert.equal(anyEngine.frames[0].scope.x, 8);
  assert.equal(anyEngine.frames[0].nodeIndex, 7);

  anyEngine.frames = [
    {
      ...calleeFrame,
      returnContinuation: { resumeFrameId: 999, nextNodeIndex: 0, refBindings: {} },
    },
  ];
  anyEngine.ended = false;
  anyEngine.finishFrame(anyEngine.frames[0]);
  assert.equal(anyEngine.ended, true);
  assert.deepEqual(anyEngine.frames, []);

  engine.start("main.script.xml");
  anyEngine.executeReturn({
    kind: "return",
    targetScript: null,
    args: [],
    location: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
  });
  assert.equal(anyEngine.ended, true);
  assert.deepEqual(anyEngine.frames, []);

  engine.start("main.script.xml");
  anyEngine.frames[0].returnContinuation = { resumeFrameId: 999, nextNodeIndex: 0, refBindings: {} };
  anyEngine.executeReturn({
    kind: "return",
    targetScript: null,
    args: [],
    location: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
  });
  assert.equal(anyEngine.ended, true);
  assert.deepEqual(anyEngine.frames, []);
});

test("executeReturn transfer with missing resume frame keeps running target then ends", () => {
  const holder = compileScript(
    `<script name="holder.script.xml" args="number:v"><text>x</text></script>`,
    "holder.script.xml"
  );
  const target = compileScript(
    `<script name="target.script.xml" args="number:n"><text>n=\${n}</text></script>`,
    "target.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { "holder.script.xml": holder, "target.script.xml": target },
    compilerVersion: "dev",
  });
  const anyEngine = engine as any;
  engine.start("holder.script.xml");

  anyEngine.frames = [
    {
      frameId: 1,
      groupId: holder.rootGroupId,
      nodeIndex: 0,
      scope: { v: 4 },
      completion: "none",
      scriptRoot: true,
      returnContinuation: { resumeFrameId: 999, nextNodeIndex: 0, refBindings: { v: "v" } },
      varTypes: { v: { kind: "primitive", name: "number" } },
    },
  ];

  anyEngine.executeReturn({
    kind: "return",
    targetScript: "target.script.xml",
    args: [{ valueExpr: "v + 1", isRef: false }],
    location: { start: { line: 1, column: 1 }, end: { line: 1, column: 1 } },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "n=5" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("engine variable helpers cover type, path, and extra-scope branches", () => {
  const script = compileScript(
    `<script name="state.script.xml" args="number:num">
      <text>x</text>
    </script>`,
    "state.script.xml"
  );
  const engine = new ScriptLangEngine({ scripts: { "state.script.xml": script }, compilerVersion: "dev" });
  engine.start("state.script.xml");
  engine.next();
  const anyEngine = engine as any;
  anyEngine.frames[0].scope.bag = { inner: { v: 1 } };

  const arrayType = { kind: "array", elementType: { kind: "primitive", name: "number" } } as const;
  expectCode(() => anyEngine.assertType("num", { kind: "primitive", name: "number" }, undefined), "ENGINE_TYPE_MISMATCH");
  anyEngine.assertType("arr", arrayType, [1]);

  assert.equal(anyEngine.readPath("bag.inner.v"), 1);
  expectCode(() => anyEngine.readPath("bag.missing"), "ENGINE_REF_PATH_READ");
  anyEngine.writePath("bag.inner.v", 3);
  assert.equal(anyEngine.readPath("bag.inner.v"), 3);
  expectCode(() => anyEngine.writePath("bag.inner.v.k", 1), "ENGINE_REF_PATH_WRITE");

  const extraRead = [{ local: 5 }];
  assert.equal(anyEngine.readVariable("local", extraRead), 5);
  assert.equal(anyEngine.readVariable("num", [{ ghost: 1 }]), 0);
  const extraWrite = [{ local: 1 }];
  anyEngine.writeVariable("local", 9, extraWrite);
  assert.equal(extraWrite[0].local, 9);
  anyEngine.writeVariable("num", 4, [{ ghost: 1 }]);
  assert.equal(anyEngine.readVariable("num"), 4);

  const scopedNoTypeFrame = {
    frameId: 1000,
    groupId: script.rootGroupId,
    nodeIndex: 0,
    scope: { temp: 1 },
    completion: "none",
    scriptRoot: false,
    returnContinuation: null,
    varTypes: {},
  };
  anyEngine.frames.push(scopedNoTypeFrame);
  anyEngine.writeVariable("temp", 2);
  assert.equal(scopedNoTypeFrame.scope.temp, 2);
  const rootIdx = anyEngine.findCurrentRootFrameIndex();
  assert.equal(rootIdx >= 0, true);
  anyEngine.frames.pop();

  expectCode(
    () => anyEngine.createScriptRootScope("state.script.xml", { num: undefined }),
    "ENGINE_CALL_ARG_UNDEFINED"
  );
  const withArgs = anyEngine.createScriptRootScope("state.script.xml", { num: 9 });
  assert.equal((withArgs.scope as Record<string, unknown>).num, 9);

  const evalWithExtra = anyEngine.evalExpression("local + 1", [{ local: 2 }]);
  assert.equal(evalWithExtra, 3);

  const savedFrames = anyEngine.frames;
  anyEngine.frames = [];
  expectCode(
    () => anyEngine.executeVarDeclaration({ name: "z", type: { kind: "primitive", name: "number" }, initialValueExpr: null }),
    "ENGINE_VAR_FRAME"
  );
  anyEngine.frames = savedFrames;
  expectCode(
    () => anyEngine.executeVarDeclaration({ name: "num", type: { kind: "primitive", name: "number" }, initialValueExpr: null }),
    "ENGINE_VAR_DUPLICATE"
  );
});

test("engine readonly JSON globals cover visibility and mutation branches", () => {
  const script = compileScript(
    `<script name="globals.script.xml">
      <text>\${g.player.hp}</text>
    </script>`,
    "globals.script.xml"
  );
  script.visibleJsonGlobals = ["g"];

  const engine = new ScriptLangEngine({
    scripts: { "globals.script.xml": script },
    compilerVersion: "dev",
    globalJson: {
      g: {
        player: { hp: 7 },
      },
    },
  });
  engine.start("globals.script.xml");
  assert.deepEqual(engine.next(), { kind: "text", text: "7" });

  const anyEngine = engine as any;
  const globalView = anyEngine.readVariable("g", [], "globals.script.xml");
  const globalViewAgain = anyEngine.readVariable("g", [], "globals.script.xml");
  assert.equal(globalView, globalViewAgain);
  assert.equal(anyEngine.isVisibleJsonGlobal(null, "g"), false);

  const savedFrames = anyEngine.frames;
  anyEngine.frames = [];
  assert.equal(anyEngine.resolveCurrentScriptName(), null);
  anyEngine.frames = [{ ...savedFrames[0], groupId: "ghost.group" }];
  assert.equal(anyEngine.resolveCurrentScriptName(), null);
  anyEngine.frames = savedFrames;

  expectCode(() => anyEngine.writeVariable("g", { player: { hp: 9 } }, [], "globals.script.xml"), "ENGINE_GLOBAL_READONLY");
  expectCode(() => anyEngine.runCode("g.player.hp = 9;"), "ENGINE_GLOBAL_READONLY");
  expectCode(() => anyEngine.runCode("delete g.player.hp;"), "ENGINE_GLOBAL_READONLY");
  expectCode(
    () => anyEngine.runCode("Object.defineProperty(g.player, 'hp', { value: 1 });"),
    "ENGINE_GLOBAL_READONLY"
  );
  expectCode(() => anyEngine.runCode("Object.setPrototypeOf(g, null);"), "ENGINE_GLOBAL_READONLY");

  const extraScope = [{ local: null as unknown }];
  anyEngine.writeVariable("local", globalView, extraScope, "globals.script.xml");
  assert.equal(extraScope[0].local === globalView, false);

  const savedVisible = anyEngine.visibleJsonByScript["globals.script.xml"];
  delete anyEngine.visibleJsonByScript["globals.script.xml"];
  anyEngine.runCode("const noop = 1;");
  anyEngine.visibleJsonByScript["globals.script.xml"] = savedVisible;

  const framesForSandbox = anyEngine.frames;
  anyEngine.frames = [];
  anyEngine.buildSandbox([]);
  anyEngine.frames = framesForSandbox;
});

test("resume covers conditional option filters and frameCounter fallback branch", () => {
  const script = compileScript(
    `
<script name="resume-branches.script.xml">
  <if when="true">
    <choice text="Choose">
      <option text="A" when="true"><text>A</text></option>
      <option text="B"><text>B</text></option>
    </choice>
  </if>
</script>
`,
    "resume-branches.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { "resume-branches.script.xml": script },
    compilerVersion: "dev",
  });
  engine.start("resume-branches.script.xml");
  const choices = engine.next();
  assert.equal(choices.kind, "choices");
  const snap = engine.snapshot();
  assert.equal(snap.runtimeFrames.length >= 2, true);

  const mutated = {
    ...snap,
    runtimeFrames: snap.runtimeFrames.map((frame, i) => ({
      ...frame,
      frameId: i === 0 ? 200 : 100,
      varTypes: undefined,
    })),
  };
  const resumed = new ScriptLangEngine({
    scripts: { "resume-branches.script.xml": script },
    compilerVersion: "dev",
  });
  resumed.resume(mutated);
  assert.equal(resumed.waitingChoice, true);
});

test("engine once-state and control-flow error branches", () => {
  const script = compileScript(
    `
<script name="branch-once.script.xml">
  <choice text="Pick">
    <option text="A" once="true"><text>a</text><continue/></option>
    <option text="B" fall_over="true"><text>b</text></option>
  </choice>
</script>
`,
    "branch-once.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: { "branch-once.script.xml": script },
    compilerVersion: "dev",
  });
  engine.start("branch-once.script.xml");
  const first = engine.next();
  assert.equal(first.kind, "choices");
  const anyEngine = engine as any;
  anyEngine.assertSnapshotOnceState(undefined);
  expectCode(() => anyEngine.assertSnapshotOnceState([]), "SNAPSHOT_ONCE_STATE_INVALID");
  expectCode(
    () => anyEngine.assertSnapshotOnceState({ "branch-once.script.xml": [1] }),
    "SNAPSHOT_ONCE_STATE_INVALID"
  );
  assert.deepEqual(anyEngine.deserializeOnceState(undefined), {});
  anyEngine.markOnceState("branch-once.script.xml", "option:c0");
  anyEngine.markOnceState("branch-once.script.xml", "option:c1");
  assert.equal(anyEngine.hasOnceState("branch-once.script.xml", "option:c1"), true);

  expectCode(() => anyEngine.executeBreak(), "ENGINE_WHILE_CONTROL_TARGET_MISSING");
  expectCode(() => anyEngine.executeContinueWhile(), "ENGINE_WHILE_CONTROL_TARGET_MISSING");
  expectCode(() => anyEngine.executeContinueChoice(), "ENGINE_CHOICE_CONTINUE_TARGET_MISSING");

  anyEngine.frames = [
    {
      frameId: 9,
      groupId: "ghost.group",
      nodeIndex: 1,
      scope: {},
      completion: "none",
      scriptRoot: false,
      returnContinuation: null,
      varTypes: {},
    },
  ];
  assert.equal(anyEngine.findChoiceContinueContext(), null);

  anyEngine.frames = [
    {
      frameId: 10,
      groupId: script.rootGroupId,
      nodeIndex: 1,
      scope: {},
      completion: "none",
      scriptRoot: true,
      returnContinuation: null,
      varTypes: {},
    },
    {
      frameId: 11,
      groupId: script.rootGroupId,
      nodeIndex: 0,
      scope: {},
      completion: "resumeAfterChild",
      scriptRoot: false,
      returnContinuation: null,
      varTypes: {},
    },
  ];
  assert.equal(anyEngine.findChoiceContinueContext(), null);

  anyEngine.frames = [
    {
      frameId: 1,
      groupId: script.rootGroupId,
      nodeIndex: 0,
      scope: {},
      completion: "none",
      scriptRoot: true,
      returnContinuation: null,
      varTypes: {},
    },
    {
      frameId: 2,
      groupId: script.rootGroupId,
      nodeIndex: 0,
      scope: {},
      completion: "whileBody",
      scriptRoot: false,
      returnContinuation: null,
      varTypes: {},
    },
  ];
  expectCode(() => anyEngine.executeBreak(), "ENGINE_WHILE_CONTROL_TARGET_MISSING");
  expectCode(() => anyEngine.executeContinueWhile(), "ENGINE_WHILE_CONTROL_TARGET_MISSING");
});

test("defs functions run in code/expr and support recursion", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "shared.defs.xml": `
<defs name="shared">
  <type name="CustomType"><field name="value" type="number"/></type>
  <function name="add" args="number:a,CustomType:b" return="number:r">
    r = a + b.value;
  </function>
  <function name="fact" args="number:n" return="number:out">
    if (n &lt;= 1) {
      out = 1;
    } else {
      out = n * fact(n - 1);
    }
  </function>
</defs>
`,
      "main.script.xml": `
<!-- include: shared.defs.xml -->
<!-- include: game.json -->
<script name="main">
  <var name="base" type="number" value="2"/>
  <var name="obj" type="CustomType" value="{ value: 3 }"/>
  <code>base = add(base, obj);</code>
  <text>\${add(base, obj)}</text>
  <text>\${fact(5)}</text>
</script>
`,
      "game.json": `{"bonus":1}`,
    },
    hostFunctions: {
      hostInc: (n: unknown) => Number(n) + 1,
    },
  });

  assert.deepEqual(engine.next(), { kind: "text", text: "8" });
  assert.deepEqual(engine.next(), { kind: "text", text: "120" });
  assert.deepEqual(engine.next(), { kind: "end" });
});

test("defs function runtime validates arity, arg/return types, and script-var isolation", () => {
  expectCode(
    () =>
      createEngineFromXml({
        scriptsXml: {
          "defs.defs.xml": `<defs name="defs"><function name="f" args="number:a" return="number:r">r = a;</function></defs>`,
          "main.script.xml": `<!-- include: defs.defs.xml -->
<script name="main"><code>f();</code></script>`,
        },
      }).next(),
    "ENGINE_FUNCTION_ARITY"
  );

  expectCode(
    () =>
      createEngineFromXml({
        scriptsXml: {
          "defs.defs.xml": `<defs name="defs"><function name="f" args="number:a" return="number:r">r = a;</function></defs>`,
          "main.script.xml": `<!-- include: defs.defs.xml -->
<script name="main"><code>f("x");</code></script>`,
        },
      }).next(),
    "ENGINE_TYPE_MISMATCH"
  );

  expectCode(
    () =>
      createEngineFromXml({
        scriptsXml: {
          "defs.defs.xml": `<defs name="defs"><function name="f" return="number:r">r = "x";</function></defs>`,
          "main.script.xml": `<!-- include: defs.defs.xml -->
<script name="main"><code>f();</code></script>`,
        },
      }).next(),
    "ENGINE_TYPE_MISMATCH"
  );

  assert.throws(
    () =>
      createEngineFromXml({
        scriptsXml: {
          "defs.defs.xml": `<defs name="defs"><function name="f" args="number:x" return="number:r">r = x + hidden;</function></defs>`,
          "main.script.xml": `<!-- include: defs.defs.xml -->
<script name="main"><var name="hidden" type="number" value="1"/><code>f(2);</code></script>`,
        },
      }).next(),
    /hidden is not defined/
  );

  const ok = createEngineFromXml({
    scriptsXml: {
      "defs.defs.xml": `<defs name="defs"><function name="f" return="number:r">/* no-op */</function></defs>`,
      "main.script.xml": `<!-- include: defs.defs.xml -->
<script name="main"><text>\${f()}</text></script>`,
    },
  });
  assert.deepEqual(ok.next(), { kind: "text", text: "0" });
});

test("engine detects host-function name conflicts with defs functions", () => {
  const scripts = compileProjectScriptsFromXmlMap({
    "main.script.xml": `<!-- include: defs.defs.xml -->
<script name="main"><text>x</text></script>`,
    "defs.defs.xml": `<defs name="defs"><function name="f" return="number:r">r = 1;</function></defs>`,
  });

  expectCode(
    () =>
      new ScriptLangEngine({
        scripts,
        hostFunctions: {
          f: () => 1,
        },
      }),
    "ENGINE_HOST_FUNCTION_CONFLICT"
  );
});

test("sandbox helper branch covers preexisting symbol when injecting defs functions", () => {
  const engine = createEngineFromXml({
    scriptsXml: {
      "defs.defs.xml": `<defs name="defs"><function name="f" return="number:r">r = 1;</function></defs>`,
      "main.script.xml": `<!-- include: defs.defs.xml -->
<script name="main"><text>x</text></script>`,
    },
  });
  const anyEngine = engine as any;
  const sandbox = anyEngine.buildSandbox([{ f: 9 }], {
    scriptName: "main",
    includeFrameVars: false,
    extraScopeVarTypes: [{}],
  });
  assert.equal((sandbox as { f: number }).f, 9);
});
