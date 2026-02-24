import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import v8 from "node:v8";

import { test } from "vitest";

import { runAgentCommand } from "../../../../src/cli/commands/agent.js";

const runWithCapture = (argv: string[]) => {
  const lines: string[] = [];
  const code = runAgentCommand(argv, (line) => lines.push(line));
  return { code, lines };
};

const scriptsDir = (id: string): string => path.resolve("examples", "scripts", id);

test("agent list command is unsupported", () => {
  const result = runWithCapture(["list"]);
  assert.equal(result.code, 1);
  assert.equal(result.lines[0], "RESULT:ERROR");
  assert.ok(result.lines.some((line) => line.startsWith("ERROR_CODE:CLI_AGENT_USAGE")));
});

test("agent start emits choices and writes state", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-start-"));
  const stateOut = path.join(dir, "state.bin");

  const result = runWithCapture([
    "start",
    "--scripts-dir",
    scriptsDir("06-snapshot-flow"),
    "--state-out",
    stateOut,
  ]);
  assert.equal(result.code, 0);
  assert.equal(result.lines[0], "RESULT:OK");
  assert.ok(result.lines.includes("EVENT:CHOICES"));
  assert.ok(result.lines.some((line) => line.startsWith("TEXT_JSON:")));
  assert.ok(result.lines.includes(`PROMPT_JSON:${JSON.stringify("Choose")}`));
  assert.ok(result.lines.some((line) => line.startsWith("CHOICE:0|")));
  assert.equal(result.lines[result.lines.length - 1], `STATE_OUT:${stateOut}`);
  assert.equal(fs.existsSync(stateOut), true);
});

test("agent start runs battle duel to first combat choice", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-battle-start-"));
  const stateOut = path.join(dir, "battle.bin");

  const result = runWithCapture([
    "start",
    "--scripts-dir",
    scriptsDir("07-battle-duel"),
    "--state-out",
    stateOut,
  ]);
  assert.equal(result.code, 0);
  assert.ok(result.lines.includes("EVENT:CHOICES"));
  assert.ok(result.lines.some((line) => line.startsWith("CHOICE:0|")));
  assert.ok(result.lines.some((line) => line.includes("Heavy Slash")));
  assert.equal(result.lines[result.lines.length - 1], `STATE_OUT:${stateOut}`);
  assert.equal(fs.existsSync(stateOut), true);
});

test("agent start can finish without state file", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-end-"));
  const stateOut = path.join(dir, "state.bin");

  const result = runWithCapture([
    "start",
    "--scripts-dir",
    scriptsDir("01-text-code"),
    "--state-out",
    stateOut,
  ]);
  assert.equal(result.code, 0);
  assert.ok(result.lines.includes("EVENT:END"));
  assert.equal(result.lines[result.lines.length - 1], "STATE_OUT:NONE");
  assert.equal(fs.existsSync(stateOut), false);
});

test("agent choices include prompt line when choice prompt exists", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-prompt-"));
  const tempScriptsDir = path.join(dir, "scripts");
  const stateOut = path.join(dir, "state.bin");
  fs.mkdirSync(tempScriptsDir, { recursive: true });
  fs.writeFileSync(
    path.join(tempScriptsDir, "main.script.xml"),
    `<script name="main"><choice text="Pick one"><option text="Go"><text>x</text></option></choice></script>`
  );

  const result = runWithCapture([
    "start",
    "--scripts-dir",
    tempScriptsDir,
    "--state-out",
    stateOut,
  ]);
  assert.equal(result.code, 0);
  assert.ok(result.lines.includes("EVENT:CHOICES"));
  assert.ok(result.lines.includes(`PROMPT_JSON:${JSON.stringify("Pick one")}`));
  assert.equal(result.lines[result.lines.length - 1], `STATE_OUT:${stateOut}`);
});

test("agent choose can continue to next choices and to end", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-choose-"));
  const firstState = path.join(dir, "first.bin");
  const secondState = path.join(dir, "second.bin");
  const thirdState = path.join(dir, "third.bin");

  const start = runWithCapture([
    "start",
    "--scripts-dir",
    scriptsDir("03-choice-once"),
    "--state-out",
    firstState,
  ]);
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

test("agent choose resumes scripts-dir state", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-agent-external-choose-"));
  const firstState = path.join(dir, "first.bin");
  const secondState = path.join(dir, "second.bin");

  const started = runWithCapture([
    "start",
    "--scripts-dir",
    scriptsDir("06-snapshot-flow"),
    "--state-out",
    firstState,
  ]);
  assert.equal(started.code, 0);
  assert.ok(started.lines.includes("EVENT:CHOICES"));

  const chosen = runWithCapture([
    "choose",
    "--state-in",
    firstState,
    "--choice",
    "0",
    "--state-out",
    secondState,
  ]);
  assert.equal(chosen.code, 0);
  assert.ok(chosen.lines.includes("EVENT:END"));
  assert.equal(chosen.lines[chosen.lines.length - 1], "STATE_OUT:NONE");
});

test("agent error protocol paths", () => {
  const missingSubcommand = runWithCapture([]);
  assert.equal(missingSubcommand.code, 1);
  assert.equal(missingSubcommand.lines[0], "RESULT:ERROR");
  assert.ok(missingSubcommand.lines.some((line) => line.startsWith("ERROR_CODE:CLI_AGENT_USAGE")));

  const unknownSubcommand = runWithCapture(["unknown"]);
  assert.equal(unknownSubcommand.code, 1);
  assert.ok(unknownSubcommand.lines.some((line) => line.startsWith("ERROR_CODE:CLI_AGENT_USAGE")));

  const badArgFormat = runWithCapture(["start", "scripts-dir", scriptsDir("06-snapshot-flow")]);
  assert.equal(badArgFormat.code, 1);
  assert.ok(badArgFormat.lines.some((line) => line.startsWith("ERROR_CODE:CLI_ARG_FORMAT")));

  const missingArgValue = runWithCapture(["start", "--scripts-dir"]);
  assert.equal(missingArgValue.code, 1);
  assert.ok(missingArgValue.lines.some((line) => line.startsWith("ERROR_CODE:CLI_ARG_MISSING")));

  const missingRequiredArg = runWithCapture([
    "start",
    "--scripts-dir",
    scriptsDir("06-snapshot-flow"),
  ]);
  assert.equal(missingRequiredArg.code, 1);
  assert.ok(missingRequiredArg.lines.some((line) => line.startsWith("ERROR_CODE:CLI_ARG_REQUIRED")));

  const missingSource = runWithCapture(["start", "--state-out", "out.bin"]);
  assert.equal(missingSource.code, 1);
  assert.ok(missingSource.lines.some((line) => line.startsWith("ERROR_CODE:CLI_SOURCE_REQUIRED")));

  const noMainDir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-no-main-"));
  fs.writeFileSync(
    path.join(noMainDir, "only.script.xml"),
    `<script name="only"><text>x</text></script>`
  );
  const missingMain = runWithCapture([
    "start",
    "--scripts-dir",
    noMainDir,
    "--state-out",
    path.join(noMainDir, "state.bin"),
  ]);
  assert.equal(missingMain.code, 1);
  assert.ok(missingMain.lines.some((line) => line.startsWith("ERROR_CODE:CLI_ENTRY_MAIN_NOT_FOUND")));

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
  const started = runWithCapture([
    "start",
    "--scripts-dir",
    scriptsDir("06-snapshot-flow"),
    "--state-out",
    statePath,
  ]);
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

  const legacyStatePath = path.join(dir, "legacy.bin");
  const parsed = v8.deserialize(fs.readFileSync(statePath)) as {
    schemaVersion: string;
    scenarioId: string;
    compilerVersion: string;
    snapshot: unknown;
  };
  parsed.scenarioId = "06-snapshot-flow";
  fs.writeFileSync(legacyStatePath, v8.serialize(parsed));

  const legacyChoose = runWithCapture([
    "choose",
    "--state-in",
    legacyStatePath,
    "--choice",
    "0",
    "--state-out",
    nextState,
  ]);
  assert.equal(legacyChoose.code, 1);
  assert.ok(legacyChoose.lines.some((line) => line.startsWith("ERROR_CODE:CLI_STATE_INVALID")));

  const lines: string[] = [];
  let writeCount = 0;
  const unknownErrorCode = runAgentCommand(
    [
      "start",
      "--scripts-dir",
      scriptsDir("06-snapshot-flow"),
      "--state-out",
      path.join(dir, "writer.bin"),
    ],
    (line) => {
      writeCount += 1;
      if (writeCount === 1) {
        throw "boom";
      }
      lines.push(line);
    }
  );
  assert.equal(unknownErrorCode, 1);
  assert.ok(lines.some((line) => line.startsWith("ERROR_CODE:CLI_ERROR")));
  assert.ok(lines.some((line) => line.startsWith("ERROR_MSG_JSON:\"Unknown CLI error.\"")));

  const mappedLines: string[] = [];
  let mappedWriteCount = 0;
  const engineMainError = Object.assign(new Error('Entry script "main" is not registered.'), {
    code: "ENGINE_SCRIPT_NOT_FOUND",
  });
  const mappedCode = runAgentCommand(
    [
      "start",
      "--scripts-dir",
      scriptsDir("06-snapshot-flow"),
      "--state-out",
      path.join(dir, "writer2.bin"),
    ],
    (line) => {
      mappedWriteCount += 1;
      if (mappedWriteCount === 1) {
        throw engineMainError;
      }
      mappedLines.push(line);
    }
  );
  assert.equal(mappedCode, 1);
  assert.ok(mappedLines.some((line) => line.startsWith("ERROR_CODE:CLI_ENTRY_MAIN_NOT_FOUND")));
});
