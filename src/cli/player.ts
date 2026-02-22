#!/usr/bin/env node

import path from "node:path";
import { fileURLToPath } from "node:url";

import { runAgentCommand } from "./commands/agent.js";
import { runTuiCommand } from "./commands/tui.js";

const usage = [
  "scriptlang-player",
  "  tui (--example <id> | --scripts-dir <path>) [--state-file <path>]",
  "  agent list",
  "  agent start (--example <id> | --scripts-dir <path>) --state-out <path>",
  "  agent choose --state-in <path> --choice <index> --state-out <path>",
].join("\n");

export const runPlayerCli = async (argv: string[]): Promise<number> => {
  const [mode, ...rest] = argv;
  if (!mode || mode === "--help" || mode === "-h") {
    process.stdout.write(`${usage}\n`);
    return 0;
  }
  if (mode === "agent") {
    return runAgentCommand(rest);
  }
  if (mode === "tui") {
    return runTuiCommand(rest);
  }
  process.stderr.write(`Unknown mode: ${mode}\n${usage}\n`);
  return 1;
};

const currentPath = fileURLToPath(import.meta.url);
const entryPath = process.argv[1] ? path.resolve(process.argv[1]) : "";

/* v8 ignore next 11 */
if (entryPath && currentPath === entryPath) {
  runPlayerCli(process.argv.slice(2))
    .then((code) => {
      process.exitCode = code;
    })
    .catch((error: unknown) => {
      const message = error instanceof Error ? error.message : "Unknown CLI crash.";
      process.stderr.write(`${message}\n`);
      process.exitCode = 1;
    });
}
