import assert from "node:assert/strict";
import { test } from "vitest";

import * as compilerExports from "../src/compiler";
import { SCRIPT_LANG_VERSION } from "../src/index";
import * as runtimeExports from "../src/runtime";

test("exports version", () => {
  assert.equal(SCRIPT_LANG_VERSION, "0.1.0");
});

test("barrel exports are available", () => {
  assert.equal(typeof compilerExports.compileScript, "function");
  assert.equal(typeof compilerExports.parseXmlDocument, "function");
  assert.equal(typeof runtimeExports.ScriptLangEngine, "function");
});
