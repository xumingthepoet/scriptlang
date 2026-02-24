import {
  compileProjectBundleFromXmlMap,
  compileProjectScriptsFromXmlMap,
} from "./compiler/index.js";
import { ScriptLangError } from "./core/errors.js";
import type { SnapshotV1 } from "./core/types.js";
import { ScriptLangEngine, type HostFunctionMap } from "./runtime/index.js";

export interface CreateEngineFromXmlOptions {
  scriptsXml: Record<string, string>;
  entryScript?: string;
  entryArgs?: Record<string, unknown>;
  hostFunctions?: HostFunctionMap;
  randomSeed?: number;
  compilerVersion?: string;
}

export const compileScriptsFromXmlMap = (
  scriptsXml: Record<string, string>
) => {
  return compileProjectScriptsFromXmlMap(scriptsXml);
};

export interface CompileProjectFromXmlMapResult {
  scripts: ReturnType<typeof compileScriptsFromXmlMap>;
  entryScript: string;
  globalJson: Record<string, unknown>;
}

const resolveEntryScript = (
  scripts: ReturnType<typeof compileScriptsFromXmlMap>,
  explicitEntryScript?: string
): string => {
  if (explicitEntryScript) {
    if (!scripts[explicitEntryScript]) {
      throw new ScriptLangError(
        "API_ENTRY_SCRIPT_NOT_FOUND",
        `Entry script "${explicitEntryScript}" is not registered.`
      );
    }
    return explicitEntryScript;
  }
  if (!scripts.main) {
    throw new ScriptLangError(
      "API_ENTRY_MAIN_NOT_FOUND",
      "Expected exactly one script with name=\"main\" as default entry."
    );
  }
  return "main";
};

export const compileProjectFromXmlMap = (
  options: { xmlByPath: Record<string, string>; entryScript?: string }
): CompileProjectFromXmlMapResult => {
  const compiled = compileProjectBundleFromXmlMap(options.xmlByPath);
  const scripts = compiled.scripts;
  const entryScript = resolveEntryScript(scripts, options.entryScript);
  return { scripts, entryScript, globalJson: compiled.globalJson };
};

export const createEngineFromXml = (options: CreateEngineFromXmlOptions): ScriptLangEngine => {
  const compiled = compileProjectFromXmlMap({
    xmlByPath: options.scriptsXml,
    entryScript: options.entryScript,
  });
  const engine = new ScriptLangEngine({
    scripts: compiled.scripts,
    globalJson: compiled.globalJson,
    hostFunctions: options.hostFunctions,
    randomSeed: options.randomSeed,
    compilerVersion: options.compilerVersion,
  });
  engine.start(compiled.entryScript, options.entryArgs);
  return engine;
};

export interface ResumeEngineFromXmlOptions {
  scriptsXml: Record<string, string>;
  snapshot: SnapshotV1;
  hostFunctions?: HostFunctionMap;
  compilerVersion?: string;
}

export const resumeEngineFromXml = (options: ResumeEngineFromXmlOptions): ScriptLangEngine => {
  const compiled = compileProjectBundleFromXmlMap(options.scriptsXml);
  const engine = new ScriptLangEngine({
    scripts: compiled.scripts,
    globalJson: compiled.globalJson,
    hostFunctions: options.hostFunctions,
    compilerVersion: options.compilerVersion,
  });
  engine.resume(options.snapshot);
  return engine;
};
