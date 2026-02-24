import { createEngineFromXml, resumeEngineFromXml } from "../../api.js";
import type { ChoiceItem, SnapshotV2 } from "../../core/types.js";
import type { ScriptLangEngine } from "../../runtime/index.js";
import type { LoadedScenario } from "./source-loader.js";

export const PLAYER_COMPILER_VERSION = "player.v1";

export interface BoundaryResult {
  event: "CHOICES" | "INPUT" | "END";
  texts: string[];
  choices: ChoiceItem[];
  choicePromptText: string | null;
  inputPromptText: string | null;
  inputDefaultText: string | null;
}

export interface StartedScenario {
  engine: ScriptLangEngine;
  boundary: BoundaryResult;
}

export const runToBoundary = (engine: ScriptLangEngine): BoundaryResult => {
  const texts: string[] = [];
  while (true) {
    const output = engine.next();
    if (output.kind === "text") {
      texts.push(output.text);
      continue;
    }
    if (output.kind === "choices") {
      return {
        event: "CHOICES",
        texts,
        choices: output.items,
        choicePromptText: output.promptText ?? null,
        inputPromptText: null,
        inputDefaultText: null,
      };
    }
    if (output.kind === "input") {
      return {
        event: "INPUT",
        texts,
        choices: [],
        choicePromptText: null,
        inputPromptText: output.promptText,
        inputDefaultText: output.defaultText,
      };
    }
    return {
      event: "END",
      texts,
      choices: [],
      choicePromptText: null,
      inputPromptText: null,
      inputDefaultText: null,
    };
  }
};

export const startScenario = (
  scenario: LoadedScenario,
  compilerVersion = PLAYER_COMPILER_VERSION
): StartedScenario => {
  const engine = createEngineFromXml({
    scriptsXml: scenario.scriptsXml,
    entryScript: scenario.entryScript,
    compilerVersion,
  });
  return { engine, boundary: runToBoundary(engine) };
};

export const resumeScenario = (
  scenario: LoadedScenario,
  snapshot: SnapshotV2,
  compilerVersion = PLAYER_COMPILER_VERSION
): StartedScenario => {
  const engine = resumeEngineFromXml({
    scriptsXml: scenario.scriptsXml,
    snapshot,
    compilerVersion,
  });
  return { engine, boundary: runToBoundary(engine) };
};

export const chooseAndContinue = (
  engine: ScriptLangEngine,
  choiceIndex: number
): BoundaryResult => {
  engine.choose(choiceIndex);
  return runToBoundary(engine);
};

export const submitInputAndContinue = (
  engine: ScriptLangEngine,
  text: string
): BoundaryResult => {
  engine.submitInput(text);
  return runToBoundary(engine);
};
