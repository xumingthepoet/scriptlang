import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { test } from "vitest";

import {
  PLAYER_COMPILER_VERSION,
  chooseAndContinue,
  resumeScenario,
  runToBoundary,
  startScenario,
  submitInputAndContinue,
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
  assert.equal(started.boundary.inputPromptText, null);
});

test("engine runner normalizes missing choice prompt text to null", () => {
  const fakeEngine = {
    next: () => ({ kind: "choices", items: [{ index: 0, id: "x", text: "x" }] }),
  } as unknown as Parameters<typeof runToBoundary>[0];
  const boundary = runToBoundary(fakeEngine);
  assert.equal(boundary.event, "CHOICES");
  assert.equal(boundary.choicePromptText, null);
});

test("engine runner carries input prompt/default text", () => {
  const scenario = loadSourceByScriptsDir(scriptsDir("16-input-name"));
  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);
  assert.equal(started.boundary.event, "INPUT");
  assert.equal(started.boundary.inputPromptText, "Name your hero");
  assert.equal(started.boundary.inputDefaultText, "Traveler");

  const resumed = resumeScenario(scenario, started.engine.snapshot(), PLAYER_COMPILER_VERSION);
  assert.equal(resumed.boundary.event, "INPUT");
  assert.equal(resumed.boundary.inputPromptText, "Name your hero");
  assert.equal(resumed.boundary.inputDefaultText, "Traveler");

  const afterHeroInput = submitInputAndContinue(resumed.engine, "Rin");
  assert.equal(afterHeroInput.event, "INPUT");
  assert.equal(afterHeroInput.inputPromptText, "Name your guild");
  assert.equal(afterHeroInput.inputDefaultText, "Nameless Guild Mk2");
});
