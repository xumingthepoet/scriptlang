import { compileScript } from "./compiler";
import type { SnapshotV1 } from "./core/types";
import { ScriptLangEngine, type HostFunctionMap } from "./runtime";

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
  return Object.fromEntries(
    Object.entries(scriptsXml).map(([scriptPath, xml]) => [scriptPath, compileScript(xml, scriptPath)])
  );
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

