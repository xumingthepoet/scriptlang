import assert from "node:assert/strict";
import { test } from "vitest";

import * as runtimeExports from "../../../src/runtime/index.js";

test("runtime barrel exports are available", () => {
  assert.equal(typeof runtimeExports.ScriptLangEngine, "function");
});
