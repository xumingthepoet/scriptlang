import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";

import { test } from "vitest";

const npmCommand = process.platform === "win32" ? "npm.cmd" : "npm";

test("choice traversal tool validates all examples across visible choice branches", () => {
  const result = spawnSync(
    npmCommand,
    [
      "run",
      "-s",
      "traverse:choices",
      "--",
      "--examples-root",
      "examples/scripts",
      "--max-choice-steps",
      "100",
      "--max-runtime-ms",
      "30000",
      "--max-paths",
      "20000",
      "--verbose",
    ],
    {
      encoding: "utf8",
      cwd: process.cwd(),
    }
  );

  const output = `${result.stdout ?? ""}\n${result.stderr ?? ""}`;
  assert.equal(result.status, 0, output);
  assert.ok(output.includes("[PASS] 01-text-code"), output);
  assert.ok(output.includes("[PASS] 13-loop-times"), output);
  assert.ok(output.includes("[PASS] 14-defs-functions"), output);
  assert.ok(output.includes("[SUMMARY]"), output);
});
