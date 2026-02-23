import assert from "node:assert/strict";
import { test } from "vitest";

import {
  ScriptLangEngine,
  ScriptLangError,
  compileScript,
  createEngineFromXml,
  parseXmlDocument,
} from "../src/index.js";

const asV2Body = (body: string): string => {
  let next = body;
  next = next.replace(/<vars\s*\/>/g, "");
  next = next.replace(/<vars>([\s\S]*?)<\/vars>/g, "$1");
  next = next.replace(/<step\s*\/>/g, "");
  next = next.replace(/<step>([\s\S]*?)<\/step>/g, "$1");
  return next;
};

const compile = (path: string, body: string): ReturnType<typeof compileScript> =>
  compileScript(
    `
<script name="${path}">
  ${asV2Body(body)}
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
        `<script name="a.script.xml"><vars><bad/></vars></script>`,
        "a.script.xml"
      ),
    "XML_REMOVED_NODE"
  );
  expectCode(
    () =>
      compileScript(
        `<script name="a.script.xml"><var name="hp" type="weird"/></script>`,
        "a.script.xml"
      ),
    "TYPE_PARSE_ERROR"
  );
  expectCode(
    () => compileScript(`<script name="a.script.xml"><if/></script>`, "a.script.xml"),
    "XML_MISSING_ATTR"
  );
  expectCode(
    () =>
      compileScript(
        `<script name="a.script.xml"><choice><bad/></choice></script>`,
        "a.script.xml"
      ),
    "XML_CHOICE_OPTION_INVALID"
  );
  expectCode(
    () => compileScript(`<script name="a.script.xml"><unknown/></script>`, "a.script.xml"),
    "XML_UNKNOWN_NODE"
  );
  expectCode(
    () =>
      compileScript(
        `<script name="a.script.xml"><call script="x" args="ref:"/></script>`,
        "a.script.xml"
      ),
    "CALL_ARGS_PARSE_ERROR"
  );
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
    `<vars/><step><choice><option text="a"><text>ok</text></option></choice></step>`
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
    `<vars/><step><choice><option text="a"><text>ok</text></option></choice></step>`
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
    `<vars/><step><choice><option text="a"><text>ok</text></option></choice></step>`
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
    `<vars/><step><choice><option text="x"><text>x</text></option></choice></step>`
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
    <choice>
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
    `<vars/><step><choice><option text="go"><text>ok</text></option></choice></step>`
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
    `<vars/><step><choice><option text="ok"><text>ok</text></option></choice></step>`
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
  <choice>
    <option text="hidden" when="false"><text>nope</text></option>
    <option text="visible"><text>ok</text></option>
  </choice>
  <choice>
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
    <choice>
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
