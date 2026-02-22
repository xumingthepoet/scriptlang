import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { test } from "vitest";

import { runAgentCommand } from "../src/cli/commands/agent.js";

const runWithCapture = (argv: string[]) => {
  const lines: string[] = [];
  const code = runAgentCommand(argv, (line) => lines.push(line));
  return { code, lines };
};

test("agent list returns scenario rows", () => {
  const result = runWithCapture(["list"]);
  assert.equal(result.code, 0);
  assert.equal(result.lines[0], "RESULT:OK");
  assert.equal(result.lines[1], "EVENT:TEXT");
  assert.equal(result.lines[result.lines.length - 1], "STATE_OUT:NONE");
  assert.ok(result.lines.some((line) => line.includes("01-text-code")));
});

test("agent start emits choices and writes state", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-start-"));
  const stateOut = path.join(dir, "state.bin");

  const result = runWithCapture(["start", "--example", "06-snapshot-flow", "--state-out", stateOut]);
  assert.equal(result.code, 0);
  assert.equal(result.lines[0], "RESULT:OK");
  assert.ok(result.lines.includes("EVENT:CHOICES"));
  assert.ok(result.lines.some((line) => line.startsWith("TEXT_JSON:")));
  assert.ok(result.lines.some((line) => line.startsWith("CHOICE:0|")));
  assert.equal(result.lines[result.lines.length - 1], `STATE_OUT:${stateOut}`);
  assert.equal(fs.existsSync(stateOut), true);
});

test("agent start can finish without state file", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-end-"));
  const stateOut = path.join(dir, "state.bin");

  const result = runWithCapture(["start", "--example", "01-text-code", "--state-out", stateOut]);
  assert.equal(result.code, 0);
  assert.ok(result.lines.includes("EVENT:END"));
  assert.equal(result.lines[result.lines.length - 1], "STATE_OUT:NONE");
  assert.equal(fs.existsSync(stateOut), false);
});

test("agent choose can continue to next choices and to end", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-choose-"));
  const firstState = path.join(dir, "first.bin");
  const secondState = path.join(dir, "second.bin");
  const thirdState = path.join(dir, "third.bin");

  const start = runWithCapture(["start", "--example", "03-choice-once", "--state-out", firstState]);
  assert.equal(start.code, 0);
  assert.ok(start.lines.includes("EVENT:CHOICES"));

  const chooseFirst = runWithCapture([
    "choose",
    "--state-in",
    firstState,
    "--choice",
    "0",
    "--state-out",
    secondState,
  ]);
  assert.equal(chooseFirst.code, 0);
  assert.ok(chooseFirst.lines.includes("EVENT:CHOICES"));
  assert.equal(chooseFirst.lines[chooseFirst.lines.length - 1], `STATE_OUT:${secondState}`);

  const chooseSecond = runWithCapture([
    "choose",
    "--state-in",
    secondState,
    "--choice",
    "0",
    "--state-out",
    thirdState,
  ]);
  assert.equal(chooseSecond.code, 0);
  assert.ok(chooseSecond.lines.includes("EVENT:END"));
  assert.equal(chooseSecond.lines[chooseSecond.lines.length - 1], "STATE_OUT:NONE");
});

test("agent error protocol paths", () => {
  const missingSubcommand = runWithCapture([]);
  assert.equal(missingSubcommand.code, 1);
  assert.equal(missingSubcommand.lines[0], "RESULT:ERROR");
  assert.ok(missingSubcommand.lines.some((line) => line.startsWith("ERROR_CODE:CLI_AGENT_USAGE")));

  const unknownSubcommand = runWithCapture(["unknown"]);
  assert.equal(unknownSubcommand.code, 1);
  assert.ok(unknownSubcommand.lines.some((line) => line.startsWith("ERROR_CODE:CLI_AGENT_USAGE")));

  const badArgFormat = runWithCapture(["start", "example", "06-snapshot-flow"]);
  assert.equal(badArgFormat.code, 1);
  assert.ok(badArgFormat.lines.some((line) => line.startsWith("ERROR_CODE:CLI_ARG_FORMAT")));

  const missingArgValue = runWithCapture(["start", "--example"]);
  assert.equal(missingArgValue.code, 1);
  assert.ok(missingArgValue.lines.some((line) => line.startsWith("ERROR_CODE:CLI_ARG_MISSING")));

  const missingRequiredArg = runWithCapture(["start", "--example", "06-snapshot-flow"]);
  assert.equal(missingRequiredArg.code, 1);
  assert.ok(missingRequiredArg.lines.some((line) => line.startsWith("ERROR_CODE:CLI_ARG_REQUIRED")));

  const badChoiceParse = runWithCapture([
    "choose",
    "--state-in",
    "missing.bin",
    "--choice",
    "abc",
    "--state-out",
    "out.bin",
  ]);
  assert.equal(badChoiceParse.code, 1);
  assert.ok(badChoiceParse.lines.some((line) => line.startsWith("ERROR_CODE:CLI_CHOICE_PARSE")));

  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-choice-range-"));
  const statePath = path.join(dir, "state.bin");
  const nextState = path.join(dir, "next.bin");
  const started = runWithCapture(["start", "--example", "06-snapshot-flow", "--state-out", statePath]);
  assert.equal(started.code, 0);
  const outOfRange = runWithCapture([
    "choose",
    "--state-in",
    statePath,
    "--choice",
    "99",
    "--state-out",
    nextState,
  ]);
  assert.equal(outOfRange.code, 1);
  assert.ok(outOfRange.lines.some((line) => line.startsWith("ERROR_CODE:ENGINE_CHOICE_INDEX")));

  const lines: string[] = [];
  let writeCount = 0;
  const unknownErrorCode = runAgentCommand(["list"], (line) => {
    writeCount += 1;
    if (writeCount === 1) {
      throw "boom";
    }
    lines.push(line);
  });
  assert.equal(unknownErrorCode, 1);
  assert.ok(lines.some((line) => line.startsWith("ERROR_CODE:CLI_ERROR")));
  assert.ok(lines.some((line) => line.startsWith("ERROR_MSG_JSON:\"Unknown CLI error.\"")));
});
