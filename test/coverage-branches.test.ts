import assert from "node:assert/strict";
import { test } from "vitest";

import { ScriptLangEngine, ScriptLangError, compileScript, createEngineFromXml, parseXmlDocument } from "../src";

const compile = (path: string, body: string): ReturnType<typeof compileScript> =>
  compileScript(
    `
<script name="${path}">
  ${body}
</script>
`,
    path
  );

const expectCode = (fn: () => unknown, code: string): void => {
  assert.throws(fn, (e: unknown) => {
    assert.ok(e instanceof ScriptLangError);
    assert.equal(e.code, code);
    return true;
  });
};

test("api supports host function usage path", () => {
  const engine = createEngineFromXml({
    entryScript: "main.script.xml",
    compilerVersion: "dev",
    hostFunctions: {
      add: (...args: unknown[]) => Number(args[0]) + Number(args[1]),
    },
    scriptsXml: {
      "main.script.xml": `
<script name="main.script.xml">
  <vars><var name="hp" type="number" value="1"/></vars>
  <step>
    <code>hp = add(hp, 2);</code>
    <text value="v=\${hp}"/>
  </step>
</script>
`,
    },
  });
  assert.deepEqual(engine.next(), { kind: "text", text: "v=3" });
});

test("xml parser throws parse and empty errors", () => {
  expectCode(() => parseXmlDocument("<script"), "XML_PARSE_ERROR");
  expectCode(() => parseXmlDocument(""), "XML_EMPTY");
  expectCode(() => parseXmlDocument("<!-- only-comment -->"), "XML_PARSE_ERROR");
});

test("compiler validation error branches", () => {
  expectCode(() => compileScript("<nope/>", "a.script.xml"), "XML_INVALID_ROOT");
  expectCode(
    () =>
      compileScript(
        `<script><vars><bad/></vars><step/></script>`,
        "a.script.xml"
      ),
    "XML_INVALID_VAR_NODE"
  );
  expectCode(
    () =>
      compileScript(
        `<script><vars><var name="hp" type="weird"/></vars><step/></script>`,
        "a.script.xml"
      ),
    "TYPE_PARSE_ERROR"
  );
  expectCode(
    () => compileScript(`<script><step><if/></step></script>`, "a.script.xml"),
    "XML_MISSING_ATTR"
  );
  expectCode(
    () =>
      compileScript(
        `<script><step><choice><bad/></choice></step></script>`,
        "a.script.xml"
      ),
    "XML_CHOICE_OPTION_INVALID"
  );
  expectCode(
    () => compileScript(`<script><step><unknown/></step></script>`, "a.script.xml"),
    "XML_UNKNOWN_NODE"
  );
  expectCode(
    () =>
      compileScript(
        `<script><step><call script="x" args="bad"/></step></script>`,
        "a.script.xml"
      ),
    "CALL_ARGS_PARSE_ERROR"
  );
});

test("engine start and next defensive branches", () => {
  const s = compile("main.script.xml", `<vars/><step><text value="x"/></step>`);
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
    `<vars/><step><choice><option text="a"><text value="ok"/></option></choice></step>`
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
    `<vars/><step><choice><option text="a"><text value="ok"/></option></choice></step>`
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
    `<vars/><step><choice><option text="a"><text value="ok"/></option></choice></step>`
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
    `<vars/><step><choice><option text="x"><text value="x"/></option></choice></step>`
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
<script>
  <vars><var name="x" type="number" value="1"/></vars>
  <step>
    <if when="true">
      <choice>
        <option text="ok"><text value="done"/></option>
      </choice>
    </if>
  </step>
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
});

test("return script valid path", () => {
  const a = compile("a.script.xml", `<vars/><step><return script="b.script.xml"/></step>`);
  const b = compile("b.script.xml", `<vars/><step><text value="B"/></step>`);
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
    `<vars><var name="hp" type="number" value="1"/></vars><step><call script="child.script.xml" args="hp:ref:hp"/></step>`
  );
  const child = compile("child.script.xml", `<vars><var name="hp" type="number" value="0"/></vars><step><return/></step>`);
  const parent = compile("parent.script.xml", `<vars/><step><call script="root.script.xml"/><text value="x"/></step>`);
  const engine = new ScriptLangEngine({
    scripts: { "root.script.xml": root, "child.script.xml": child, "parent.script.xml": parent },
    compilerVersion: "dev",
  });
  engine.start("parent.script.xml");
  expectCode(() => engine.next(), "ENGINE_TAIL_REF_UNSUPPORTED");
});

test("return continuation missing and root frame missing branches", () => {
  const s = compile("main.script.xml", `<vars/><step><text value="x"/></step>`);
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  const anyEngine = engine as any;

  expectCode(() => anyEngine.executeReturn(null), "ENGINE_ROOT_FRAME");
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
  const s = compile("main.script.xml", `<vars><var name="hp" type="number" value="1"/></vars><step><text value="x"/></step>`);
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  engine.start("main.script.xml");
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
    `<vars><var name="v" type="number" value="1"/></vars><step><if when="1"><text value="x"/></if></step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "a.script.xml": a }, compilerVersion: "dev" });
  engine.start("a.script.xml");
  expectCode(() => engine.next(), "ENGINE_BOOLEAN_EXPECTED");

  const anyEngine = engine as any;
  expectCode(() => anyEngine.buildVarTypeMap("missing.script.xml"), "ENGINE_SCRIPT_NOT_FOUND");
  expectCode(() => anyEngine.createScriptRootScope("missing.script.xml", {}), "ENGINE_SCRIPT_NOT_FOUND");

  // cover record/map type compatibility branches
  const recordType = { kind: "record", valueType: { kind: "primitive", name: "number" } } as const;
  const mapType = { kind: "map", keyType: "string", valueType: { kind: "primitive", name: "number" } } as const;
  expectCode(() => anyEngine.assertType("r", recordType, []), "ENGINE_TYPE_MISMATCH");
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
    `<script><vars><var name="n" type="number" value="undefined"/></vars><step/></script>`,
    "bad.script.xml"
  );
  const e2 = new ScriptLangEngine({ scripts: { "bad.script.xml": badInit }, compilerVersion: "dev" });
  expectCode(() => e2.start("bad.script.xml"), "ENGINE_VAR_UNDEFINED");

  const target = compile(
    "target.script.xml",
    `<vars><var name="n" type="number" value="1"/></vars><step><return/></step>`
  );
  const caller = compile(
    "caller.script.xml",
    `<vars/><step><call script="target.script.xml" args="ghost:1"/></step>`
  );
  const e3 = new ScriptLangEngine({
    scripts: { "caller.script.xml": caller, "target.script.xml": target },
    compilerVersion: "dev",
  });
  e3.start("caller.script.xml");
  expectCode(() => e3.next(), "ENGINE_CALL_ARG_UNKNOWN");
});

test("direct next with corrupted node kind hits unknown node branch", () => {
  const s = compile("main.script.xml", `<vars/><step><text value="x"/></step>`);
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
  const s = compile("main.script.xml", `<vars/><step><text value="x"/></step>`);
  const engine = new ScriptLangEngine({ scripts: { "main.script.xml": s }, compilerVersion: "dev" });
  engine.start("main.script.xml");
  const anyEngine = engine as any;
  anyEngine.frames[0].groupId = "ghost.group";
  expectCode(() => engine.next(), "ENGINE_GROUP_NOT_FOUND");
});

test("engine start/reset, empty-step completion, and direct return target path", () => {
  const main = compile("main.script.xml", `<vars/><step/>`);
  const target = compile("target.script.xml", `<vars/><step><text value="T"/></step>`);
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
      varTypes: null,
    },
  ];
  anyEngine.pendingChoice = { frameId: 99, nodeId: "x", options: [] };
  anyEngine.selectedChoices = new Set(["x"]);
  anyEngine.ended = true;
  anyEngine.frameCounter = 123;

  engine.start("main.script.xml");
  assert.equal(anyEngine.pendingChoice, null);
  assert.equal(anyEngine.ended, false);
  assert.equal(anyEngine.frames.length, 1);
  assert.equal(anyEngine.frameCounter, 2);

  assert.deepEqual(engine.next(), { kind: "end" });

  engine.start("main.script.xml");
  anyEngine.executeReturn("target.script.xml");
  assert.equal(anyEngine.frames[0].groupId, target.rootGroupId);
  assert.deepEqual(engine.next(), { kind: "text", text: "T" });

  const built = anyEngine.createScriptRootScope("target.script.xml", {});
  assert.equal(typeof built, "object");
  assert.ok("scope" in built);
});

test("direct executeReturn missing target and createScriptRootScope var loop path", () => {
  const script = compile(
    "vars.script.xml",
    `<vars><var name="hp" type="number" value="3"/></vars><step><text value="ok"/></step>`
  );
  const engine = new ScriptLangEngine({
    scripts: { "vars.script.xml": script },
    compilerVersion: "dev",
  });
  const anyEngine = engine as any;
  engine.start("vars.script.xml");
  expectCode(() => anyEngine.executeReturn("missing.script.xml"), "ENGINE_RETURN_TARGET");
  const built = anyEngine.createScriptRootScope("vars.script.xml", {});
  assert.equal((built.scope as Record<string, unknown>).hp, 3);
});

test("resume reconstructs continuation-bearing runtime frames", () => {
  const main = compile(
    "main.script.xml",
    `<vars/><step><call script="child.script.xml"/><text value="done"/></step>`
  );
  const child = compile(
    "child.script.xml",
    `<vars/><step><choice><option text="go"><text value="ok"/></option></choice></step>`
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
    `<vars/><step><choice><option text="ok"><text value="ok"/></option></choice></step>`
  );
  const target = compile(
    "target.script.xml",
    `<vars><var name="n" type="number" value="1"/></vars><step><text value="ok"/></step>`
  );
  const bad = compileScript(
    `<script><vars><var name="n" type="number" value="undefined"/></vars><step/></script>`,
    "bad-init.script.xml"
  );
  const engine = new ScriptLangEngine({
    scripts: {
      "waiting.script.xml": waiting,
      "target.script.xml": target,
      "bad-init.script.xml": bad,
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
  expectCode(() => anyEngine.createScriptRootScope("bad-init.script.xml", {}), "ENGINE_VAR_UNDEFINED");
});

test("engine control-flow branches for pending choices and hidden options", () => {
  const script = compileScript(
    `
<script name="control.script.xml">
  <vars><var name="n" type="number" value="1"/></vars>
  <step>
    <while when="false">
      <text value="never"/>
    </while>
    <choice>
      <option text="hidden" when="false"><text value="nope"/></option>
      <option text="visible"><text value="ok"/></option>
    </choice>
    <choice>
      <option text="all-hidden" when="false"><text value="x"/></option>
    </choice>
    <if when="false">
      <text value="then"/>
      <else><text value="else"/></else>
    </if>
  </step>
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
  const script = compile("main.script.xml", `<vars><var name="x" type="number" value="1"/></vars><step><text value="x"/></step>`);
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
    varTypes: null,
  };
  const calleeFrame = {
    frameId: 22,
    groupId: script.rootGroupId,
    nodeIndex: 0,
    scope: { v: 8 },
    completion: "none",
    scriptRoot: true,
    returnContinuation: { resumeFrameId: 11, nextNodeIndex: 7, refBindings: { v: "x" } },
    varTypes: null,
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
  anyEngine.executeReturn(null);
  assert.equal(anyEngine.ended, true);
  assert.deepEqual(anyEngine.frames, []);

  engine.start("main.script.xml");
  anyEngine.frames[0].returnContinuation = { resumeFrameId: 999, nextNodeIndex: 0, refBindings: {} };
  anyEngine.executeReturn(null);
  assert.equal(anyEngine.ended, true);
  assert.deepEqual(anyEngine.frames, []);
});

test("engine variable helpers cover type, path, and extra-scope branches", () => {
  const script = compile(
    "state.script.xml",
    `<vars>
      <var name="num" type="number" value="1"/>
      <var name="bag" type="Record&lt;string,Record&lt;string,number&gt;&gt;" value="({inner:{v:1}})"/>
    </vars>
    <step><text value="x"/></step>`
  );
  const engine = new ScriptLangEngine({ scripts: { "state.script.xml": script }, compilerVersion: "dev" });
  engine.start("state.script.xml");
  const anyEngine = engine as any;

  const arrayType = { kind: "array", elementType: { kind: "primitive", name: "number" } } as const;
  const recordType = { kind: "record", valueType: { kind: "primitive", name: "number" } } as const;
  expectCode(() => anyEngine.assertType("num", { kind: "primitive", name: "number" }, undefined), "ENGINE_TYPE_MISMATCH");
  anyEngine.assertType("arr", arrayType, [1]);
  anyEngine.assertType("rec", recordType, { a: 1 });

  assert.equal(anyEngine.readPath("bag.inner.v"), 1);
  expectCode(() => anyEngine.readPath("bag.missing"), "ENGINE_REF_PATH_READ");
  anyEngine.writePath("bag.inner.v", 3);
  assert.equal(anyEngine.readPath("bag.inner.v"), 3);
  expectCode(() => anyEngine.writePath("bag.inner.v.k", 1), "ENGINE_REF_PATH_WRITE");

  const extraRead = [{ local: 5 }];
  assert.equal(anyEngine.readVariable("local", extraRead), 5);
  assert.equal(anyEngine.readVariable("num", [{ ghost: 1 }]), 1);
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
    varTypes: null,
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
});

test("resume covers conditional option filters and frameCounter fallback branch", () => {
  const script = compileScript(
    `
<script name="resume-branches.script.xml">
  <vars/>
  <step>
    <if when="true">
      <choice>
        <option text="A" when="true"><text value="A"/></option>
        <option text="B"><text value="B"/></option>
      </choice>
    </if>
  </step>
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
    })),
  };
  const resumed = new ScriptLangEngine({
    scripts: { "resume-branches.script.xml": script },
    compilerVersion: "dev",
  });
  resumed.resume(mutated);
  assert.equal(resumed.waitingChoice, true);
});
