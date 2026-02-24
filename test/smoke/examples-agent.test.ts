import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { test } from "vitest";

import { runAgentCommand } from "../../src/cli/commands/agent.js";

const EXAMPLES = [
  "01-text-code",
  "02-if-while",
  "03-choice-once",
  "04-call-ref-return",
  "05-return-transfer",
  "06-snapshot-flow",
  "07-battle-duel",
  "08-json-globals",
  "09-random",
  "10-once-static",
  "11-choice-fallover-continue",
  "12-while-break-continue",
  "13-loop-times",
] as const;

const MAX_STEPS = 30;

const scriptsDir = (id: string): string => path.resolve("examples", "scripts", id);

const runWithCapture = (argv: string[]) => {
  const lines: string[] = [];
  const code = runAgentCommand(argv, (line) => lines.push(line));
  return { code, lines };
};

const parseEvent = (lines: string[]): "CHOICES" | "END" | null => {
  const eventLine = lines.find((line) => line.startsWith("EVENT:"));
  if (!eventLine) {
    return null;
  }
  const value = eventLine.slice("EVENT:".length);
  if (value === "CHOICES" || value === "END") {
    return value;
  }
  return null;
};

test("examples 01..13 run to END via agent protocol", () => {
  for (const example of EXAMPLES) {
    const dir = fs.mkdtempSync(path.join(os.tmpdir(), `scriptlang-smoke-${example}-`));
    const state0 = path.join(dir, "s0.bin");

    const started = runWithCapture([
      "start",
      "--scripts-dir",
      scriptsDir(example),
      "--state-out",
      state0,
    ]);

    assert.equal(started.code, 0, `${example}: start should succeed`);
    assert.equal(started.lines[0], "RESULT:OK", `${example}: protocol should start with RESULT:OK`);

    let step = 0;
    let stateIn = state0;
    let currentLines = started.lines;
    let event = parseEvent(currentLines);

    while (event === "CHOICES" && step < MAX_STEPS) {
      assert.ok(
        currentLines.some((line) => line.startsWith("CHOICE:")),
        `${example}: choices payload should include CHOICE lines`
      );

      const nextState = path.join(dir, `s${step + 1}.bin`);
      const chosen = runWithCapture([
        "choose",
        "--state-in",
        stateIn,
        "--choice",
        "0",
        "--state-out",
        nextState,
      ]);
      assert.equal(chosen.code, 0, `${example}: choose step ${step} should succeed`);
      assert.equal(chosen.lines[0], "RESULT:OK", `${example}: choose step ${step} should return RESULT:OK`);
      if (parseEvent(chosen.lines) === "CHOICES") {
        assert.ok(
          chosen.lines.some((line) => line.startsWith("CHOICE:")),
          `${example}: choose step ${step} should include CHOICE lines`
        );
      }
      currentLines = chosen.lines;
      event = parseEvent(currentLines);
      stateIn = nextState;
      step += 1;
    }

    assert.notEqual(event, "CHOICES", `${example}: should not stay in choices after max steps`);
    assert.equal(event, "END", `${example}: should reach END`);
  }
});
