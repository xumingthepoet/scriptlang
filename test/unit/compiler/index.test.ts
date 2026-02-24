import assert from "node:assert/strict";
import { test } from "vitest";

import * as compilerExports from "../../../src/compiler/index.js";

test("compiler barrel exports are available", () => {
  assert.equal(typeof compilerExports.compileScript, "function");
  assert.equal(typeof compilerExports.parseXmlDocument, "function");
});
