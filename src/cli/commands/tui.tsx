import React, { useRef, useState } from "react";
import { Box, Text, render, useApp, useInput } from "ink";

import {
  PLAYER_COMPILER_VERSION,
  chooseAndContinue,
  resumeScenario,
  startScenario,
  type BoundaryResult,
} from "../core/engine-runner.js";
import {
  loadScenarioById,
  loadScenarioByScriptsDir,
  type LoadedScenario,
} from "../core/scenario-registry.js";
import { createPlayerState, loadPlayerState, savePlayerState } from "../core/state-store.js";

export const DEFAULT_STATE_FILE = "./.scriptlang/save.bin";

export interface TuiOptions {
  example: string | null;
  scriptsDir: string | null;
  stateFile: string;
}

export const parseTuiArgs = (argv: string[]): TuiOptions => {
  const options: TuiOptions = {
    example: null,
    scriptsDir: null,
    stateFile: DEFAULT_STATE_FILE,
  };

  for (let i = 0; i < argv.length; i += 1) {
    const token = argv[i];
    if (token === "--example") {
      options.example = argv[i + 1] ?? "";
      i += 1;
      continue;
    }
    if (token === "--scripts-dir") {
      options.scriptsDir = argv[i + 1] ?? "";
      i += 1;
      continue;
    }
    if (token === "--state-file") {
      options.stateFile = argv[i + 1] ?? "";
      i += 1;
      continue;
    }
    throw new Error(`Unknown argument for tui mode: ${token}`);
  }

  if (options.example && options.scriptsDir) {
    throw new Error("Use exactly one source selector: --example <id> or --scripts-dir <path>.");
  }
  if (!options.example && !options.scriptsDir) {
    throw new Error("Missing source selector. Use --example <id> or --scripts-dir <path>.");
  }
  if (!options.stateFile) {
    throw new Error("--state-file cannot be empty.");
  }

  return options;
};

const applyBoundaryToState = (
  boundary: BoundaryResult,
  setChoices: (next: Array<{ index: number; id: string; text: string }>) => void,
  setEnded: (next: boolean) => void
): void => {
  if (boundary.event === "CHOICES") {
    setChoices(boundary.choices);
    setEnded(false);
    return;
  }
  setChoices([]);
  setEnded(true);
};

const PlayerApp = ({ scenario, stateFile }: { scenario: LoadedScenario; stateFile: string }) => {
  const { exit } = useApp();

  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);
  const engineRef = useRef(started.engine);
  const [lines, setLines] = useState<string[]>(started.boundary.texts);
  const [choices, setChoices] = useState(started.boundary.choices);
  const [ended, setEnded] = useState(started.boundary.event === "END");
  const [helpVisible, setHelpVisible] = useState(false);
  const [status, setStatus] = useState("ready");

  const appendBoundary = (boundary: BoundaryResult): void => {
    if (boundary.texts.length > 0) {
      setLines((prev) => [...prev, ...boundary.texts]);
    }
    applyBoundaryToState(boundary, setChoices, setEnded);
  };

  const replaceBoundary = (boundary: BoundaryResult): void => {
    setLines(boundary.texts);
    applyBoundaryToState(boundary, setChoices, setEnded);
  };

  const restart = (): void => {
    const next = startScenario(scenario, PLAYER_COMPILER_VERSION);
    engineRef.current = next.engine;
    replaceBoundary(next.boundary);
    setStatus("restarted");
  };

  useInput((input, key) => {
    try {
      if (key.escape || input === "q") {
        exit();
        return;
      }
      if (input === "h") {
        setHelpVisible((prev) => !prev);
        return;
      }
      if (input === "r") {
        restart();
        return;
      }
      if (input === "s") {
        const snapshot = engineRef.current.snapshot();
        const state = createPlayerState(scenario.id, PLAYER_COMPILER_VERSION, snapshot);
        savePlayerState(stateFile, state);
        setStatus(`saved to ${stateFile}`);
        return;
      }
      if (input === "l") {
        const state = loadPlayerState(stateFile);
        if (state.scenarioId !== scenario.id) {
          throw new Error(
            `State scenario mismatch. expected=${scenario.id} actual=${state.scenarioId}`
          );
        }
        const resumed = resumeScenario(scenario, state.snapshot, state.compilerVersion);
        engineRef.current = resumed.engine;
        appendBoundary(resumed.boundary);
        setStatus(`loaded from ${stateFile}`);
        return;
      }
      if (/^[1-9]$/.test(input)) {
        const index = Number.parseInt(input, 10) - 1;
        if (choices.length === 0) {
          setStatus("no pending choice");
          return;
        }
        if (index < 0 || index >= choices.length) {
          setStatus(`choice out of range: ${index}`);
          return;
        }
        const boundary = chooseAndContinue(engineRef.current, index);
        appendBoundary(boundary);
        setStatus(`chose ${index}`);
      }
    } catch (error) {
      const message = error instanceof Error ? error.message : "unknown error";
      setStatus(message);
    }
  });

  return (
    <Box flexDirection="column" paddingX={1}>
      <Text>
        {scenario.id} | {scenario.title}
      </Text>
      <Text color="gray">state: {stateFile}</Text>
      <Text color="gray">status: {status}</Text>
      {lines.map((line, index) => (
        <Text key={`line-${index}`}>{line}</Text>
      ))}
      {choices.length > 0 && <Text color="cyan">choices:</Text>}
      {choices.map((choice) => (
        <Text key={choice.id}>{`${choice.index + 1}. ${choice.text}`}</Text>
      ))}
      {ended && <Text color="green">[end]</Text>}
      <Text color="yellow">keys: 1..9 choose | s save | l load | r restart | h help | q quit</Text>
      {helpVisible && (
        <Text color="magenta">
          snapshot is valid only when waiting at choices. if save fails, continue until a choice appears.
        </Text>
      )}
    </Box>
  );
};

export const runTuiCommand = async (argv: string[]): Promise<number> => {
  try {
    const options = parseTuiArgs(argv);
    const scenario = options.example
      ? loadScenarioById(options.example)
      : loadScenarioByScriptsDir(options.scriptsDir as string);
    const app = render(<PlayerApp scenario={scenario} stateFile={options.stateFile} />);
    await app.waitUntilExit();
    return 0;
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown TUI error.";
    process.stderr.write(`${message}\n`);
    return 1;
  }
};
