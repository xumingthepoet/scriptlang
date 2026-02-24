import assert from "node:assert/strict";
import { test } from "vitest";

import { ScriptLangError } from "../../../src/core/errors.js";

test("ScriptLangError carries code and span", () => {
  const span = {
    start: { line: 1, column: 1 },
    end: { line: 1, column: 2 },
  };
  const error = new ScriptLangError("X_CODE", "boom", span);
  assert.equal(error.name, "ScriptLangError");
  assert.equal(error.code, "X_CODE");
  assert.equal(error.message, "boom");
  assert.deepEqual(error.span, span);
});
