import { ScriptLangError } from "../../core/errors.js";
import {
  PLAYER_COMPILER_VERSION,
  chooseAndContinue,
  resumeScenario,
  startScenario,
} from "../core/engine-runner.js";
import {
  loadScenarioById,
  loadScenarioByRef,
  loadScenarioByScriptsDir,
  listScenarios,
} from "../core/scenario-registry.js";
import { createPlayerState, loadPlayerState, savePlayerState } from "../core/state-store.js";

type WriteLine = (line: string) => void;

const makeCliError = (code: string, message: string): Error & { code: string } => {
  const error = new Error(message) as Error & { code: string };
  error.code = code;
  return error;
};

const parseFlags = (args: string[]): Record<string, string> => {
  const flags: Record<string, string> = {};
  for (let i = 0; i < args.length; i += 1) {
    const token = args[i];
    if (!token.startsWith("--")) {
      throw makeCliError("CLI_ARG_FORMAT", `Unexpected argument: ${token}`);
    }
    const name = token.slice(2);
    const value = args[i + 1];
    if (!value || value.startsWith("--")) {
      throw makeCliError("CLI_ARG_MISSING", `Missing value for --${name}`);
    }
    flags[name] = value;
    i += 1;
  }
  return flags;
};

const getRequiredFlag = (flags: Record<string, string>, name: string): string => {
  const value = flags[name];
  if (!value) {
    throw makeCliError("CLI_ARG_REQUIRED", `Missing required argument --${name}`);
  }
  return value;
};

const emitError = (writeLine: WriteLine, error: unknown): number => {
  let code =
    error instanceof ScriptLangError
      ? error.code
      : typeof error === "object" && error !== null && "code" in error
        ? String((error as { code: unknown }).code)
        : "CLI_ERROR";
  const message = error instanceof Error ? error.message : "Unknown CLI error.";
  if (
    code === "ENGINE_SCRIPT_NOT_FOUND" &&
    typeof message === "string" &&
    message.includes('Entry script "main"')
  ) {
    code = "CLI_ENTRY_MAIN_NOT_FOUND";
  }
  if (code === "API_ENTRY_MAIN_NOT_FOUND") {
    code = "CLI_ENTRY_MAIN_NOT_FOUND";
  }

  writeLine("RESULT:ERROR");
  writeLine(`ERROR_CODE:${code}`);
  writeLine(`ERROR_MSG_JSON:${JSON.stringify(message)}`);
  return 1;
};

const resolveStartScenario = (flags: Record<string, string>) => {
  const example = flags.example ?? "";
  const scriptsDir = flags["scripts-dir"] ?? "";
  if (example && scriptsDir) {
    throw makeCliError(
      "CLI_SOURCE_CONFLICT",
      "Use exactly one source selector: --example <id> or --scripts-dir <path>."
    );
  }
  if (!example && !scriptsDir) {
    throw makeCliError(
      "CLI_SOURCE_REQUIRED",
      "Missing source selector. Use --example <id> or --scripts-dir <path>."
    );
  }
  return example ? loadScenarioById(example) : loadScenarioByScriptsDir(scriptsDir);
};

const emitBoundary = (
  writeLine: WriteLine,
  event: "CHOICES" | "END",
  texts: string[],
  choices: Array<{ index: number; text: string }>,
  stateOut: string | null
): number => {
  writeLine("RESULT:OK");
  writeLine(`EVENT:${event}`);
  for (let i = 0; i < texts.length; i += 1) {
    writeLine(`TEXT_JSON:${JSON.stringify(texts[i])}`);
  }
  for (let i = 0; i < choices.length; i += 1) {
    const choice = choices[i];
    writeLine(`CHOICE:${choice.index}|${JSON.stringify(choice.text)}`);
  }
  writeLine(`STATE_OUT:${stateOut ?? "NONE"}`);
  return 0;
};

const runList = (writeLine: WriteLine): number => {
  const scenarios = listScenarios();
  writeLine("RESULT:OK");
  writeLine("EVENT:TEXT");
  for (let i = 0; i < scenarios.length; i += 1) {
    const scenario = scenarios[i];
    writeLine(`TEXT_JSON:${JSON.stringify(`${scenario.id}\t${scenario.title}`)}`);
  }
  writeLine("STATE_OUT:NONE");
  return 0;
};

const runStart = (args: string[], writeLine: WriteLine): number => {
  const flags = parseFlags(args);
  const stateOut = getRequiredFlag(flags, "state-out");

  const scenario = resolveStartScenario(flags);
  const { engine, boundary } = startScenario(scenario, PLAYER_COMPILER_VERSION);

  if (boundary.event === "CHOICES") {
    const state = createPlayerState(scenario.id, PLAYER_COMPILER_VERSION, engine.snapshot());
    savePlayerState(stateOut, state);
    return emitBoundary(writeLine, boundary.event, boundary.texts, boundary.choices, stateOut);
  }

  return emitBoundary(writeLine, boundary.event, boundary.texts, boundary.choices, null);
};

const runChoose = (args: string[], writeLine: WriteLine): number => {
  const flags = parseFlags(args);
  const stateIn = getRequiredFlag(flags, "state-in");
  const stateOut = getRequiredFlag(flags, "state-out");
  const rawChoice = getRequiredFlag(flags, "choice");
  const choice = Number.parseInt(rawChoice, 10);
  if (Number.isNaN(choice)) {
    throw makeCliError("CLI_CHOICE_PARSE", `Invalid choice index: ${rawChoice}`);
  }

  const state = loadPlayerState(stateIn);
  const scenario = loadScenarioByRef(state.scenarioId);
  const resumed = resumeScenario(scenario, state.snapshot, state.compilerVersion);
  const boundary = chooseAndContinue(resumed.engine, choice);

  if (boundary.event === "CHOICES") {
    const next = createPlayerState(scenario.id, state.compilerVersion, resumed.engine.snapshot());
    savePlayerState(stateOut, next);
    return emitBoundary(writeLine, boundary.event, boundary.texts, boundary.choices, stateOut);
  }

  return emitBoundary(writeLine, boundary.event, boundary.texts, boundary.choices, null);
};

export const runAgentCommand = (
  argv: string[],
  writeLine: WriteLine = (line) => {
    process.stdout.write(`${line}\n`);
  }
): number => {
  try {
    const [subcommand, ...rest] = argv;
    if (!subcommand) {
      throw makeCliError("CLI_AGENT_USAGE", "Missing agent subcommand. Use list/start/choose.");
    }
    if (subcommand === "list") {
      return runList(writeLine);
    }
    if (subcommand === "start") {
      return runStart(rest, writeLine);
    }
    if (subcommand === "choose") {
      return runChoose(rest, writeLine);
    }
    throw makeCliError("CLI_AGENT_USAGE", `Unknown agent subcommand: ${subcommand}`);
  } catch (error) {
    return emitError(writeLine, error);
  }
};
