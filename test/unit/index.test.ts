import assert from "node:assert/strict";
import { test } from "vitest";

import {
  SCRIPT_LANG_VERSION,
  compileScript,
  createEngineFromXml,
  resumeEngineFromXml,
} from "../../src/index.js";

test("SCRIPT_LANG_VERSION is exported", () => {
  assert.equal(SCRIPT_LANG_VERSION, "0.1.0");
});

test("index exports core top-level API functions", () => {
  assert.equal(typeof compileScript, "function");
  assert.equal(typeof createEngineFromXml, "function");
  assert.equal(typeof resumeEngineFromXml, "function");
});
