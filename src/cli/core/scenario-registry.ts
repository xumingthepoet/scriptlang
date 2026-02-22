import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCENARIOS = [
  {
    id: "01-text-code",
    title: "Text and Code",
    entryScript: "main",
    files: ["main.script.xml"],
  },
  {
    id: "02-if-while",
    title: "If and While",
    entryScript: "main",
    files: ["main.script.xml"],
  },
  {
    id: "03-choice-once",
    title: "Choice",
    entryScript: "main",
    files: ["main.script.xml"],
  },
  {
    id: "04-call-ref-return",
    title: "Call Ref Return",
    entryScript: "main",
    files: ["main.script.xml", "buff.script.xml"],
  },
  {
    id: "05-return-transfer",
    title: "Return Transfer",
    entryScript: "main",
    files: ["main.script.xml", "next.script.xml"],
  },
  {
    id: "06-snapshot-flow",
    title: "Snapshot Flow",
    entryScript: "main",
    files: ["main.script.xml"],
  },
  {
    id: "07-battle-duel",
    title: "Battle Duel",
    entryScript: "main",
    files: ["main.script.xml", "battle-loop.script.xml", "victory.script.xml", "defeat.script.xml"],
  },
] as const;

export interface ScenarioSummary {
  id: string;
  title: string;
}

export interface LoadedScenario {
  id: string;
  title: string;
  entryScript: string;
  scriptsXml: Record<string, string>;
}

const makeCliError = (code: string, message: string): Error & { code: string } => {
  const error = new Error(message) as Error & { code: string };
  error.code = code;
  return error;
};

const findProjectRoot = (): string => {
  let current = path.dirname(fileURLToPath(import.meta.url));
  while (true) {
    const pkg = path.join(current, "package.json");
    if (fs.existsSync(pkg)) {
      return current;
    }
    const parent = path.dirname(current);
    if (parent === current) {
      throw makeCliError("CLI_PROJECT_ROOT", "Cannot locate project root from CLI module path.");
    }
    current = parent;
  }
};

export const getScenarioScriptsRoot = (): string => {
  return path.join(findProjectRoot(), "examples", "scripts");
};

export const listScenarios = (): ScenarioSummary[] => {
  return SCENARIOS.map((scenario) => ({ id: scenario.id, title: scenario.title }));
};

export const loadScenarioById = (scenarioId: string): LoadedScenario => {
  const scenario = SCENARIOS.find((item) => item.id === scenarioId);
  if (!scenario) {
    throw makeCliError("CLI_SCENARIO_NOT_FOUND", `Unknown scenario id: ${scenarioId}`);
  }
  const scenarioDir = path.join(getScenarioScriptsRoot(), scenario.id);
  const scriptsXml: Record<string, string> = {};
  for (let i = 0; i < scenario.files.length; i += 1) {
    const name = scenario.files[i];
    const fullPath = path.join(scenarioDir, name);
    if (!fs.existsSync(fullPath)) {
      throw makeCliError("CLI_SCENARIO_FILE_MISSING", `Missing scenario file: ${fullPath}`);
    }
    scriptsXml[name] = fs.readFileSync(fullPath, "utf8");
  }
  return {
    id: scenario.id,
    title: scenario.title,
    entryScript: scenario.entryScript,
    scriptsXml,
  };
};
