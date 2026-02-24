import assert from "node:assert/strict";
import { test } from "vitest";

import { expandScriptMacros } from "../../../src/compiler/macros.js";
import { parseXmlDocument } from "../../../src/compiler/xml.js";

test("loop macro expands into var + while + decrement code", () => {
  const root = parseXmlDocument(
    `<script name="main"><loop times="3"><text>x</text></loop></script>`
  ).root;
  const expanded = expandScriptMacros(root, { reservedVarNames: [] });
  const elements = expanded.children.filter((node) => node.kind === "element");

  assert.equal(elements[0]?.name, "var");
  assert.equal(elements[1]?.name, "while");
  if (elements[0]?.name === "var") {
    assert.match(elements[0].attributes.name ?? "", /^__sl_loop_\d+_remaining$/);
    assert.equal(elements[0].attributes.value, "3");
  }
});

test("loop macro avoids temp name collisions with user vars", () => {
  const root = parseXmlDocument(
    `
<script name="main">
  <var name="__sl_loop_0_remaining" type="number" value="99"/>
  <loop times="2"><text>x</text></loop>
</script>
`
  ).root;
  const expanded = expandScriptMacros(root, { reservedVarNames: [] });
  const elements = expanded.children.filter((node) => node.kind === "element");

  assert.equal(elements[0]?.name, "var");
  assert.equal(elements[1]?.name, "var");
  assert.equal(elements[0]?.attributes.name, "__sl_loop_0_remaining");
  assert.notEqual(elements[1]?.attributes.name, "__sl_loop_0_remaining");
});
