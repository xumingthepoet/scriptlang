import { compileScript } from "./compiler/index.js";
import type { SnapshotV1 } from "./core/types.js";
import { ScriptLangEngine, type HostFunctionMap } from "./runtime/index.js";

export interface CreateEngineFromXmlOptions {
  scriptsXml: Record<string, string>;
  entryScript: string;
  hostFunctions?: HostFunctionMap;
  compilerVersion?: string;
  vmTimeoutMs?: number;
}

export const compileScriptsFromXmlMap = (
  scriptsXml: Record<string, string>
) => {
  const compiled: Record<string, ReturnType<typeof compileScript>> = {};
  const scriptPaths = Object.keys(scriptsXml);
  for (let i = 0; i < scriptPaths.length; i += 1) {
    const scriptPath = scriptPaths[i];
    compiled[scriptPath] = compileScript(scriptsXml[scriptPath], scriptPath);
  }
  return compiled;
};

export const createEngineFromXml = (options: CreateEngineFromXmlOptions): ScriptLangEngine => {
  const scripts = compileScriptsFromXmlMap(options.scriptsXml);
  const engine = new ScriptLangEngine({
    scripts,
    hostFunctions: options.hostFunctions,
    compilerVersion: options.compilerVersion,
    vmTimeoutMs: options.vmTimeoutMs,
  });
  engine.start(options.entryScript);
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
