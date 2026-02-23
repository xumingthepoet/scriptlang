import React, { useEffect, useRef, useState } from "react";
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
const CHOICE_VIEWPORT_ROWS = 5;
const TYPEWRITER_CHARS_PER_SECOND = 60;
const TYPEWRITER_TICK_MS = Math.floor(1000 / TYPEWRITER_CHARS_PER_SECOND);
const ELLIPSIS = "…";

const truncateToWidth = (value: string, width: number): string => {
  if (width <= 0) {
    return "";
  }
  if (value.length <= width) {
    return value;
  }
  if (width === 1) {
    return ELLIPSIS;
  }
  return `${value.slice(0, width - 1)}${ELLIPSIS}`;
};

const wrapLineToWidth = (value: string, width: number): string[] => {
  if (width <= 0) {
    return [""];
  }
  if (value.length === 0) {
    return [""];
  }
  const rows: string[] = [];
  for (let i = 0; i < value.length; i += width) {
    rows.push(value.slice(i, i + width));
  }
  return rows;
};

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

const PlayerApp = ({ scenario, stateFile }: { scenario: LoadedScenario; stateFile: string }) => {
  const { exit } = useApp();
  const [terminalSize, setTerminalSize] = useState(() => ({
    columns: process.stdout.columns ?? 80,
    rows: process.stdout.rows ?? 24,
  }));

  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);
  const engineRef = useRef(started.engine);
  const [renderedLines, setRenderedLines] = useState<string[]>([]);
  const [pendingLines, setPendingLines] = useState<string[]>(started.boundary.texts);
  const [typingLine, setTypingLine] = useState<string | null>(null);
  const [typingChars, setTypingChars] = useState(0);
  const [choices, setChoices] = useState(started.boundary.choices);
  const [selectedChoiceIndex, setSelectedChoiceIndex] = useState(0);
  const [choiceScrollOffset, setChoiceScrollOffset] = useState(0);
  const [ended, setEnded] = useState(started.boundary.event === "END");
  const [helpVisible, setHelpVisible] = useState(false);
  const [status, setStatus] = useState("ready");

  useEffect(() => {
    const updateTerminalSize = (): void => {
      setTerminalSize({
        columns: process.stdout.columns ?? 80,
        rows: process.stdout.rows ?? 24,
      });
    };
    process.stdout.on("resize", updateTerminalSize);
    return () => {
      process.stdout.off("resize", updateTerminalSize);
    };
  }, []);

  useEffect(() => {
    if (typingLine === null) {
      if (pendingLines.length === 0) {
        return;
      }
      const [nextLine, ...rest] = pendingLines;
      setPendingLines(rest);
      if (nextLine.length === 0) {
        setRenderedLines((prev) => [...prev, nextLine]);
        return;
      }
      setTypingLine(nextLine);
      setTypingChars(1);
      return;
    }

    if (typingChars >= typingLine.length) {
      setRenderedLines((prev) => [...prev, typingLine]);
      setTypingLine(null);
      setTypingChars(0);
      return;
    }

    const timer = globalThis.setTimeout(() => {
      setTypingChars((prev) => prev + 1);
    }, TYPEWRITER_TICK_MS);

    return () => {
      globalThis.clearTimeout(timer);
    };
  }, [pendingLines, typingLine, typingChars]);

  const setBoundaryChoices = (boundary: BoundaryResult): void => {
    if (boundary.event === "CHOICES") {
      setChoices(boundary.choices);
      setEnded(false);
      setSelectedChoiceIndex(0);
      setChoiceScrollOffset(0);
      return;
    }
    setChoices([]);
    setEnded(true);
    setSelectedChoiceIndex(0);
    setChoiceScrollOffset(0);
  };

  const appendBoundary = (boundary: BoundaryResult): void => {
    if (boundary.texts.length > 0) {
      setPendingLines((prev) => [...prev, ...boundary.texts]);
    }
    setBoundaryChoices(boundary);
  };

  const replaceBoundary = (boundary: BoundaryResult): void => {
    setRenderedLines([]);
    setPendingLines(boundary.texts);
    setTypingLine(null);
    setTypingChars(0);
    setBoundaryChoices(boundary);
  };

  const restart = (): void => {
    const next = startScenario(scenario, PLAYER_COMPILER_VERSION);
    engineRef.current = next.engine;
    replaceBoundary(next.boundary);
    setStatus("restarted");
  };

  const moveChoiceCursor = (delta: -1 | 1): void => {
    if (choices.length === 0) {
      setStatus("no pending choice");
      return;
    }
    const nextIndex = Math.max(0, Math.min(choices.length - 1, selectedChoiceIndex + delta));
    setSelectedChoiceIndex(nextIndex);
    setChoiceScrollOffset((prev) => {
      if (choices.length <= CHOICE_VIEWPORT_ROWS) {
        return 0;
      }
      if (nextIndex < prev) {
        return nextIndex;
      }
      if (nextIndex >= prev + CHOICE_VIEWPORT_ROWS) {
        return nextIndex - CHOICE_VIEWPORT_ROWS + 1;
      }
      return prev;
    });
  };

  const chooseCurrent = (): void => {
    if (choices.length === 0) {
      setStatus("no pending choice");
      return;
    }
    const boundary = chooseAndContinue(engineRef.current, selectedChoiceIndex);
    appendBoundary(boundary);
    setStatus(`chose ${selectedChoiceIndex}`);
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
      const typingInProgress = typingLine !== null || pendingLines.length > 0;
      if (key.upArrow) {
        if (typingInProgress) {
          setStatus("text streaming...");
          return;
        }
        moveChoiceCursor(-1);
        return;
      }
      if (key.downArrow) {
        if (typingInProgress) {
          setStatus("text streaming...");
          return;
        }
        moveChoiceCursor(1);
        return;
      }
      if (key.return) {
        if (typingInProgress) {
          setStatus("text streaming...");
          return;
        }
        chooseCurrent();
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
    } catch (error) {
      const message = error instanceof Error ? error.message : "unknown error";
      setStatus(message);
    }
  });

  const lines = typingLine
    ? [...renderedLines, typingLine.slice(0, Math.max(0, Math.min(typingChars, typingLine.length)))]
    : renderedLines;
  const typingInProgress = typingLine !== null || pendingLines.length > 0;
  const contentWidth = Math.max(16, terminalSize.columns - 2);
  const choiceTextWidth = Math.max(8, contentWidth - 2);
  const reservedRows =
    3 + // header rows: scenario/state/status
    1 + // divider
    1 + // choice title
    CHOICE_VIEWPORT_ROWS +
    1 + // choice window info
    (ended ? 1 : 0) +
    1 + // keys
    (helpVisible ? 1 : 0);
  const availableTextRows = Math.max(1, terminalSize.rows - reservedRows);
  const wrappedTextRows = lines.flatMap((line) => wrapLineToWidth(line, contentWidth));
  const visibleTextRows =
    wrappedTextRows.length <= availableTextRows
      ? wrappedTextRows
      : wrappedTextRows.slice(-availableTextRows);
  const choiceDisplayEnabled = !typingInProgress && choices.length > 0;
  const choiceRowsSource = choiceDisplayEnabled ? choices : [];
  const visibleChoiceRows = Array.from({ length: CHOICE_VIEWPORT_ROWS }, (_value, rowIndex) => {
    const absoluteIndex = choiceScrollOffset + rowIndex;
    const choice = choiceRowsSource[absoluteIndex];
    if (!choice) {
      return { key: `choice-empty-${rowIndex}`, text: " ", selected: false };
    }
    return {
      key: choice.id,
      text: truncateToWidth(choice.text, choiceTextWidth),
      selected: absoluteIndex === selectedChoiceIndex,
    };
  });
  const windowStart = choiceRowsSource.length === 0 ? 0 : choiceScrollOffset + 1;
  const windowEnd =
    choiceRowsSource.length === 0
      ? 0
      : Math.min(choiceScrollOffset + CHOICE_VIEWPORT_ROWS, choiceRowsSource.length);
  const dividerLine = "─".repeat(contentWidth);
  const choiceWindowText =
    choiceRowsSource.length > CHOICE_VIEWPORT_ROWS
      ? truncateToWidth(`window ${windowStart}-${windowEnd} / ${choiceRowsSource.length}`, contentWidth)
      : " ";
  const headerText = truncateToWidth(`${scenario.id} | ${scenario.title}`, contentWidth);
  const stateText = truncateToWidth(`state: ${stateFile}`, contentWidth);
  const statusText = truncateToWidth(`status: ${status}`, contentWidth);
  const keyText = truncateToWidth(
    "keys: up/down move | enter choose | s save | l load | r restart | h help | q quit",
    contentWidth
  );
  const helpText = truncateToWidth(
    "snapshot is valid only when waiting at choices. if save fails, continue until a choice appears.",
    contentWidth
  );

  return (
    <Box flexDirection="column" paddingX={1}>
      <Text>{headerText}</Text>
      <Text color="gray">{stateText}</Text>
      <Text color="gray">{statusText}</Text>
      {visibleTextRows.map((line, index) => (
        <Text key={`line-${index}`}>{line}</Text>
      ))}
      <Text color="gray">{dividerLine}</Text>
      <Text color="cyan">{truncateToWidth("choices (up/down + enter):", contentWidth)}</Text>
      <Box flexDirection="column">
        {visibleChoiceRows.map((row) => (
          <Text key={row.key} color={row.selected ? "green" : undefined}>
            {row.selected ? `> ${row.text}` : `  ${row.text}`}
          </Text>
        ))}
        <Text color="gray">{choiceWindowText}</Text>
      </Box>
      {ended && <Text color="green">[end]</Text>}
      <Text color="yellow">{keyText}</Text>
      {helpVisible && <Text color="magenta">{helpText}</Text>}
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
