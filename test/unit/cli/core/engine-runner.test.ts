import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { test } from "vitest";

import {
  PLAYER_COMPILER_VERSION,
  chooseAndContinue,
  resumeScenario,
  startScenario,
} from "../../../../src/cli/core/engine-runner.js";
import { loadSourceByScriptsDir } from "../../../../src/cli/core/source-loader.js";

const scriptsDir = (id: string): string => path.resolve("examples", "scripts", id);

test("engine runner start choose and resume flows", () => {
  const scenario = loadSourceByScriptsDir(scriptsDir("06-snapshot-flow"));
  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);

  assert.equal(started.boundary.event, "CHOICES");
  assert.deepEqual(started.boundary.texts, ["before 10"]);
  assert.equal(started.boundary.choices.length, 2);

  const snapshot = started.engine.snapshot();
  const resumed = resumeScenario(scenario, snapshot, PLAYER_COMPILER_VERSION);
  assert.equal(resumed.boundary.event, "CHOICES");

  const ended = chooseAndContinue(resumed.engine, 0);
  assert.equal(ended.event, "END");
  assert.deepEqual(ended.texts, ["after 15"]);
});

test("engine runner carries choice prompt text", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-choice-prompt-runner-"));
  fs.writeFileSync(
    path.join(dir, "main.script.xml"),
    `<script name="main"><choice text="Pick"><option text="Go"><text>done</text></option></choice></script>`
  );
  const scenario = loadSourceByScriptsDir(dir);
  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);
  assert.equal(started.boundary.event, "CHOICES");
  assert.equal(started.boundary.choicePromptText, "Pick");
});

test("engine runner normalizes legacy snapshot without prompt text to null", () => {
  const scenario = loadSourceByScriptsDir(scriptsDir("03-choice-once"));
  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);
  const snapshot = started.engine.snapshot();
  const legacySnapshot = structuredClone(snapshot);
  delete legacySnapshot.pendingChoicePromptText;

  const resumed = resumeScenario(scenario, legacySnapshot, PLAYER_COMPILER_VERSION);
  assert.equal(resumed.boundary.event, "CHOICES");
  assert.equal(resumed.boundary.choicePromptText, null);
});
