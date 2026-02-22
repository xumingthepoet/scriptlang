import assert from "node:assert/strict";
import test from "node:test";

import { SCRIPT_LANG_VERSION } from "../src/index";

test("exports version", () => {
  assert.equal(SCRIPT_LANG_VERSION, "0.1.0");
});

