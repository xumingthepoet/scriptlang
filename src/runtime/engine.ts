import vm from "node:vm";

import { ScriptLangError } from "../core/errors";
import type {
  ChoiceItem,
  ChoiceNode,
  ContinuationFrame,
  EngineOutput,
  ScriptIR,
  ScriptNode,
  SnapshotFrameV1,
  SnapshotV1,
  VarDeclaration,
} from "../core/types";

export type HostFunctionMap = Record<string, (...args: unknown[]) => unknown>;

type CompletionKind = "none" | "whileBody" | "resumeAfterChild";

interface RuntimeFrame {
  frameId: number;
  groupId: string;
  nodeIndex: number;
  scope: Record<string, unknown>;
  completion: CompletionKind;
  scriptRoot: boolean;
  returnContinuation: ContinuationFrame | null;
}

interface GroupLookup {
  scriptPath: string;
  group: ScriptIR["groups"][string];
}

interface PendingChoice {
  frameId: number;
  nodeId: string;
  options: ChoiceItem[];
}

const deepClone = <T>(value: T): T => structuredClone(value);

const defaultValueFromVar = (decl: VarDeclaration): unknown => {
  if (decl.type.kind === "primitive") {
    if (decl.type.name === "number") return 0;
    if (decl.type.name === "string") return "";
    if (decl.type.name === "boolean") return false;
    return null;
  }
  if (decl.type.kind === "array") return [];
  if (decl.type.kind === "record") return {};
  return new Map<string, unknown>();
};

const parseRefPath = (path: string): string[] => {
  return path
    .split(".")
    .map((part) => part.trim())
    .filter(Boolean);
};

export interface ScriptLangEngineOptions {
  scripts: Record<string, ScriptIR>;
  hostFunctions?: HostFunctionMap;
  compilerVersion?: string;
  vmTimeoutMs?: number;
}

export class ScriptLangEngine {
  private readonly scripts: Record<string, ScriptIR>;
  private readonly hostFunctions: HostFunctionMap;
  private readonly compilerVersion: string;
  private readonly vmTimeoutMs: number;
  private readonly groupLookup: Record<string, GroupLookup>;

  private frames: RuntimeFrame[] = [];
  private pendingChoice: PendingChoice | null = null;
  private selectedChoices = new Set<string>();
  private frameCounter = 1;
  private ended = false;

  constructor(options: ScriptLangEngineOptions) {
    this.scripts = options.scripts;
    this.hostFunctions = options.hostFunctions ?? {};
    this.compilerVersion = options.compilerVersion ?? "dev";
    this.vmTimeoutMs = options.vmTimeoutMs ?? 100;
    this.groupLookup = {};

    for (const [scriptPath, script] of Object.entries(this.scripts)) {
      for (const [groupId, group] of Object.entries(script.groups)) {
        this.groupLookup[groupId] = { scriptPath, group };
      }
    }
  }

  get waitingChoice(): boolean {
    return this.pendingChoice !== null;
  }

  start(entryScriptPath: string): void {
    this.reset();
    const entry = this.scripts[entryScriptPath];
    if (!entry) {
      throw new ScriptLangError(
        "ENGINE_SCRIPT_NOT_FOUND",
        `Entry script "${entryScriptPath}" is not registered.`
      );
    }
    const rootScope = this.createScriptRootScope(entryScriptPath, {});
    this.frames.push({
      frameId: this.frameCounter++,
      groupId: entry.rootGroupId,
      nodeIndex: 0,
      scope: rootScope,
      completion: "none",
      scriptRoot: true,
      returnContinuation: null,
    });
  }

  next(): EngineOutput {
    if (this.waitingChoice && this.pendingChoice) {
      return { kind: "choices", items: this.pendingChoice.options };
    }
    if (this.ended) {
      return { kind: "end" };
    }

    let guard = 0;
    while (guard < 10000) {
      guard += 1;
      const top = this.frames[this.frames.length - 1];
      if (!top) {
        this.ended = true;
        return { kind: "end" };
      }
      const lookup = this.groupLookup[top.groupId];
      if (!lookup) {
        throw new ScriptLangError(
          "ENGINE_GROUP_NOT_FOUND",
          `Group "${top.groupId}" is not registered.`
        );
      }
      const group = lookup.group;

      if (top.nodeIndex >= group.nodes.length) {
        this.finishFrame(top);
        continue;
      }

      const node = group.nodes[top.nodeIndex];
      if (node.kind === "text") {
        top.nodeIndex += 1;
        return { kind: "text", text: this.renderText(node.value) };
      }
      if (node.kind === "code") {
        this.runCode(node.code);
        top.nodeIndex += 1;
        continue;
      }
      if (node.kind === "if") {
        const condition = this.evalBoolean(node.whenExpr);
        top.nodeIndex += 1;
        const target = condition ? node.thenGroupId : (node.elseGroupId as string);
        this.pushGroupFrame(target, "resumeAfterChild");
        continue;
      }
      if (node.kind === "while") {
        const condition = this.evalBoolean(node.whenExpr);
        if (!condition) {
          top.nodeIndex += 1;
          continue;
        }
        this.pushGroupFrame(node.bodyGroupId, "whileBody");
        continue;
      }
      if (node.kind === "choice") {
        const options = node.options
          .filter((opt) => (opt.whenExpr ? this.evalBoolean(opt.whenExpr) : true))
          .filter((opt) => (opt.once ? !this.selectedChoices.has(opt.id) : true))
          .map((opt, index) => ({
            index,
            id: opt.id,
            text: this.renderText(opt.text),
          }));
        if (options.length === 0) {
          top.nodeIndex += 1;
          continue;
        }
        this.pendingChoice = { frameId: top.frameId, nodeId: node.id, options };
        return { kind: "choices", items: options };
      }
      if (node.kind === "call") {
        this.executeCall(node);
        continue;
      }
      if (node.kind === "return") {
        this.executeReturn(node.targetScript);
        continue;
      }

      throw new ScriptLangError("ENGINE_NODE_UNKNOWN", `Unhandled node kind: ${(node as ScriptNode).kind}`);
    }

    throw new ScriptLangError("ENGINE_GUARD_EXCEEDED", "Execution guard exceeded 10000 iterations.");
  }

  choose(index: number): void {
    if (!this.pendingChoice) {
      throw new ScriptLangError("ENGINE_NO_PENDING_CHOICE", "No pending choice is available.");
    }
    const frame = this.findFrame(this.pendingChoice.frameId);
    if (!frame) {
      throw new ScriptLangError("ENGINE_CHOICE_FRAME_MISSING", "Pending choice frame is missing.");
    }
    const group = this.groupLookup[frame.groupId].group;
    const node = group.nodes[frame.nodeIndex];
    if (!node || node.kind !== "choice") {
      throw new ScriptLangError("ENGINE_CHOICE_NODE_MISSING", "Pending choice node is no longer valid.");
    }

    if (index < 0 || index >= this.pendingChoice.options.length) {
      throw new ScriptLangError("ENGINE_CHOICE_INDEX", `Choice index "${index}" is out of range.`);
    }

    const item = this.pendingChoice.options[index];
    const option = node.options.find((x) => x.id === item.id);
    if (!option) {
      throw new ScriptLangError("ENGINE_CHOICE_NOT_FOUND", `Choice "${item.id}" not found.`);
    }

    this.selectedChoices.add(option.id);
    frame.nodeIndex += 1;
    this.pushGroupFrame(option.groupId, "resumeAfterChild");
    this.pendingChoice = null;
  }

  snapshot(): SnapshotV1 {
    if (!this.pendingChoice) {
      throw new ScriptLangError(
        "SNAPSHOT_NOT_ALLOWED",
        "snapshot() is only allowed while waiting for a choice."
      );
    }

    const runtimeFrames: SnapshotFrameV1[] = this.frames.map((frame) => ({
      frameId: frame.frameId,
      groupId: frame.groupId,
      nodeIndex: frame.nodeIndex,
      scope: deepClone(frame.scope),
      completion: { kind: frame.completion },
      scriptRoot: frame.scriptRoot,
      returnContinuation: frame.returnContinuation ? deepClone(frame.returnContinuation) : null,
    }));

    return {
      schemaVersion: "snapshot.v1",
      compilerVersion: this.compilerVersion,
      cursor: {
        groupPath: this.frames.map((f) => f.groupId),
        nodeIndex: this.frames[this.frames.length - 1]?.nodeIndex ?? 0,
      },
      scopeChain: this.frames.map((f) => ({
        groupId: f.groupId,
        values: deepClone(f.scope),
      })),
      continuations: this.frames
        .filter((f) => f.scriptRoot && f.returnContinuation)
        .map((f) => deepClone(f.returnContinuation as ContinuationFrame)),
      runtimeFrames,
      waitingChoice: true,
      pendingChoiceNodeId: this.pendingChoice.nodeId,
      selectedChoices: Array.from(this.selectedChoices),
    };
  }

  resume(snapshot: SnapshotV1): void {
    if (snapshot.schemaVersion !== "snapshot.v1") {
      throw new ScriptLangError(
        "SNAPSHOT_SCHEMA",
        `Unsupported snapshot schema "${snapshot.schemaVersion}".`
      );
    }
    if (snapshot.compilerVersion !== this.compilerVersion) {
      throw new ScriptLangError(
        "SNAPSHOT_COMPILER_VERSION",
        `Snapshot compiler version "${snapshot.compilerVersion}" does not match engine "${this.compilerVersion}".`
      );
    }
    if (!snapshot.waitingChoice) {
      throw new ScriptLangError(
        "SNAPSHOT_WAITING_CHOICE",
        "Only waiting-choice snapshots are supported in V1."
      );
    }

    this.reset();
    this.frames = snapshot.runtimeFrames.map((frame) => {
      if (!this.groupLookup[frame.groupId]) {
        throw new ScriptLangError("SNAPSHOT_GROUP_MISSING", `Group "${frame.groupId}" is unknown.`);
      }
      return {
        frameId: frame.frameId,
        groupId: frame.groupId,
        nodeIndex: frame.nodeIndex,
        scope: deepClone(frame.scope),
        completion: frame.completion.kind,
        scriptRoot: frame.scriptRoot,
        returnContinuation: frame.returnContinuation ? deepClone(frame.returnContinuation) : null,
      };
    });
    this.selectedChoices = new Set(snapshot.selectedChoices);
    this.frameCounter =
      this.frames.reduce((max, frame) => (frame.frameId > max ? frame.frameId : max), 0) + 1;

    const top = this.frames[this.frames.length - 1];
    if (!top) {
      throw new ScriptLangError("SNAPSHOT_EMPTY", "Snapshot contains no runtime frames.");
    }
    const group = this.groupLookup[top.groupId].group;
    const node = group.nodes[top.nodeIndex];
    if (!node || node.kind !== "choice" || node.id !== snapshot.pendingChoiceNodeId) {
      throw new ScriptLangError("SNAPSHOT_PENDING_CHOICE", "Pending choice node cannot be reconstructed.");
    }

    const options = node.options
      .filter((opt) => (opt.whenExpr ? this.evalBoolean(opt.whenExpr) : true))
      .filter((opt) => (opt.once ? !this.selectedChoices.has(opt.id) : true))
      .map((opt, index) => ({
        index,
        id: opt.id,
        text: this.renderText(opt.text),
      }));
    this.pendingChoice = {
      frameId: top.frameId,
      nodeId: node.id,
      options,
    };
  }

  private reset(): void {
    this.frames = [];
    this.pendingChoice = null;
    this.selectedChoices = new Set<string>();
    this.ended = false;
    this.frameCounter = 1;
  }

  private findFrame(frameId: number): RuntimeFrame | null {
    return this.frames.find((frame) => frame.frameId === frameId) ?? null;
  }

  private pushGroupFrame(groupId: string, completion: CompletionKind): void {
    if (!this.groupLookup[groupId]) {
      throw new ScriptLangError("ENGINE_GROUP_NOT_FOUND", `Group "${groupId}" is not registered.`);
    }
    this.frames.push({
      frameId: this.frameCounter++,
      groupId,
      nodeIndex: 0,
      scope: {},
      completion,
      scriptRoot: false,
      returnContinuation: null,
    });
  }

  private finishFrame(frame: RuntimeFrame): void {
    this.frames.pop();
    if (!frame.scriptRoot) {
      return;
    }

    const continuation = frame.returnContinuation;
    if (!continuation) {
      this.ended = true;
      this.frames = [];
      return;
    }

    const resumeFrame = this.findFrame(continuation.resumeFrameId);
    if (!resumeFrame) {
      this.ended = true;
      this.frames = [];
      return;
    }
    for (const [calleeVar, callerPath] of Object.entries(continuation.refBindings)) {
      const value = deepClone(frame.scope[calleeVar]);
      this.writePath(callerPath, value);
    }
    resumeFrame.nodeIndex = continuation.nextNodeIndex;
  }

  private executeCall(node: Extract<ScriptNode, { kind: "call" }>): void {
    const caller = this.frames[this.frames.length - 1];
    if (!caller) {
      throw new ScriptLangError("ENGINE_CALL_NO_FRAME", "No frame available for <call>.");
    }
    const group = this.groupLookup[caller.groupId].group;
    const targetScript = this.scripts[node.targetScript];
    if (!targetScript) {
      throw new ScriptLangError(
        "ENGINE_CALL_TARGET",
        `Call target script "${node.targetScript}" is not registered.`,
        node.location
      );
    }

    const argValues: Record<string, unknown> = {};
    const refBindings: Record<string, string> = {};
    for (const arg of node.args) {
      if (arg.isRef) {
        argValues[arg.name] = this.readPath(arg.valueExpr);
        refBindings[arg.name] = arg.valueExpr;
      } else {
        argValues[arg.name] = this.evalExpression(arg.valueExpr);
      }
    }

    const isTailAtRoot =
      caller.scriptRoot &&
      caller.nodeIndex === group.nodes.length - 1 &&
      caller.returnContinuation !== null;

    if (isTailAtRoot && Object.keys(refBindings).length > 0) {
      throw new ScriptLangError(
        "ENGINE_TAIL_REF_UNSUPPORTED",
        "Tail call with ref args is not supported in V1."
      );
    }

    if (isTailAtRoot) {
      const inherited = caller.returnContinuation;
      this.frames.pop();
      const rootScope = this.createScriptRootScope(node.targetScript, argValues);
      this.frames.push({
        frameId: this.frameCounter++,
        groupId: targetScript.rootGroupId,
        nodeIndex: 0,
        scope: rootScope,
        completion: "none",
        scriptRoot: true,
        returnContinuation: inherited,
      });
      return;
    }

    const continuation: ContinuationFrame = {
      resumeFrameId: caller.frameId,
      nextNodeIndex: caller.nodeIndex + 1,
      refBindings,
    };
    const rootScope = this.createScriptRootScope(node.targetScript, argValues);
    this.frames.push({
      frameId: this.frameCounter++,
      groupId: targetScript.rootGroupId,
      nodeIndex: 0,
      scope: rootScope,
      completion: "none",
      scriptRoot: true,
      returnContinuation: continuation,
    });
  }

  private executeReturn(targetScript: string | null): void {
    const rootIndex = this.findCurrentRootFrameIndex();
    const rootFrame = this.frames[rootIndex];
    const inherited = rootFrame.returnContinuation;

    this.frames.splice(rootIndex);
    if (targetScript) {
      const script = this.scripts[targetScript];
      if (!script) {
        throw new ScriptLangError(
          "ENGINE_RETURN_TARGET",
          `Return target script "${targetScript}" is not registered.`
        );
      }
      const rootScope = this.createScriptRootScope(targetScript, {});
      this.frames.push({
        frameId: this.frameCounter++,
        groupId: script.rootGroupId,
        nodeIndex: 0,
        scope: rootScope,
        completion: "none",
        scriptRoot: true,
        returnContinuation: inherited,
      });
      return;
    }

    if (!inherited) {
      this.ended = true;
      this.frames = [];
      return;
    }
    const resumeFrame = this.findFrame(inherited.resumeFrameId);
    if (!resumeFrame) {
      this.ended = true;
      this.frames = [];
      return;
    }
    for (const [calleeVar, callerPath] of Object.entries(inherited.refBindings)) {
      const value = deepClone(rootFrame.scope[calleeVar]);
      this.writePath(callerPath, value);
    }
    resumeFrame.nodeIndex = inherited.nextNodeIndex;
  }

  private findCurrentRootFrameIndex(): number {
    for (let i = this.frames.length - 1; i >= 0; i -= 1) {
      if (this.frames[i].scriptRoot) {
        return i;
      }
    }
    throw new ScriptLangError("ENGINE_ROOT_FRAME", "No script root frame found.");
  }

  private createScriptRootScope(
    scriptPath: string,
    argValues: Record<string, unknown>
  ): Record<string, unknown> {
    const script = this.scripts[scriptPath];
    if (!script) {
      throw new ScriptLangError("ENGINE_SCRIPT_NOT_FOUND", `Script "${scriptPath}" is not registered.`);
    }
    const scope: Record<string, unknown> = {};

    for (const decl of script.vars) {
      let value = defaultValueFromVar(decl);
      if (decl.initialValueExpr) {
        value = this.evalExpression(decl.initialValueExpr, [scope]);
      }
      if (value === undefined) {
        throw new ScriptLangError(
          "ENGINE_VAR_UNDEFINED",
          `Initial value for "${decl.name}" cannot be undefined.`
        );
      }
      scope[decl.name] = value;
    }

    for (const [name, value] of Object.entries(argValues)) {
      if (!(name in scope)) {
        throw new ScriptLangError(
          "ENGINE_CALL_ARG_UNKNOWN",
          `Call argument "${name}" is not declared in target script vars.`
        );
      }
      if (value === undefined) {
        throw new ScriptLangError(
          "ENGINE_CALL_ARG_UNDEFINED",
          `Call argument "${name}" cannot be undefined.`
        );
      }
      scope[name] = value;
    }
    return scope;
  }

  private renderText(template: string): string {
    return template.replace(/\$\{([^{}]+)\}/g, (_all, expr) => {
      const value = this.evalExpression(String(expr));
      return String(value);
    });
  }

  private evalBoolean(expr: string): boolean {
    const value = this.evalExpression(expr);
    if (typeof value !== "boolean") {
      throw new ScriptLangError("ENGINE_BOOLEAN_EXPECTED", `Expression "${expr}" must evaluate to boolean.`);
    }
    return value;
  }

  private readPath(path: string): unknown {
    const parts = parseRefPath(path);
    if (parts.length === 0) {
      throw new ScriptLangError("ENGINE_REF_PATH", `Invalid ref path "${path}".`);
    }
    const [head, ...rest] = parts;
    let current = this.readVariable(head);
    for (const part of rest) {
      if (current && typeof current === "object" && part in (current as Record<string, unknown>)) {
        current = (current as Record<string, unknown>)[part];
      } else {
        throw new ScriptLangError("ENGINE_REF_PATH_READ", `Cannot resolve path "${path}".`);
      }
    }
    return current;
  }

  private writePath(path: string, value: unknown): void {
    const parts = parseRefPath(path);
    if (parts.length === 0) {
      throw new ScriptLangError("ENGINE_REF_PATH", `Invalid ref path "${path}".`);
    }
    if (value === undefined) {
      throw new ScriptLangError("ENGINE_UNDEFINED_WRITE", "undefined is not allowed in ScriptLang state.");
    }
    const [head, ...rest] = parts;
    if (rest.length === 0) {
      this.writeVariable(head, value);
      return;
    }

    const owner = this.readVariable(head);
    if (!owner || typeof owner !== "object") {
      throw new ScriptLangError("ENGINE_REF_PATH_WRITE", `Cannot resolve write path "${path}".`);
    }
    let current = owner as Record<string, unknown>;
    for (let i = 0; i < rest.length - 1; i += 1) {
      const key = rest[i];
      const next = current[key];
      if (!next || typeof next !== "object") {
        throw new ScriptLangError("ENGINE_REF_PATH_WRITE", `Cannot resolve write path "${path}".`);
      }
      current = next as Record<string, unknown>;
    }
    current[rest[rest.length - 1]] = value;
  }

  private evalExpression(expr: string, extraScopes: Array<Record<string, unknown>> = []): unknown {
    const sandbox = this.buildSandbox(extraScopes);
    const script = new vm.Script(`"use strict"; (${expr})`);
    return script.runInContext(sandbox, { timeout: this.vmTimeoutMs });
  }

  private runCode(code: string): void {
    const sandbox = this.buildSandbox([]);
    const script = new vm.Script(`"use strict";\n${code}`);
    script.runInContext(sandbox, { timeout: this.vmTimeoutMs });
  }

  private buildSandbox(extraScopes: Array<Record<string, unknown>>): vm.Context {
    const variableNames = new Set<string>();
    for (const frame of this.frames) {
      for (const name of Object.keys(frame.scope)) {
        variableNames.add(name);
      }
    }
    for (const scope of extraScopes) {
      for (const name of Object.keys(scope)) {
        variableNames.add(name);
      }
    }

    const sandbox: Record<string, unknown> = Object.create(null);
    for (const name of variableNames) {
      Object.defineProperty(sandbox, name, {
        configurable: false,
        enumerable: true,
        get: () => this.readVariable(name, extraScopes),
        set: (value: unknown) => {
          if (value === undefined) {
            throw new ScriptLangError(
              "ENGINE_UNDEFINED_ASSIGN",
              `Variable "${name}" cannot be assigned undefined.`
            );
          }
          this.writeVariable(name, value, extraScopes);
        },
      });
    }
    for (const [name, fn] of Object.entries(this.hostFunctions)) {
      Object.defineProperty(sandbox, name, {
        configurable: false,
        enumerable: true,
        writable: false,
        value: fn,
      });
    }
    Object.defineProperty(sandbox, "Math", {
      configurable: false,
      enumerable: true,
      writable: false,
      value: Math,
    });

    return vm.createContext(sandbox, {
      codeGeneration: {
        strings: false,
        wasm: false,
      },
    });
  }

  private readVariable(name: string, extraScopes: Array<Record<string, unknown>> = []): unknown {
    for (let i = extraScopes.length - 1; i >= 0; i -= 1) {
      if (name in extraScopes[i]) {
        return extraScopes[i][name];
      }
    }
    for (let i = this.frames.length - 1; i >= 0; i -= 1) {
      if (name in this.frames[i].scope) {
        return this.frames[i].scope[name];
      }
    }
    throw new ScriptLangError("ENGINE_VAR_READ", `Variable "${name}" is not defined.`);
  }

  private writeVariable(
    name: string,
    value: unknown,
    extraScopes: Array<Record<string, unknown>> = []
  ): void {
    for (let i = extraScopes.length - 1; i >= 0; i -= 1) {
      if (name in extraScopes[i]) {
        extraScopes[i][name] = value;
        return;
      }
    }
    for (let i = this.frames.length - 1; i >= 0; i -= 1) {
      if (name in this.frames[i].scope) {
        this.frames[i].scope[name] = value;
        return;
      }
    }
    throw new ScriptLangError(
      "ENGINE_VAR_WRITE",
      `Variable "${name}" is not declared in visible group scopes.`
    );
  }
}

