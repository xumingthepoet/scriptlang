import assert from "node:assert/strict";

import { test } from "vitest";

import { runPlayerCli } from "../src/cli/player.js";

test("esm exports remain available", async () => {
  const indexModule = await import("../src/index.js");
  const compilerModule = await import("../src/compiler/index.js");
  const runtimeModule = await import("../src/runtime/index.js");

  assert.equal(typeof indexModule.compileScript, "function");
  assert.equal(typeof indexModule.createEngineFromXml, "function");
  assert.equal(typeof compilerModule.parseXmlDocument, "function");
  assert.equal(typeof runtimeModule.ScriptLangEngine, "function");
});

test("player cli usage paths", async () => {
  const writes: string[] = [];
  const errWrites: string[] = [];
  const stdoutWrite = process.stdout.write.bind(process.stdout);
  const stderrWrite = process.stderr.write.bind(process.stderr);

  process.stdout.write = ((chunk: string | Uint8Array) => {
    writes.push(String(chunk));
    return true;
  }) as typeof process.stdout.write;
  process.stderr.write = ((chunk: string | Uint8Array) => {
    errWrites.push(String(chunk));
    return true;
  }) as typeof process.stderr.write;

  try {
    const helpCode = await runPlayerCli([]);
    assert.equal(helpCode, 0);

    const unknownCode = await runPlayerCli(["bad-mode"]);
    assert.equal(unknownCode, 1);

    const agentListCode = await runPlayerCli(["agent", "list"]);
    assert.equal(agentListCode, 0);

    const tuiBadCode = await runPlayerCli(["tui"]);
    assert.equal(tuiBadCode, 1);

    assert.ok(writes.join("").includes("scriptlang-player"));
    assert.ok(errWrites.join("").includes("Unknown mode: bad-mode"));
  } finally {
    process.stdout.write = stdoutWrite;
    process.stderr.write = stderrWrite;
  }
});
