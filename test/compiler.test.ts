import assert from "node:assert/strict";
import { test } from "vitest";

import { ScriptLangError, compileProjectScriptsFromXmlMap, compileScript } from "../src/index.js";

test("compile script into implicit groups with params and var nodes", () => {
  const xml = `
<script name="main" args="seed:number,target:number:ref">
  <var name="hp" type="number" value="100"/>
  <text>hello</text>
  <if when="hp > 0">
    <text>alive</text>
    <else>
      <text>dead</text>
    </else>
  </if>
  <while when="hp > 0">
    <code>hp = hp - 1;</code>
  </while>
  <choice>
    <option text="Go" when="hp > 0">
      <call script="next" args="value:hp"/>
    </option>
  </choice>
</script>
`;
  const ir = compileScript(xml, "main.script.xml");

  assert.equal(ir.scriptName, "main");
  assert.equal(ir.params.length, 2);
  assert.equal(ir.params[0].name, "seed");
  assert.equal(ir.params[1].isRef, true);
  assert.ok(Object.keys(ir.groups).length >= 5);
  assert.equal(ir.groups[ir.rootGroupId].nodes[0]?.kind, "var");
  assert.equal(ir.groups[ir.rootGroupId].nodes[1]?.kind, "text");
  assert.equal(ir.groups[ir.rootGroupId].nodes[2]?.kind, "if");
  assert.equal(ir.groups[ir.rootGroupId].nodes[3]?.kind, "while");
  assert.equal(ir.groups[ir.rootGroupId].nodes[4]?.kind, "choice");
});

test("reject removed nodes", () => {
  const xmlWithVars = `<script name="x"><vars/></script>`;
  const xmlWithStep = `<script name="x"><step/></script>`;
  const xmlWithSet = `<script name="x"><set target="hp" value="1"/></script>`;

  for (const xml of [xmlWithVars, xmlWithStep, xmlWithSet]) {
    assert.throws(
      () => compileScript(xml, "broken.script.xml"),
      (error: unknown) => {
        assert.ok(error instanceof ScriptLangError);
        assert.equal(error.code, "XML_REMOVED_NODE");
        return true;
      }
    );
  }
});

test("script args parser validates syntax and duplicates", () => {
  const ok = compileScript(
    `<script name="ok" args="a:number,b:Map&lt;string,number&gt;:ref"><text>x</text></script>`,
    "ok.script.xml"
  );
  assert.equal(ok.params[1].isRef, true);
  assert.equal(ok.params[1].type.kind, "map");

  assert.throws(() => compileScript(`<script name="x" args="bad"><text>x</text></script>`, "bad.script.xml"));
  assert.throws(
    () => compileScript(`<script name="x" args="a:   "><text>x</text></script>`, "bad-space.script.xml"),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "SCRIPT_ARGS_PARSE_ERROR");
      return true;
    }
  );

  assert.throws(
    () => compileScript(`<script name="x" args="a:number,a:string"><text>x</text></script>`, "dup.script.xml"),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "SCRIPT_ARGS_DUPLICATE");
      return true;
    }
  );

  const trailingComma = compileScript(
    `<script name="x" args="a:number,"><text>x</text></script>`,
    "trailing.script.xml"
  );
  assert.equal(trailingComma.params.length, 1);

  assert.throws(
    () =>
      compileScript(
        `<script name="x" args="r:Record&lt;string,number&gt;"><text>x</text></script>`,
        "record-removed.script.xml"
      ),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "TYPE_PARSE_ERROR");
      return true;
    }
  );

  assert.throws(
    () =>
      compileScript(
        `<script name="x" args="n:null"><text>x</text></script>`,
        "null-arg-removed.script.xml"
      ),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "TYPE_PARSE_ERROR");
      return true;
    }
  );
});

test("null type is rejected in var declaration", () => {
  assert.throws(
    () =>
      compileScript(
        `<script name="x"><var name="n" type="null"/></script>`,
        "null-var-removed.script.xml"
      ),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "TYPE_PARSE_ERROR");
      return true;
    }
  );
});

test("call args parser separator edge cases", () => {
  const quoted = compileScript(
    `<script name="x"><call script="target" args="msg:&quot;a,b&quot;"/></script>`,
    "quoted.script.xml"
  );
  const quotedCall = quoted.groups[quoted.rootGroupId].nodes[0];
  assert.equal(quotedCall.kind, "call");
  assert.equal(quotedCall.args.length, 1);

  const nested = compileScript(
    `<script name="x"><call script="target" args="a:({k:[1,2]}),b:(1,2),c:'x,y'"/></script>`,
    "nested.script.xml"
  );
  const nestedCall = nested.groups[nested.rootGroupId].nodes[0];
  assert.equal(nestedCall.kind, "call");
  assert.equal(nestedCall.args.length, 3);

  assert.throws(
    () =>
      compileScript(
        `<script name="x"><call script="target" args="a:"/></script>`,
        "badargs.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "CALL_ARGS_PARSE_ERROR");
      return true;
    }
  );

  assert.throws(
    () =>
      compileScript(
        `<script name="x"><call script="target" args="a: "/></script>`,
        "badargs2.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "CALL_ARGS_PARSE_ERROR");
      return true;
    }
  );

  assert.throws(
    () =>
      compileScript(
        `<script name="x"><call script="target" args=" :x"/></script>`,
        "badargs3.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "CALL_ARGS_PARSE_ERROR");
      return true;
    }
  );
});

test("required attributes reject empty string", () => {
  assert.throws(
    () => compileScript(`<script name="x"><call script=""/></script>`, "empty-attr.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_MISSING_ATTR");
      return true;
    }
  );
});

test("compile text node supports inline text content", () => {
  const ir = compileScript(
    `
<script name="inline">
  <text>inline value</text>
</script>
`,
    "inline-text.script.xml"
  );
  const node = ir.groups[ir.rootGroupId].nodes[0];
  assert.equal(node.kind, "text");
  assert.equal(node.value, "inline value");
});

test("text/code reject value attribute and empty inline content", () => {
  const textWithValueAttr = `<script name="x"><text ${"value"}="x"/></script>`;
  const codeWithValueAttr = `<script name="x"><code ${"value"}="x"/></script>`;
  assert.throws(
    () => compileScript(textWithValueAttr, "text-value.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_ATTR_NOT_ALLOWED");
      return true;
    }
  );
  assert.throws(
    () => compileScript(codeWithValueAttr, "code-value.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_ATTR_NOT_ALLOWED");
      return true;
    }
  );
  assert.throws(
    () => compileScript(`<script name="x"><text></text></script>`, "text-empty.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_EMPTY_NODE_CONTENT");
      return true;
    }
  );
  assert.throws(
    () => compileScript(`<script name="x"><code>   </code></script>`, "code-empty.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_EMPTY_NODE_CONTENT");
      return true;
    }
  );
});

test("script name is required", () => {
  assert.throws(
    () => compileScript(`<script><text>x</text></script>`, "missing-name.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_MISSING_ATTR");
      return true;
    }
  );
});

const expectCode = (fn: () => unknown, code: string): void => {
  assert.throws(fn, (error: unknown) => {
    assert.ok(error instanceof ScriptLangError);
    assert.equal(error.code, code);
    return true;
  });
};

test("project compiler validates types include graph and script type references", () => {
  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.types.xml -->
<script name="main"><text>x</text></script>`,
        "bad.types.xml": `<types name="bad"><bad/></types>`,
      }),
    "XML_TYPES_NODE_INVALID"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.types.xml -->
<script name="main"><text>x</text></script>`,
        "bad.types.xml": `<types name="bad"><type name="A"><bad/></type></types>`,
      }),
    "XML_TYPES_FIELD_INVALID"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.types.xml -->
<script name="main"><text>x</text></script>`,
        "bad.types.xml": `<types name="bad"><type name="A"><field name="x" type="number"/><field name="x" type="number"/></type></types>`,
      }),
    "TYPE_FIELD_DUPLICATE"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.types.xml -->
<script name="main"><text>x</text></script>`,
        "bad.types.xml": `<types name="bad"><type name="A"><field name="x" type="Missing"/></type></types>`,
      }),
    "TYPE_UNKNOWN"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.types.xml -->
<script name="main"><text>x</text></script>`,
        "bad.types.xml": `<types name="bad"><type name="A"><field name="x" type="A"/></type></types>`,
      }),
    "TYPE_RECURSIVE"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include:  -->
<script name="main"><text>x</text></script>`,
      }),
    "XML_INCLUDE_INVALID"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: /abs.types.xml -->
<script name="main"><text>x</text></script>`,
        "/abs.types.xml": `<types name="abs"></types>`,
      }),
    "XML_INCLUDE_INVALID"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.xml -->
<script name="main"><text>x</text></script>`,
        "bad.xml": `<invalid/>`,
      }),
    "XML_INVALID_ROOT"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<script name="main"><var name="v" type="Missing"/><text>x</text></script>`,
      }),
    "TYPE_UNKNOWN"
  );

  const compiled = compileProjectScriptsFromXmlMap({
    "main.script.xml": `<!-- include: a.script.xml -->
<!-- include: b.script.xml -->
<!-- include: shared.types.xml -->
<script name="main"><text>m</text></script>`,
    "a.script.xml": `<!-- include: shared.types.xml -->
<script name="a"><text>a</text></script>`,
    "b.script.xml": `<!-- include: shared.types.xml -->
<script name="b"><text>b</text></script>`,
    "shared.types.xml": `<types name="shared"><type name="Score"><field name="n" type="number"/></type></types>`,
  });
  assert.deepEqual(Object.keys(compiled).sort(), ["a", "b", "main"]);

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main-1.script.xml": `<script name="main"><text>a</text></script>`,
        "main-2.script.xml": `<script name="main"><text>b</text></script>`,
      }),
    "API_DUPLICATE_SCRIPT_NAME"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<types name="oops"></types>`,
      }),
    "XML_INVALID_ROOT"
  );
});

test("project compiler include source defensive branches", () => {
  const missingOnFirstRead = new Proxy(
    {
      "main.script.xml": `<script name="main"><text>x</text></script>`,
    },
    {
      get(target, prop, receiver) {
        if (prop === "main.script.xml") {
          return undefined;
        }
        return Reflect.get(target, prop, receiver);
      },
    }
  );
  expectCode(
    () =>
      compileProjectScriptsFromXmlMap(
        missingOnFirstRead as unknown as Record<string, string>
      ),
    "XML_INCLUDE_MISSING"
  );

  let reads = 0;
  const missingOnSecondRead = new Proxy(
    {
      "main.script.xml": `<script name="main"><text>x</text></script>`,
    },
    {
      get(target, prop, receiver) {
        if (prop === "main.script.xml") {
          reads += 1;
          return reads === 1 ? target["main.script.xml"] : undefined;
        }
        return Reflect.get(target, prop, receiver);
      },
    }
  );
  expectCode(
    () =>
      compileProjectScriptsFromXmlMap(
        missingOnSecondRead as unknown as Record<string, string>
      ),
    "XML_INCLUDE_MISSING"
  );

  let thirdReads = 0;
  const missingOnThirdRead = new Proxy(
    {
      "main.script.xml": `<script name="main"><text>x</text></script>`,
    },
    {
      get(target, prop, receiver) {
        if (prop === "main.script.xml") {
          thirdReads += 1;
          return thirdReads <= 2 ? target["main.script.xml"] : undefined;
        }
        return Reflect.get(target, prop, receiver);
      },
    }
  );
  expectCode(
    () =>
      compileProjectScriptsFromXmlMap(
        missingOnThirdRead as unknown as Record<string, string>
      ),
    "XML_INCLUDE_MISSING"
  );
});
