import { ScriptLangError } from "../../core/errors.js";
import {
  PLAYER_COMPILER_VERSION,
  chooseAndContinue,
  resumeScenario,
  startScenario,
  submitInputAndContinue,
} from "../core/engine-runner.js";
import { loadSourceByRef, loadSourceByScriptsDir } from "../core/source-loader.js";
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
    if (value === undefined || value.startsWith("--")) {
      throw makeCliError("CLI_ARG_MISSING", `Missing value for --${name}`);
    }
    flags[name] = value;
    i += 1;
  }
  return flags;
};

const getRequiredFlag = (flags: Record<string, string>, name: string): string => {
  const value = flags[name];
  if (value === undefined) {
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
    typeof message === "string"
  ) {
    code = message.includes('Entry script "main"')
      ? "CLI_ENTRY_MAIN_NOT_FOUND"
      : "CLI_ENTRY_SCRIPT_NOT_FOUND";
  }
  if (code === "API_ENTRY_MAIN_NOT_FOUND") {
    code = "CLI_ENTRY_MAIN_NOT_FOUND";
  }
  if (code === "API_ENTRY_SCRIPT_NOT_FOUND") {
    code =
      typeof message === "string" && message.includes('Entry script "main"')
        ? "CLI_ENTRY_MAIN_NOT_FOUND"
        : "CLI_ENTRY_SCRIPT_NOT_FOUND";
  }

  writeLine("RESULT:ERROR");
  writeLine(`ERROR_CODE:${code}`);
  writeLine(`ERROR_MSG_JSON:${JSON.stringify(message)}`);
  return 1;
};

const resolveStartScenario = (flags: Record<string, string>) => {
  const scriptsDir = flags["scripts-dir"] ?? "";
  const entryScript = flags["entry-script"] ?? "main";
  if (!scriptsDir) {
    throw makeCliError(
      "CLI_SOURCE_REQUIRED",
      "Missing source selector. Use --scripts-dir <path>."
    );
  }
  return loadSourceByScriptsDir(scriptsDir, entryScript);
};

const emitBoundary = (
  writeLine: WriteLine,
  event: "CHOICES" | "INPUT" | "END",
  texts: string[],
  choices: Array<{ index: number; text: string }>,
  choicePromptText: string | null,
  inputPromptText: string | null,
  inputDefaultText: string | null,
  stateOut: string | null
): number => {
  writeLine("RESULT:OK");
  writeLine(`EVENT:${event}`);
  for (let i = 0; i < texts.length; i += 1) {
    writeLine(`TEXT_JSON:${JSON.stringify(texts[i])}`);
  }
  if (event === "CHOICES" && choicePromptText !== null) {
    writeLine(`PROMPT_JSON:${JSON.stringify(choicePromptText)}`);
  }
  if (event === "INPUT" && inputPromptText !== null) {
    writeLine(`PROMPT_JSON:${JSON.stringify(inputPromptText)}`);
  }
  for (let i = 0; i < choices.length; i += 1) {
    const choice = choices[i];
    writeLine(`CHOICE:${choice.index}|${JSON.stringify(choice.text)}`);
  }
  if (event === "INPUT" && inputDefaultText !== null) {
    writeLine(`INPUT_DEFAULT_JSON:${JSON.stringify(inputDefaultText)}`);
  }
  writeLine(`STATE_OUT:${stateOut ?? "NONE"}`);
  return 0;
};

const runStart = (args: string[], writeLine: WriteLine): number => {
  const flags = parseFlags(args);
  const stateOut = getRequiredFlag(flags, "state-out");

  const scenario = resolveStartScenario(flags);
  const { engine, boundary } = startScenario(scenario, PLAYER_COMPILER_VERSION);

  if (boundary.event === "CHOICES" || boundary.event === "INPUT") {
    const state = createPlayerState(scenario.id, PLAYER_COMPILER_VERSION, engine.snapshot());
    savePlayerState(stateOut, state);
    return emitBoundary(
      writeLine,
      boundary.event,
      boundary.texts,
      boundary.choices,
      boundary.choicePromptText,
      boundary.inputPromptText,
      boundary.inputDefaultText,
      stateOut
    );
  }

  return emitBoundary(
    writeLine,
    boundary.event,
    boundary.texts,
    boundary.choices,
    boundary.choicePromptText,
    boundary.inputPromptText,
    boundary.inputDefaultText,
    null
  );
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
  const scenario = loadSourceByRef(state.scenarioId);
  const resumed = resumeScenario(scenario, state.snapshot, state.compilerVersion);
  const boundary = chooseAndContinue(resumed.engine, choice);

  if (boundary.event === "CHOICES" || boundary.event === "INPUT") {
    const next = createPlayerState(scenario.id, state.compilerVersion, resumed.engine.snapshot());
    savePlayerState(stateOut, next);
    return emitBoundary(
      writeLine,
      boundary.event,
      boundary.texts,
      boundary.choices,
      boundary.choicePromptText,
      boundary.inputPromptText,
      boundary.inputDefaultText,
      stateOut
    );
  }

  return emitBoundary(
    writeLine,
    boundary.event,
    boundary.texts,
    boundary.choices,
    boundary.choicePromptText,
    boundary.inputPromptText,
    boundary.inputDefaultText,
    null
  );
};

const runInput = (args: string[], writeLine: WriteLine): number => {
  const flags = parseFlags(args);
  const stateIn = getRequiredFlag(flags, "state-in");
  const stateOut = getRequiredFlag(flags, "state-out");
  const text = getRequiredFlag(flags, "text");

  const state = loadPlayerState(stateIn);
  const scenario = loadSourceByRef(state.scenarioId);
  const resumed = resumeScenario(scenario, state.snapshot, state.compilerVersion);
  const boundary = submitInputAndContinue(resumed.engine, text);

  if (boundary.event === "CHOICES" || boundary.event === "INPUT") {
    const next = createPlayerState(scenario.id, state.compilerVersion, resumed.engine.snapshot());
    savePlayerState(stateOut, next);
    return emitBoundary(
      writeLine,
      boundary.event,
      boundary.texts,
      boundary.choices,
      boundary.choicePromptText,
      boundary.inputPromptText,
      boundary.inputDefaultText,
      stateOut
    );
  }

  return emitBoundary(
    writeLine,
    boundary.event,
    boundary.texts,
    boundary.choices,
    boundary.choicePromptText,
    boundary.inputPromptText,
    boundary.inputDefaultText,
    null
  );
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
      throw makeCliError("CLI_AGENT_USAGE", "Missing agent subcommand. Use start/choose.");
    }
    if (subcommand === "start") {
      return runStart(rest, writeLine);
    }
    if (subcommand === "choose") {
      return runChoose(rest, writeLine);
    }
    if (subcommand === "input") {
      return runInput(rest, writeLine);
    }
    throw makeCliError(
      "CLI_AGENT_USAGE",
      `Unknown agent subcommand: ${subcommand}. Use start/choose/input.`
    );
  } catch (error) {
    return emitError(writeLine, error);
  }
};
