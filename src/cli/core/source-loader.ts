import fs from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

const SCRIPTS_DIR_SCENARIO_PREFIX = "scripts-dir:";

export interface LoadedScenario {
  id: string;
  title: string;
  entryScript: string;
  scriptsXml: Record<string, string>;
}

const isScriptXmlFile = (file: string): boolean => file.endsWith(".script.xml");
const isTypesXmlFile = (file: string): boolean => file.endsWith(".types.xml");
const isJsonDataFile = (file: string): boolean => file.endsWith(".json");
const isScenarioXmlFile = (file: string): boolean =>
  isScriptXmlFile(file) || isTypesXmlFile(file) || isJsonDataFile(file);

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

export const getExamplesScriptsRoot = (): string => {
  return path.join(findProjectRoot(), "examples", "scripts");
};

export const resolveScriptsDir = (scriptsDir: string): string => {
  const resolved = path.resolve(scriptsDir);
  if (!fs.existsSync(resolved)) {
    throw makeCliError("CLI_SCRIPTS_DIR_NOT_FOUND", `Scripts directory does not exist: ${resolved}`);
  }
  const stat = fs.statSync(resolved);
  if (!stat.isDirectory()) {
    throw makeCliError("CLI_SCRIPTS_DIR_NOT_FOUND", `Scripts path is not a directory: ${resolved}`);
  }
  return resolved;
};

export const readScriptsXmlFromDir = (scriptsDir: string): Record<string, string> => {
  const files = fs
    .readdirSync(scriptsDir)
    .filter((file) => isScenarioXmlFile(file))
    .sort();
  if (!files.some((file) => isScriptXmlFile(file))) {
    throw makeCliError("CLI_SCRIPTS_DIR_EMPTY", `No .script.xml files found in: ${scriptsDir}`);
  }
  const scriptsXml: Record<string, string> = {};
  for (let i = 0; i < files.length; i += 1) {
    const file = files[i];
    const fullPath = path.join(scriptsDir, file);
    scriptsXml[file] = fs.readFileSync(fullPath, "utf8");
  }
  return scriptsXml;
};

export const makeScriptsDirScenarioId = (scriptsDir: string): string =>
  `${SCRIPTS_DIR_SCENARIO_PREFIX}${scriptsDir}`;

const parseScriptsDirScenarioId = (scenarioId: string): string => {
  if (!scenarioId.startsWith(SCRIPTS_DIR_SCENARIO_PREFIX)) {
    throw makeCliError(
      "CLI_STATE_INVALID",
      `State scenarioId must use ${SCRIPTS_DIR_SCENARIO_PREFIX}<absolute-path> format.`
    );
  }
  const scriptsDir = scenarioId.slice(SCRIPTS_DIR_SCENARIO_PREFIX.length);
  if (scriptsDir.length === 0) {
    throw makeCliError(
      "CLI_STATE_INVALID",
      `State scenarioId must use ${SCRIPTS_DIR_SCENARIO_PREFIX}<absolute-path> format.`
    );
  }
  return scriptsDir;
};

export const loadSourceByScriptsDir = (scriptsDir: string): LoadedScenario => {
  const resolvedDir = resolveScriptsDir(scriptsDir);
  return {
    id: makeScriptsDirScenarioId(resolvedDir),
    title: `Scripts ${path.basename(resolvedDir)}`,
    entryScript: "main",
    scriptsXml: readScriptsXmlFromDir(resolvedDir),
  };
};

export const loadSourceByRef = (scenarioRef: string): LoadedScenario => {
  return loadSourceByScriptsDir(parseScriptsDirScenarioId(scenarioRef));
};
