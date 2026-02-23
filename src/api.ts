import { compileProjectScriptsFromXmlMap } from "./compiler/index.js";
import { ScriptLangError } from "./core/errors.js";
import type { SnapshotV1 } from "./core/types.js";
import { ScriptLangEngine, type HostFunctionMap } from "./runtime/index.js";

export interface CreateEngineFromXmlOptions {
  scriptsXml: Record<string, string>;
  entryScript?: string;
  hostFunctions?: HostFunctionMap;
  compilerVersion?: string;
  vmTimeoutMs?: number;
}

export const compileScriptsFromXmlMap = (
  scriptsXml: Record<string, string>
) => {
  return compileProjectScriptsFromXmlMap(scriptsXml);
};

export interface CompileProjectFromXmlMapResult {
  scripts: ReturnType<typeof compileScriptsFromXmlMap>;
  entryScript: string;
}

const resolveEntryScript = (
  scripts: ReturnType<typeof compileScriptsFromXmlMap>,
  explicitEntryScript?: string
): string => {
  if (!scripts.main) {
    throw new ScriptLangError(
      "API_ENTRY_MAIN_NOT_FOUND",
      "Expected exactly one script with name=\"main\" as default entry."
    );
  }
  if (explicitEntryScript) {
    return explicitEntryScript;
  }
  return "main";
};

export const compileProjectFromXmlMap = (
  options: { xmlByPath: Record<string, string>; entryScript?: string }
): CompileProjectFromXmlMapResult => {
  const scripts = compileScriptsFromXmlMap(options.xmlByPath);
  const entryScript = resolveEntryScript(scripts, options.entryScript);
  return { scripts, entryScript };
};

export const createEngineFromXml = (options: CreateEngineFromXmlOptions): ScriptLangEngine => {
  const compiled = compileProjectFromXmlMap({
    xmlByPath: options.scriptsXml,
    entryScript: options.entryScript,
  });
  const engine = new ScriptLangEngine({
    scripts: compiled.scripts,
    hostFunctions: options.hostFunctions,
    compilerVersion: options.compilerVersion,
    vmTimeoutMs: options.vmTimeoutMs,
  });
  engine.start(compiled.entryScript);
  return engine;
};

export interface ResumeEngineFromXmlOptions {
  scriptsXml: Record<string, string>;
  snapshot: SnapshotV1;
  hostFunctions?: HostFunctionMap;
  compilerVersion?: string;
  vmTimeoutMs?: number;
}

export const resumeEngineFromXml = (options: ResumeEngineFromXmlOptions): ScriptLangEngine => {
  const scripts = compileScriptsFromXmlMap(options.scriptsXml);
  const engine = new ScriptLangEngine({
    scripts,
    hostFunctions: options.hostFunctions,
    compilerVersion: options.compilerVersion,
    vmTimeoutMs: options.vmTimeoutMs,
  });
  engine.resume(options.snapshot);
  return engine;
};
