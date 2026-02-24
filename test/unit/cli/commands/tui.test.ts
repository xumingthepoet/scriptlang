import assert from "node:assert/strict";

import { test } from "vitest";

import { DEFAULT_STATE_FILE, parseTuiArgs, runTuiCommand } from "../../../../src/cli/commands/tui.js";

test("parseTuiArgs parses scripts-dir and optional state-file", () => {
  const parsed = parseTuiArgs(["--scripts-dir", "/tmp/demo"]);
  assert.equal(parsed.scriptsDir, "/tmp/demo");
  assert.equal(parsed.stateFile, DEFAULT_STATE_FILE);

  const withState = parseTuiArgs(["--scripts-dir", "/tmp/demo", "--state-file", "/tmp/s.bin"]);
  assert.equal(withState.scriptsDir, "/tmp/demo");
  assert.equal(withState.stateFile, "/tmp/s.bin");
});

test("parseTuiArgs validates required and unknown args", () => {
  assert.throws(() => parseTuiArgs([]));
  assert.throws(() => parseTuiArgs(["--scripts-dir"]));
  assert.throws(() => parseTuiArgs(["--scripts-dir", "/tmp/demo", "--unknown", "x"]));
  assert.throws(() => parseTuiArgs(["--scripts-dir", "/tmp/demo", "--state-file"]));
});

test("runTuiCommand returns non-zero on argument errors", async () => {
  const errWrites: string[] = [];
  const stderrWrite = process.stderr.write.bind(process.stderr);
  process.stderr.write = ((chunk: string | Uint8Array) => {
    errWrites.push(String(chunk));
    return true;
  }) as typeof process.stderr.write;
  try {
    const code = await runTuiCommand([]);
    assert.equal(code, 1);
    assert.ok(errWrites.join("").includes("Missing source selector"));
  } finally {
    process.stderr.write = stderrWrite;
  }
});
