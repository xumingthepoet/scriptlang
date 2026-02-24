import assert from "node:assert/strict";
import { test, vi } from "vitest";

import {
  ScriptLangError,
  compileProjectBundleFromXmlMap,
  compileProjectScriptsFromXmlMap,
  compileScript,
} from "../../../src/index.js";

test("compile script into implicit groups with params and var nodes", () => {
  const xml = `
<script name="main" args="number:seed,ref:number:target">
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
  <choice text="Choose">
    <option text="Go" when="hp > 0">
      <call script="next" args="hp"/>
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
    `<script name="ok" args="number:a,ref:Map&lt;string,number&gt;:b"><text>x</text></script>`,
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
    () => compileScript(`<script name="x" args="number:   "><text>x</text></script>`, "bad-empty-name.script.xml"),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "SCRIPT_ARGS_PARSE_ERROR");
      return true;
    }
  );

  assert.throws(
    () => compileScript(`<script name="x" args="number:a,string:a"><text>x</text></script>`, "dup.script.xml"),
    (error: unknown) => {
      assert.ok(error instanceof ScriptLangError);
      assert.equal(error.code, "SCRIPT_ARGS_DUPLICATE");
      return true;
    }
  );

  const trailingComma = compileScript(
    `<script name="x" args="number:a,"><text>x</text></script>`,
    "trailing.script.xml"
  );
  assert.equal(trailingComma.params.length, 1);

  assert.throws(
    () =>
      compileScript(
        `<script name="x" args="Record&lt;string,number&gt;:r"><text>x</text></script>`,
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
        `<script name="x" args="null:n"><text>x</text></script>`,
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
    `<script name="x"><call script="target" args="&quot;a,b&quot;"/></script>`,
    "quoted.script.xml"
  );
  const quotedCall = quoted.groups[quoted.rootGroupId].nodes[0];
  assert.equal(quotedCall.kind, "call");
  assert.equal(quotedCall.args.length, 1);

  const nested = compileScript(
    `<script name="x"><call script="target" args="({k:[1,2]}),(1,2),'x,y'"/></script>`,
    "nested.script.xml"
  );
  const nestedCall = nested.groups[nested.rootGroupId].nodes[0];
  assert.equal(nestedCall.kind, "call");
  assert.equal(nestedCall.args.length, 3);

  assert.throws(
    () =>
      compileScript(
        `<script name="x"><call script="target" args="ref:"/></script>`,
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
        `<script name="x"><call script="target" args="ref: "/></script>`,
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
        `<script name="x"><call script="target" args="ref:,1"/></script>`,
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

test("return args require script attribute", () => {
  assert.throws(
    () => compileScript(`<script name="x"><return args="1"/></script>`, "return-args.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_RETURN_ARGS_WITHOUT_TARGET");
      return true;
    }
  );
});

test("return args reject ref mode", () => {
  assert.throws(
    () =>
      compileScript(
        `<script name="x"><return script="next" args="ref:hp"/></script>`,
        "return-ref-args.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_RETURN_REF_UNSUPPORTED");
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

test("choice requires prompt text attribute and keeps raw template source", () => {
  const ir = compileScript(
    `
<script name="main">
  <choice text="pick \${1 + 1}">
    <option text="ok"><text>done</text></option>
  </choice>
</script>
`,
    "choice-prompt.script.xml"
  );
  const node = ir.groups[ir.rootGroupId].nodes[0];
  assert.equal(node.kind, "choice");
  assert.equal(node.promptText, "pick ${1 + 1}");
});

test("choice rejects missing prompt text attribute", () => {
  assert.throws(
    () =>
      compileScript(
        `<script name="main"><choice><option text="ok"><text>x</text></option></choice></script>`,
        "choice-prompt-missing.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_MISSING_ATTR");
      return true;
    }
  );
});

test("choice prompt text rejects empty attribute", () => {
  assert.throws(
    () =>
      compileScript(
        `<script name="main"><choice text="   "><option text="ok"><text>x</text></option></choice></script>`,
        "choice-prompt-empty.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_EMPTY_ATTR");
      return true;
    }
  );
});

test("input node parses and validates var/text constraints", () => {
  const ir = compileScript(
    `<script name="main"><var name="name" type="string" value="'A'"/><input var="name" text="Name hero"/></script>`,
    "input-ok.script.xml"
  );
  const node = ir.groups[ir.rootGroupId].nodes[1];
  assert.equal(node.kind, "input");
  assert.equal(node.targetVar, "name");
  assert.equal(node.promptText, "Name hero");

  expectCode(
    () => compileScript(`<script name="main"><input text="x"/></script>`, "input-missing-var.script.xml"),
    "XML_MISSING_ATTR"
  );
  expectCode(
    () => compileScript(`<script name="main"><var name="name" type="string"/><input var="name"/></script>`, "input-missing-text.script.xml"),
    "XML_MISSING_ATTR"
  );
  expectCode(
    () => compileScript(`<script name="main"><var name="name" type="string"/><input var="name" text="  "/></script>`, "input-empty-text.script.xml"),
    "XML_EMPTY_ATTR"
  );
  expectCode(
    () =>
      compileScript(
        `<script name="main"><input var="missing" text="x"/></script>`,
        "input-missing-scope.script.xml"
      ),
    "XML_INPUT_VAR_UNKNOWN"
  );
  expectCode(
    () =>
      compileScript(
        `<script name="main"><var name="hp" type="number" value="1"/><input var="hp" text="x"/></script>`,
        "input-var-type.script.xml"
      ),
    "XML_INPUT_VAR_TYPE"
  );
  expectCode(
    () =>
      compileScript(
        `<script name="main"><var name="name" type="string"/><input var="name" text="name \${x}"/></script>`,
        "input-template.script.xml"
      ),
    "XML_INPUT_TEMPLATE_UNSUPPORTED"
  );
  expectCode(
    () =>
      compileScript(
        `<script name="main"><var name="name" type="string"/><input var="name" text="x" default="y"/></script>`,
        "input-default-attr.script.xml"
      ),
    "XML_ATTR_NOT_ALLOWED"
  );
  expectCode(
    () =>
      compileScript(
        `<script name="main"><var name="name" type="string"/><input var="name" text="x"><text>bad</text></input></script>`,
        "input-child-node.script.xml"
      ),
    "XML_INPUT_CHILD_INVALID"
  );
  expectCode(
    () =>
      compileScript(
        `<script name="main"><var name="name" type="string"/><input var="name" text="x">bad</input></script>`,
        "input-inline-text.script.xml"
      ),
    "XML_INPUT_CHILD_INVALID"
  );
});

test("text once attribute is parsed", () => {
  const ir = compileScript(`<script name="main"><text once="true">intro</text></script>`, "text-once.script.xml");
  const node = ir.groups[ir.rootGroupId].nodes[0];
  assert.equal(node.kind, "text");
  assert.equal(node.once, true);

  const irFalse = compileScript(`<script name="main"><text once="false">intro</text></script>`, "text-once-false.script.xml");
  const nodeFalse = irFalse.groups[irFalse.rootGroupId].nodes[0];
  assert.equal(nodeFalse.kind, "text");
  assert.equal(nodeFalse.once, false);
});

test("choice option once/fall_over and direct continue are parsed", () => {
  const ir = compileScript(
    `
<script name="main">
  <choice text="Pick">
    <option text="A" once="true">
      <continue/>
    </option>
    <option text="B" fall_over="true">
      <text>end</text>
    </option>
  </choice>
</script>
`,
    "choice-attrs.script.xml"
  );
  const node = ir.groups[ir.rootGroupId].nodes[0];
  assert.equal(node.kind, "choice");
  assert.equal(node.options[0].once, true);
  assert.equal(node.options[0].fallOver, false);
  assert.equal(node.options[1].fallOver, true);
  const optionGroup = ir.groups[node.options[0].groupId];
  assert.equal(optionGroup.nodes[0]?.kind, "continue");
  if (optionGroup.nodes[0]?.kind === "continue") {
    assert.equal(optionGroup.nodes[0].target, "choice");
  }
});

test("choice fall_over validation rejects duplicate/non-last/when", () => {
  assert.throws(
    () =>
      compileScript(
        `<script name="main"><choice text="x"><option text="a" fall_over="true"/><option text="b" fall_over="true"/></choice></script>`,
        "fall-over-dup.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_OPTION_FALL_OVER_DUPLICATE");
      return true;
    }
  );
  assert.throws(
    () =>
      compileScript(
        `<script name="main"><choice text="x"><option text="a" fall_over="true"/><option text="b"/></choice></script>`,
        "fall-over-order.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_OPTION_FALL_OVER_NOT_LAST");
      return true;
    }
  );
  assert.throws(
    () =>
      compileScript(
        `<script name="main"><choice text="x"><option text="a"/><option text="b" fall_over="true" when="true"/></choice></script>`,
        "fall-over-when.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_OPTION_FALL_OVER_WHEN_FORBIDDEN");
      return true;
    }
  );
});

test("once/fall_over boolean attrs reject invalid literals", () => {
  assert.throws(
    () => compileScript(`<script name="main"><text once="yes">x</text></script>`, "text-once-bool.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_ATTR_BOOL_INVALID");
      return true;
    }
  );
  assert.throws(
    () =>
      compileScript(
        `<script name="main"><choice text="x"><option text="a" fall_over="1"/></choice></script>`,
        "option-fall-over-bool.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_ATTR_BOOL_INVALID");
      return true;
    }
  );
});

test("once attribute is rejected outside text/option", () => {
  assert.throws(
    () => compileScript(`<script name="main"><var name="x" type="number" value="1" once="true"/></script>`, "once-var.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_ATTR_NOT_ALLOWED");
      return true;
    }
  );
});

test("break/continue placement is validated at compile time", () => {
  assert.throws(
    () => compileScript(`<script name="main"><break/></script>`, "break-invalid.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_BREAK_OUTSIDE_WHILE");
      return true;
    }
  );
  assert.throws(
    () => compileScript(`<script name="main"><continue/></script>`, "continue-invalid.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_CONTINUE_OUTSIDE_WHILE_OR_OPTION");
      return true;
    }
  );
  const ir = compileScript(
    `<script name="main"><while when="true"><if when="true"><continue/></if></while></script>`,
    "continue-while.script.xml"
  );
  const whileNode = ir.groups[ir.rootGroupId].nodes[0];
  assert.equal(whileNode.kind, "while");
  const whileGroup = ir.groups[whileNode.bodyGroupId];
  const ifNode = whileGroup.nodes[0];
  assert.equal(ifNode.kind, "if");
  if (ifNode.kind === "if") {
    const thenGroup = ir.groups[ifNode.thenGroupId];
    const continueNode = thenGroup.nodes[0];
    assert.equal(continueNode.kind, "continue");
    if (continueNode.kind === "continue") {
      assert.equal(continueNode.target, "while");
    }
  }
});

test("loop macro expands into var + while + decrement code", () => {
  const ir = compileScript(
    `
<script name="main">
  <loop times="3">
    <text>x</text>
  </loop>
</script>
`,
    "loop-expand.script.xml"
  );
  const rootNodes = ir.groups[ir.rootGroupId].nodes;
  assert.equal(rootNodes[0]?.kind, "var");
  assert.equal(rootNodes[1]?.kind, "while");
  if (rootNodes[0]?.kind === "var") {
    assert.equal(rootNodes[0].declaration.type.kind, "primitive");
    assert.equal(rootNodes[0].declaration.initialValueExpr, "3");
    assert.match(rootNodes[0].declaration.name, /^__sl_loop_\d+_remaining$/);
  }
  if (rootNodes[1]?.kind === "while" && rootNodes[0]?.kind === "var") {
    assert.equal(rootNodes[1].whenExpr, `${rootNodes[0].declaration.name} > 0`);
    const body = ir.groups[rootNodes[1].bodyGroupId];
    assert.equal(body.nodes[0]?.kind, "code");
    assert.equal(body.nodes[1]?.kind, "text");
  }
});

test("reserved __ prefix is rejected for script/arg/var names", () => {
  assert.throws(
    () => compileScript(`<script name="__main"><text>x</text></script>`, "reserved-script-name.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "NAME_RESERVED_PREFIX");
      return true;
    }
  );
  assert.throws(
    () => compileScript(`<script name="main" args="number:__x"><text>x</text></script>`, "reserved-arg-name.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "NAME_RESERVED_PREFIX");
      return true;
    }
  );
  assert.throws(
    () => compileScript(`<script name="main"><var name="__x" type="number" value="1"/></script>`, "reserved-var-name.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "NAME_RESERVED_PREFIX");
      return true;
    }
  );
});

test("reserved __ prefix is rejected for types/fields/json symbols", () => {
  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="__meta"><type name="A"><field name="x" type="number"/></type></defs>`,
      }),
    "NAME_RESERVED_PREFIX"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="meta"><type name="__A"><field name="x" type="number"/></type></defs>`,
      }),
    "NAME_RESERVED_PREFIX"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="meta"><type name="A"><field name="__x" type="number"/></type></defs>`,
      }),
    "NAME_RESERVED_PREFIX"
  );

  expectCode(
    () =>
      compileProjectBundleFromXmlMap({
        "main.script.xml": `<!-- include: __game.json -->
<script name="main"><text>x</text></script>`,
        "__game.json": `{"x":1}`,
      }),
    "NAME_RESERVED_PREFIX"
  );
});

test("loop times rejects ${...} wrapper form", () => {
  assert.throws(
    () =>
      compileScript(
        `<script name="main"><loop times="\${n}"><text>x</text></loop></script>`,
        "loop-template-times.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_LOOP_TIMES_TEMPLATE_UNSUPPORTED");
      return true;
    }
  );
});

test("loop times requires non-empty attribute", () => {
  assert.throws(
    () => compileScript(`<script name="main"><loop><text>x</text></loop></script>`, "loop-missing-times.script.xml"),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_MISSING_ATTR");
      return true;
    }
  );
  assert.throws(
    () =>
      compileScript(
        `<script name="main"><loop times="   "><text>x</text></loop></script>`,
        "loop-empty-times.script.xml"
      ),
    (e: unknown) => {
      assert.ok(e instanceof ScriptLangError);
      assert.equal(e.code, "XML_EMPTY_ATTR");
      return true;
    }
  );
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

test("project compiler validates defs include graph and script type references", () => {
  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><bad/></defs>`,
      }),
    "XML_DEFS_NODE_INVALID"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><type name="A"><bad/></type></defs>`,
      }),
    "XML_DEFS_FIELD_INVALID"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><type name="A"><field name="x" type="number"/><field name="x" type="number"/></type></defs>`,
      }),
    "TYPE_FIELD_DUPLICATE"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><type name="A"><field name="x" type="Missing"/></type></defs>`,
      }),
    "TYPE_UNKNOWN"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><type name="A"><field name="x" type="A"/></type></defs>`,
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
        "main.script.xml": `<!-- include: /abs.defs.xml -->
<script name="main"><text>x</text></script>`,
        "/abs.defs.xml": `<defs name="abs"></defs>`,
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

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: battle.script.xml -->
<!-- include: actors.defs.xml -->
<script name="main"><text>m</text></script>`,
        "battle.script.xml": `<script name="battle" args="Combatant:actor"><text>b</text></script>`,
        "actors.defs.xml": `<defs name="actors"><type name="Combatant"><field name="hp" type="number"/></type></defs>`,
      }),
    "TYPE_UNKNOWN"
  );

  const compiled = compileProjectScriptsFromXmlMap({
    "main.script.xml": `<!-- include: a.script.xml -->
<!-- include: b.script.xml -->
<!-- include: shared.defs.xml -->
<script name="main"><text>m</text></script>`,
    "a.script.xml": `<!-- include: shared.defs.xml -->
<script name="a"><text>a</text></script>`,
    "b.script.xml": `<!-- include: shared.defs.xml -->
<script name="b"><text>b</text></script>`,
    "shared.defs.xml": `<defs name="shared"><type name="Score"><field name="n" type="number"/></type></defs>`,
  });
  assert.deepEqual(Object.keys(compiled).sort(), ["a", "b", "main"]);

  const compiledWithScopedTypes = compileProjectScriptsFromXmlMap({
    "main.script.xml": `<!-- include: battle.script.xml -->
<!-- include: actors.defs.xml -->
<script name="main"><text>m</text></script>`,
    "battle.script.xml": `<!-- include: actors.defs.xml -->
<script name="battle" args="Combatant:actor"><text>b</text></script>`,
    "actors.defs.xml": `<defs name="actors"><type name="Combatant"><field name="hp" type="number"/></type></defs>`,
  });
  assert.deepEqual(Object.keys(compiledWithScopedTypes).sort(), ["battle", "main"]);

  const compiledWithoutMain = compileProjectScriptsFromXmlMap({
    "alt.script.xml": `<script name="alt"><text>x</text></script>`,
  });
  assert.deepEqual(Object.keys(compiledWithoutMain).sort(), ["alt"]);

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<script name="main"><text>x</text></script>`,
        "extra.script.xml": `<!-- include: missing.script.xml -->
<script name="extra"><text>x</text></script>`,
      }),
    "XML_INCLUDE_MISSING"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<script name="main"><text>x</text></script>`,
        "a.script.xml": `<!-- include: b.script.xml -->
<script name="a"><text>a</text></script>`,
        "b.script.xml": `<!-- include: a.script.xml -->
<script name="b"><text>b</text></script>`,
      }),
    "XML_INCLUDE_CYCLE"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main-1.script.xml": `<script name="main"><text>a</text></script>`,
        "main-2.script.xml": `<script name="main"><text>b</text></script>`,
      }),
    "API_DUPLICATE_SCRIPT_NAME"
  );

  const defsOnly = compileProjectScriptsFromXmlMap({
    "main.script.xml": `<defs name="oops"></defs>`,
  });
  assert.deepEqual(defsOnly, {});

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: old.types.xml -->
<script name="main"><text>x</text></script>`,
        "old.types.xml": `<types name="legacy"></types>`,
      }),
    "XML_INVALID_ROOT"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<script name="main" args="oops"><text>x</text></script>`,
      }),
    "SCRIPT_ARGS_PARSE_ERROR"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<script name="main" args="number:a,number:a"><text>x</text></script>`,
      }),
    "SCRIPT_ARGS_DUPLICATE"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<script name="main"><var name="" type="number" value="1"/><text>x</text></script>`,
      }),
    "XML_MISSING_ATTR"
  );
});

test("defs function declarations parse, resolve, and validate conflicts", () => {
  const compiled = compileProjectScriptsFromXmlMap({
    "main.script.xml": `<!-- include: shared.defs.xml -->
<script name="main" args="number:v">
  <var name="sum" type="number" value="add(v, { value: 1 })"/>
  <text>\${add(sum, { value: 2 })}</text>
</script>`,
    "shared.defs.xml": `<defs name="shared">
  <type name="CustomType"><field name="value" type="number"/></type>
  <function name="add" args="number:a,CustomType:b" return="number:r">
    r = a + b.value;
  </function>
</defs>`,
  });
  const main = compiled.main;
  assert.ok(main.visibleFunctions);
  assert.equal(main.visibleFunctions?.add?.name, "add");
  assert.equal(main.visibleFunctions?.add?.params.length, 2);
  assert.equal(main.visibleFunctions?.add?.returnBinding.name, "r");

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><function name="f" args="ref:number:a" return="number:r">r = a;</function></defs>`,
      }),
    "XML_FUNCTION_ARGS_REF_UNSUPPORTED"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><function name="f" args="number:a,number:a" return="number:r">r = a;</function></defs>`,
      }),
    "FUNCTION_ARGS_DUPLICATE"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><function name="f" args="oops" return="number:r">r = 1;</function></defs>`,
      }),
    "FUNCTION_ARGS_PARSE_ERROR"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><function name="f" return="ref:number:r">r = 1;</function></defs>`,
      }),
    "XML_FUNCTION_RETURN_REF_UNSUPPORTED"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><function name="f" return="bad">const x = 1;</function></defs>`,
      }),
    "FUNCTION_RETURN_PARSE_ERROR"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><function name="f" args="number:a" return="number:a">a = 1;</function></defs>`,
      }),
    "FUNCTION_RETURN_NAME_CONFLICT"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><function name="f" return="number:r"><text>x</text></function></defs>`,
      }),
    "XML_FUNCTION_CHILD_NODE_INVALID"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><function name="f" return="number:r">   </function></defs>`,
      }),
    "XML_EMPTY_NODE_CONTENT"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="bad"><function name="f" return="number:r" value="1">r = 1;</function></defs>`,
      }),
    "XML_ATTR_NOT_ALLOWED"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad-a.defs.xml -->
<!-- include: bad-b.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad-a.defs.xml": `<defs name="a"><function name="f" return="number:r">r = 1;</function></defs>`,
        "bad-b.defs.xml": `<defs name="b"><function name="f" return="number:r">r = 2;</function></defs>`,
      }),
    "FUNCTION_DECL_DUPLICATE_VISIBLE"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><var name="f" type="number" value="1"/><text>x</text></script>`,
        "bad.defs.xml": `<defs name="a"><function name="f" return="number:r">r = 1;</function></defs>`,
      }),
    "FUNCTION_NAME_CONFLICT_SCRIPT_SYMBOL"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: game.json -->
<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "game.json": `{"n":1}`,
        "bad.defs.xml": `<defs name="a"><function name="game" return="number:r">r = 1;</function></defs>`,
      }),
    "FUNCTION_NAME_CONFLICT_JSON_SYMBOL"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="a"><function name="random" return="number:r">r = 1;</function></defs>`,
      }),
    "FUNCTION_NAME_CONFLICT_BUILTIN"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="a"><function name="f" args="number:g" return="number:r">r = g;</function><function name="g" return="number:r">r = 1;</function></defs>`,
      }),
    "FUNCTION_LOCAL_NAME_CONFLICT"
  );

  expectCode(
    () =>
      compileProjectScriptsFromXmlMap({
        "main.script.xml": `<!-- include: bad.defs.xml -->
<script name="main"><text>x</text></script>`,
        "bad.defs.xml": `<defs name="a"><function name="f" return="number:g">g = 1;</function><function name="g" return="number:r">r = 1;</function></defs>`,
      }),
    "FUNCTION_LOCAL_NAME_CONFLICT"
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

test("project compiler loads JSON globals and validates symbol rules", () => {
  const bundled = compileProjectBundleFromXmlMap({
    "main.script.xml": `<!-- include: game.json -->
<!-- include: child.script.xml -->
<script name="main"><text>\${game.player.name}</text></script>`,
    "child.script.xml": `<script name="child"><text>x</text></script>`,
    "game.json": `{"player":{"name":"Hero","stats":{"hp":12}}}`,
  });
  assert.equal(bundled.globalJson.game !== undefined, true);
  assert.equal(
    (
      bundled.globalJson.game as {
        player: { name: string };
      }
    ).player.name,
    "Hero"
  );
  assert.deepEqual(bundled.scripts.main.visibleJsonGlobals, ["game"]);
  assert.deepEqual(bundled.scripts.child.visibleJsonGlobals, []);

  expectCode(
    () =>
      compileProjectBundleFromXmlMap({
        "main.script.xml": `<!-- include: bad.json -->
<script name="main"><text>x</text></script>`,
        "bad.json": `{"x": }`,
      }),
    "JSON_PARSE_ERROR"
  );

  const parseSpy = vi.spyOn(JSON, "parse").mockImplementationOnce(() => {
    throw "boom";
  });
  try {
    expectCode(
      () =>
        compileProjectBundleFromXmlMap({
          "main.script.xml": `<!-- include: bad.json -->
<script name="main"><text>x</text></script>`,
          "bad.json": `{"x":1}`,
        }),
      "JSON_PARSE_ERROR"
    );
  } finally {
    parseSpy.mockRestore();
  }

  expectCode(
    () =>
      compileProjectBundleFromXmlMap({
        "main.script.xml": `<!-- include: game-data.json -->
<script name="main"><text>x</text></script>`,
        "game-data.json": `{"x":1}`,
      }),
    "JSON_SYMBOL_INVALID"
  );

  expectCode(
    () =>
      compileProjectBundleFromXmlMap({
        "main.script.xml": `<!-- include: a/config.json -->
<!-- include: b/config.json -->
<script name="main"><text>x</text></script>`,
        "a/config.json": `{"x":1}`,
        "b/config.json": `{"x":2}`,
      }),
    "JSON_SYMBOL_DUPLICATE"
  );
});

test("compiler defensive validation branches", () => {
  expectCode(() => compileScript("<nope/>", "a.script.xml"), "XML_INVALID_ROOT");
  expectCode(
    () =>
      compileScript(
        `<script name="a.script.xml"><choice text="Choose"><bad/></choice></script>`,
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
        `<script name="a.script.xml"><var name="" type="number" value="1"/></script>`,
        "a.script.xml"
      ),
    "XML_MISSING_ATTR"
  );
  expectCode(
    () => compileScript(`<script name="a.script.xml"><text once="1">x</text></script>`, "a.script.xml"),
    "XML_ATTR_BOOL_INVALID"
  );
});
