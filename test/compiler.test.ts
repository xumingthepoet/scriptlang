import assert from "node:assert/strict";
import test from "node:test";

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
