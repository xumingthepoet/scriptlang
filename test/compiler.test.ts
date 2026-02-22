import assert from "node:assert/strict";
import { test } from "vitest";

import { ScriptLangError, compileScript } from "../src/index.js";

test("compile script into implicit groups with params and var nodes", () => {
  const xml = `
<script name="main" args="seed:number,target:number:ref">
  <var name="hp" type="number" value="100"/>
  <text value="hello"/>
  <if when="hp > 0">
    <text value="alive"/>
    <else>
      <text value="dead"/>
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
    `<script name="ok" args="a:number,b:Map&lt;string,number&gt;:ref"><text value="x"/></script>`,
    "ok.script.xml"
  );
  assert.equal(ok.params[1].isRef, true);
  assert.equal(ok.params[1].type.kind, "map");

  assert.throws(() => compileScript(`<script name="x" args="bad"><text value="x"/></script>`, "bad.script.xml"));
  assert.throws(
    () => compileScript(`<script name="x" args="a:   "><text value="x"/></script>`, "bad-space.script.xml"),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "SCRIPT_ARGS_PARSE_ERROR");
      return true;
    }
  );

  assert.throws(
    () => compileScript(`<script name="x" args="a:number,a:string"><text value="x"/></script>`, "dup.script.xml"),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "SCRIPT_ARGS_DUPLICATE");
      return true;
    }
  );

  const trailingComma = compileScript(
    `<script name="x" args="a:number,"><text value="x"/></script>`,
    "trailing.script.xml"
  );
  assert.equal(trailingComma.params.length, 1);
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

test("script name is required", () => {
  assert.throws(
    () => compileScript(`<script><text value="x"/></script>`, "missing-name.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_MISSING_ATTR");
      return true;
    }
  );
});
