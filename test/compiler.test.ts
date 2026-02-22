import assert from "node:assert/strict";
import { test } from "vitest";

import { ScriptLangError, compileScript } from "../src";

test("compile script into implicit groups", () => {
  const xml = `
<script name="main.script.xml">
  <vars>
    <var name="hp" type="number" value="100"/>
    <var name="flags" type="Record&lt;string,boolean&gt;" value="{}"/>
  </vars>
  <step>
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
        <call script="next.script.xml" args="value:hp,target:ref:hp"/>
      </option>
    </choice>
  </step>
</script>
`;
  const ir = compileScript(xml, "main.script.xml");

  assert.equal(ir.rootGroupId, "main.script.xml::g0");
  assert.equal(ir.vars.length, 2);
  assert.ok(Object.keys(ir.groups).length >= 5);
  assert.equal(ir.groups[ir.rootGroupId].nodes[0]?.kind, "text");
  assert.equal(ir.groups[ir.rootGroupId].nodes[1]?.kind, "if");
  assert.equal(ir.groups[ir.rootGroupId].nodes[2]?.kind, "while");
  assert.equal(ir.groups[ir.rootGroupId].nodes[3]?.kind, "choice");
});

test("reject removed mutation nodes", () => {
  const xml = `
<script>
  <step>
    <set target="hp" value="1"/>
  </step>
</script>
`;
  assert.throws(
    () => compileScript(xml, "broken.script.xml"),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "XML_UNSUPPORTED_NODE");
      return true;
    }
  );
});

test("reject duplicate var declarations", () => {
  const xml = `
<script>
  <vars>
    <var name="hp" type="number" value="1"/>
    <var name="hp" type="number" value="2"/>
  </vars>
  <step />
</script>
`;
  assert.throws(() => compileScript(xml, "dup.script.xml"));
});

test("compile supports map type and script without vars", () => {
  const mapType = compileScript(
    `
<script>
  <vars>
    <var name="m" type="Map&lt;string,number&gt;"/>
  </vars>
  <step>
    <text value="ok"/>
  </step>
</script>
`,
    "map.script.xml"
  );
  assert.equal(mapType.vars[0].type.kind, "map");

  const noVars = compileScript(
    `
<script>
  <step>
    <text value="x"/>
  </step>
</script>
`,
    "novars.script.xml"
  );
  assert.equal(noVars.vars.length, 0);

  const arrayType = compileScript(
    `
<script>
  <vars>
    <var name="nums" type="number[]"/>
  </vars>
  <step />
</script>
`,
    "array.script.xml"
  );
  assert.equal(arrayType.vars[0].type.kind, "array");

  const noStep = compileScript(
    `
<script>
  <vars>
    <var name="hp" type="number" value="1"/>
  </vars>
</script>
`,
    "nostep.script.xml"
  );
  assert.equal(noStep.groups[noStep.rootGroupId].nodes.length, 0);
});

test("call args parser separator edge cases", () => {
  assert.throws(
    () =>
      compileScript(
        `<script><step><call script="x.script.xml" args="a:"/></step></script>`,
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
        `<script><step><call script="x.script.xml" args="a: "/></step></script>`,
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
        `<script><step><call script="x.script.xml" args="&#32;:x"/></step></script>`,
        "badargs3.script.xml"
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
        `<script><step><call script="x.script.xml" args="a:&#10;"/></step></script>`,
        "badargs4.script.xml"
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
    () => compileScript(`<script><step><call script=""/></step></script>`, "empty-attr.script.xml"),
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
<script>
  <step>
    <text>inline value</text>
  </step>
</script>
`,
    "inline-text.script.xml"
  );
  const node = ir.groups[ir.rootGroupId].nodes[0];
  assert.equal(node.kind, "text");
  assert.equal(node.value, "inline value");
});
