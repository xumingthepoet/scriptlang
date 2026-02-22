import assert from "node:assert/strict";
import test from "node:test";

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
