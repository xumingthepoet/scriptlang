import vm from "node:vm";

import { ScriptLangError } from "../core/errors.js";
import type {
  ChoiceItem,
  ChoiceNode,
  ContinuationFrame,
  EngineOutput,
  ScriptIR,
  ScriptNode,
  ScriptType,
  SnapshotFrameV1,
  SnapshotV1,
} from "../core/types.js";

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
  varTypes: Record<string, ScriptType>;
}

interface GroupLookup {
  scriptName: string;
  group: ScriptIR["groups"][string];
}

interface PendingChoice {
  frameId: number;
  nodeId: string;
  options: ChoiceItem[];
  promptText: string | null;
}

const deepClone = <T>(value: T): T => structuredClone(value);
const isObjectLike = (value: unknown): value is object => value !== null && typeof value === "object";

const isPrimitiveTypeName = (name: string): name is "number" | "string" | "boolean" => {
  return name === "number" || name === "string" || name === "boolean";
};

const assertSupportedPrimitiveType = (
  typeName: string,
  errorCode: "ENGINE_TYPE_MISMATCH" | "SNAPSHOT_TYPE_UNSUPPORTED"
): void => {
  if (!isPrimitiveTypeName(typeName)) {
    throw new ScriptLangError(errorCode, `Unsupported primitive type "${typeName}".`);
  }
};

const isMapValue = (value: unknown): value is Map<unknown, unknown> => {
  return Object.prototype.toString.call(value) === "[object Map]";
};

const isPlainObject = (value: unknown): value is Record<string, unknown> => {
  if (value === null || typeof value !== "object") {
    return false;
  }
  if (Array.isArray(value) || isMapValue(value)) {
    return false;
  }
  return true;
};

const defaultValueFromType = (type: ScriptType): unknown => {
  if (type.kind === "primitive") {
    assertSupportedPrimitiveType(type.name, "ENGINE_TYPE_MISMATCH");
    if (type.name === "number") return 0;
    if (type.name === "string") return "";
    return false;
  }
  if (type.kind === "array") return [];
  if (type.kind === "map") return new Map<string, unknown>();
  const value: Record<string, unknown> = {};
  for (const [fieldName, fieldType] of Object.entries(type.fields)) {
    value[fieldName] = defaultValueFromType(fieldType);
  }
  return value;
};

const parseRefPath = (path: string): string[] => {
  return path
    .split(".")
    .map((part) => part.trim())
    .filter(Boolean);
};

const MAX_UINT32 = 0xffffffff;
const DEFAULT_RANDOM_SEED = 1;

const isUint32Integer = (value: unknown): value is number => {
  return typeof value === "number" && Number.isInteger(value) && value >= 0 && value <= MAX_UINT32;
};

const isTypeCompatible = (value: unknown, type: ScriptType): boolean => {
  if (value === undefined) {
    return false;
  }
  if (type.kind === "primitive") {
    assertSupportedPrimitiveType(type.name, "ENGINE_TYPE_MISMATCH");
    return typeof value === type.name;
  }
  if (type.kind === "array") {
    return Array.isArray(value) && value.every((item) => isTypeCompatible(item, type.elementType));
  }
  if (type.kind === "map") {
    if (!isMapValue(value)) {
      return false;
    }
    for (const [key, mapValue] of value.entries()) {
      if (typeof key !== "string" || !isTypeCompatible(mapValue, type.valueType)) {
        return false;
      }
    }
    return true;
  }

  if (!isPlainObject(value)) {
    return false;
  }
  const keys = Object.keys(value);
  const expectedKeys = Object.keys(type.fields);
  if (keys.length !== expectedKeys.length) {
    return false;
  }
  for (let i = 0; i < expectedKeys.length; i += 1) {
    const key = expectedKeys[i];
    if (!(key in value)) {
      return false;
    }
    if (!isTypeCompatible(value[key], type.fields[key])) {
      return false;
    }
  }
  return true;
};

const assertSnapshotTypeSupported = (type: ScriptType): void => {
  if (type.kind === "primitive") {
    assertSupportedPrimitiveType(type.name, "SNAPSHOT_TYPE_UNSUPPORTED");
    return;
  }
  if (type.kind === "array") {
    assertSnapshotTypeSupported(type.elementType);
    return;
  }
  if (type.kind === "map") {
    assertSnapshotTypeSupported(type.valueType);
    return;
  }
  for (const fieldType of Object.values(type.fields)) {
    assertSnapshotTypeSupported(fieldType);
  }
};

export interface ScriptLangEngineOptions {
  scripts: Record<string, ScriptIR>;
  globalJson?: Record<string, unknown>;
  hostFunctions?: HostFunctionMap;
  randomSeed?: number;
  compilerVersion?: string;
}

export class ScriptLangEngine {
  private readonly scripts: Record<string, ScriptIR>;
  private readonly hostFunctions: HostFunctionMap;
  private readonly compilerVersion: string;
  private readonly groupLookup: Record<string, GroupLookup>;
  private readonly globalJsonRaw: Record<string, unknown>;
  private readonly globalJsonReadonly: Record<string, unknown>;
  private readonly visibleJsonByScript: Record<string, Set<string>>;
  private readonly initialRandomSeed: number;
  private readonly readonlyProxyByRaw = new WeakMap<object, unknown>();
  private readonly rawByReadonlyProxy = new WeakMap<object, object>();

  private frames: RuntimeFrame[] = [];
  private pendingChoice: PendingChoice | null = null;
  public waitingChoice = false;
  private frameCounter = 1;
  private ended = false;
  private rngState = DEFAULT_RANDOM_SEED;

  constructor(options: ScriptLangEngineOptions) {
    this.scripts = options.scripts;
    this.globalJsonRaw = options.globalJson ?? {};
    this.hostFunctions = options.hostFunctions ?? {};
    this.compilerVersion = options.compilerVersion ?? "dev";
    this.initialRandomSeed = this.normalizeRandomSeed(options.randomSeed);
    this.rngState = this.initialRandomSeed;
    this.groupLookup = {};
    this.globalJsonReadonly = {};
    this.visibleJsonByScript = {};

    if (Object.hasOwn(this.hostFunctions, "random")) {
      throw new ScriptLangError(
        "ENGINE_HOST_FUNCTION_RESERVED",
        'hostFunctions cannot register reserved builtin name "random".'
      );
    }

    for (const [scriptName, script] of Object.entries(this.scripts)) {
      for (const [groupId, group] of Object.entries(script.groups)) {
        this.groupLookup[groupId] = { scriptName, group };
      }
      this.visibleJsonByScript[scriptName] = new Set(script.visibleJsonGlobals ?? []);
    }

    for (const [name, value] of Object.entries(this.globalJsonRaw)) {
      this.globalJsonReadonly[name] = this.makeReadonlyGlobalValue(value);
    }
  }

  start(entryScriptName: string): void {
    this.reset();
    const entry = this.scripts[entryScriptName];
    if (!entry) {
      throw new ScriptLangError(
        "ENGINE_SCRIPT_NOT_FOUND",
        `Entry script "${entryScriptName}" is not registered.`
      );
    }
    const { scope: rootScope, varTypes } = this.createScriptRootScope(entryScriptName, {});
    this.pushRootFrame(entry.rootGroupId, rootScope, null, varTypes);
  }

  next(): EngineOutput {
    if (this.pendingChoice) {
      return this.toChoiceOutput(this.pendingChoice.options, this.pendingChoice.promptText);
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
      if (!lookup) throw new ScriptLangError("ENGINE_GROUP_NOT_FOUND", `Group "${top.groupId}" is not registered.`);
      const group = lookup.group;

      if (top.nodeIndex >= group.nodes.length) {
        this.finishFrame(top);
      } else {
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
        if (node.kind === "var") {
          this.executeVarDeclaration(node.declaration);
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
            .map((opt, index) => ({
              index,
              id: opt.id,
              text: this.renderText(opt.text),
            }));
          if (options.length === 0) {
            top.nodeIndex += 1;
            continue;
          }
          const promptText = node.promptText ? this.renderText(node.promptText) : null;
          this.pendingChoice = { frameId: top.frameId, nodeId: node.id, options, promptText };
          this.waitingChoice = true;
          return this.toChoiceOutput(options, promptText);
        }
        if (node.kind === "call") {
          this.executeCall(node);
          continue;
        }
        if (node.kind === "return") {
          this.executeReturn(node);
          continue;
        }

        throw new ScriptLangError("ENGINE_NODE_UNKNOWN", `Unhandled node kind: ${(node as ScriptNode).kind}`);
      }
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

    frame.nodeIndex += 1;
    this.pushGroupFrame(option.groupId, "resumeAfterChild");
    this.pendingChoice = null;
    this.waitingChoice = false;
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
      varTypes: deepClone(frame.varTypes),
      completion: { kind: frame.completion },
      scriptRoot: frame.scriptRoot,
      returnContinuation: frame.returnContinuation ? deepClone(frame.returnContinuation) : null,
    }));
    const topFrame = this.requireSnapshotTopFrame();
    return {
      schemaVersion: "snapshot.v1",
      compilerVersion: this.compilerVersion,
      cursor: {
        groupPath: this.frames.map((f) => f.groupId),
        nodeIndex: topFrame.nodeIndex,
      },
      scopeChain: this.frames.map((f) => ({
        groupId: f.groupId,
        values: deepClone(f.scope),
      })),
      continuations: this.frames
        .filter((f) => f.scriptRoot && f.returnContinuation)
        .map((f) => deepClone(f.returnContinuation as ContinuationFrame)),
      runtimeFrames,
      rngState: this.rngState,
      waitingChoice: true,
      pendingChoiceNodeId: this.pendingChoice.nodeId,
      pendingChoiceItems: this.pendingChoice.options.map((item) => ({ ...item })),
      pendingChoicePromptText: this.pendingChoice.promptText,
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
    if (!isUint32Integer(snapshot.rngState)) {
      throw new ScriptLangError(
        "SNAPSHOT_RNG_STATE",
        "Snapshot rngState must be a uint32 integer."
      );
    }
    this.assertSnapshotPendingChoiceItems(snapshot.pendingChoiceItems);
    this.assertSnapshotPendingChoicePromptText(snapshot.pendingChoicePromptText);

    this.reset();
    this.rngState = snapshot.rngState;
    this.frames = snapshot.runtimeFrames.map((frame) => this.restoreFrame(frame));
    this.frameCounter = this.frames.reduce((max, frame) => (frame.frameId > max ? frame.frameId : max), 0) + 1;
    const top = this.requireSnapshotTopFrame();
    const node = this.requirePendingChoiceNode(top, snapshot.pendingChoiceNodeId);
    const options = snapshot.pendingChoiceItems.map((item) => ({ ...item }));
    this.pendingChoice = {
      frameId: top.frameId,
      nodeId: node.id,
      options,
      promptText: snapshot.pendingChoicePromptText ?? null,
    };
    this.waitingChoice = true;
  }

  private reset(): void {
    this.frames = [];
    this.pendingChoice = null;
    this.waitingChoice = false;
    this.ended = false;
    this.frameCounter = 1;
    this.rngState = this.initialRandomSeed;
  }

  private throwReadonlyGlobalMutation(name: string | null = null): never {
    if (name) {
      throw new ScriptLangError(
        "ENGINE_GLOBAL_READONLY",
        `Global JSON "${name}" is readonly and cannot be mutated.`
      );
    }
    throw new ScriptLangError(
      "ENGINE_GLOBAL_READONLY",
      "Global JSON data is readonly and cannot be mutated."
    );
  }

  private makeReadonlyGlobalValue(value: unknown): unknown {
    if (!isObjectLike(value)) {
      return value;
    }
    const cached = this.readonlyProxyByRaw.get(value);
    if (cached) {
      return cached;
    }

    const proxy = new Proxy(value, {
      get: (target, prop, receiver) => {
        const next = Reflect.get(target, prop, receiver);
        return this.makeReadonlyGlobalValue(next);
      },
      set: () => this.throwReadonlyGlobalMutation(),
      deleteProperty: () => this.throwReadonlyGlobalMutation(),
      defineProperty: () => this.throwReadonlyGlobalMutation(),
      setPrototypeOf: () => this.throwReadonlyGlobalMutation(),
    });
    this.readonlyProxyByRaw.set(value, proxy);
    this.rawByReadonlyProxy.set(proxy, value);
    return proxy;
  }

  private normalizeStoredValue(value: unknown): unknown {
    if (!isObjectLike(value)) {
      return value;
    }
    const raw = this.rawByReadonlyProxy.get(value);
    if (!raw) {
      return value;
    }
    return deepClone(raw);
  }

  private resolveCurrentScriptName(): string | null {
    const top = this.frames[this.frames.length - 1];
    if (!top) {
      return null;
    }
    const lookup = this.groupLookup[top.groupId];
    if (!lookup) {
      return null;
    }
    return lookup.scriptName;
  }

  private isVisibleJsonGlobal(scriptName: string | null, name: string): boolean {
    if (!scriptName) {
      return false;
    }
    const visible = this.visibleJsonByScript[scriptName];
    if (!visible || !visible.has(name)) {
      return false;
    }
    return name in this.globalJsonReadonly;
  }

  private findFrame(frameId: number): RuntimeFrame | null {
    return this.frames.find((frame) => frame.frameId === frameId) ?? null;
  }

  private pushRootFrame(
    groupId: string,
    scope: Record<string, unknown>,
    returnContinuation: ContinuationFrame | null,
    varTypes: Record<string, ScriptType>
  ): void {
    this.frames.push({
      frameId: this.frameCounter++,
      groupId,
      nodeIndex: 0,
      scope,
      completion: "none",
      scriptRoot: true,
      returnContinuation,
      varTypes,
    });
  }

  private restoreFrame(frame: SnapshotFrameV1): RuntimeFrame {
    const lookup = this.groupLookup[frame.groupId];
    if (!lookup) {
      throw new ScriptLangError("SNAPSHOT_GROUP_MISSING", `Group "${frame.groupId}" is unknown.`);
    }
    const varTypes = frame.varTypes
      ? deepClone(frame.varTypes)
      : frame.scriptRoot
        ? this.buildParamTypeMap(lookup.scriptName)
        : {};
    for (const type of Object.values(varTypes)) {
      assertSnapshotTypeSupported(type);
    }
    return {
      frameId: frame.frameId,
      groupId: frame.groupId,
      nodeIndex: frame.nodeIndex,
      scope: deepClone(frame.scope),
      completion: frame.completion.kind,
      scriptRoot: frame.scriptRoot,
      returnContinuation: frame.returnContinuation ? deepClone(frame.returnContinuation) : null,
      varTypes,
    };
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
      varTypes: {},
    });
  }

  private finishFrame(frame: RuntimeFrame): void {
    this.frames.pop();
    if (!frame.scriptRoot) {
      return;
    }
    const continuation = frame.returnContinuation;
    if (!continuation) {
      this.endExecution();
      return;
    }

    const resumeFrame = this.findFrame(continuation.resumeFrameId);
    if (!resumeFrame) {
      this.endExecution();
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
    for (let i = 0; i < node.args.length; i += 1) {
      const arg = node.args[i];
      const param = targetScript.params[i];
      if (!param) {
        throw new ScriptLangError(
          "ENGINE_CALL_ARG_UNKNOWN",
          `Call argument at position ${i + 1} has no declared target script parameter.`
        );
      }
      if (param.isRef && !arg.isRef) {
        throw new ScriptLangError(
          "ENGINE_CALL_REF_MISMATCH",
          `Call argument at position ${i + 1} must use ref mode because target script declares "${param.name}" as ref.`
        );
      }
      if (!param.isRef && arg.isRef) {
        throw new ScriptLangError(
          "ENGINE_CALL_REF_MISMATCH",
          `Call argument at position ${i + 1} cannot use ref mode because target script does not declare "${param.name}" as ref.`
        );
      }
      if (arg.isRef) {
        argValues[param.name] = this.readPath(arg.valueExpr);
        refBindings[param.name] = arg.valueExpr;
      } else {
        argValues[param.name] = this.evalExpression(arg.valueExpr);
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
      const { scope: rootScope, varTypes } = this.createScriptRootScope(
        node.targetScript,
        argValues
      );
      this.pushRootFrame(targetScript.rootGroupId, rootScope, inherited, varTypes);
      return;
    }

    const continuation: ContinuationFrame = {
      resumeFrameId: caller.frameId,
      nextNodeIndex: caller.nodeIndex + 1,
      refBindings,
    };
    const { scope: rootScope, varTypes } = this.createScriptRootScope(node.targetScript, argValues);
    this.pushRootFrame(targetScript.rootGroupId, rootScope, continuation, varTypes);
  }

  private executeReturn(node: Extract<ScriptNode, { kind: "return" }>): void {
    const rootIndex = this.findCurrentRootFrameIndex();
    const rootFrame = this.frames[rootIndex];
    const inherited = rootFrame.returnContinuation;
    const targetScript = node.targetScript;
    let target: ScriptIR | null = null;
    const transferArgValues: Record<string, unknown> = {};

    if (targetScript) {
      target = this.requireReturnTargetScript(targetScript);
      for (let i = 0; i < node.args.length; i += 1) {
        const arg = node.args[i];
        const param = target.params[i];
        if (!param) {
          throw new ScriptLangError(
            "ENGINE_RETURN_ARG_UNKNOWN",
            `Return argument at position ${i + 1} has no declared target script parameter.`,
            node.location
          );
        }
        transferArgValues[param.name] = this.evalExpression(arg.valueExpr);
      }
    }

    this.frames.splice(rootIndex);
    if (targetScript) {
      const script = target as ScriptIR;

      let forwardedContinuation = inherited;
      if (inherited) {
        const resumeFrame = this.findFrame(inherited.resumeFrameId);
        if (resumeFrame) {
          for (const [calleeVar, callerPath] of Object.entries(inherited.refBindings)) {
            const value = deepClone(rootFrame.scope[calleeVar]);
            this.writePath(callerPath, value);
          }
        }
        forwardedContinuation = {
          ...inherited,
          refBindings: {},
        };
      }

      const { scope: rootScope, varTypes } = this.createScriptRootScope(targetScript, transferArgValues);
      this.pushRootFrame(script.rootGroupId, rootScope, forwardedContinuation, varTypes);
      return;
    }

    if (!inherited) {
      this.endExecution();
      return;
    }
    const resumeFrame = this.findFrame(inherited.resumeFrameId);
    if (!resumeFrame) {
      this.endExecution();
      return;
    }
    for (const [calleeVar, callerPath] of Object.entries(inherited.refBindings)) {
      const value = deepClone(rootFrame.scope[calleeVar]);
      this.writePath(callerPath, value);
    }
    resumeFrame.nodeIndex = inherited.nextNodeIndex;
  }

  private requireReturnTargetScript(targetScript: string): ScriptIR {
    const script = this.scripts[targetScript];
    if (!script) throw new ScriptLangError("ENGINE_RETURN_TARGET", `Return target script "${targetScript}" is not registered.`);
    return script;
  }

  private findCurrentRootFrameIndex(): number {
    for (let i = this.frames.length - 1; i >= 0; i -= 1) {
      if (this.frames[i].scriptRoot) {
        return i;
      }
    }
    throw new ScriptLangError("ENGINE_ROOT_FRAME", "No script root frame found.");
  }

  private buildParamTypeMap(scriptName: string): Record<string, ScriptType> {
    const script = this.scripts[scriptName];
    if (!script) {
      throw new ScriptLangError("ENGINE_SCRIPT_NOT_FOUND", `Script "${scriptName}" is not registered.`);
    }
    return Object.fromEntries(script.params.map((param) => [param.name, param.type]));
  }

  private assertType(name: string, type: ScriptType, value: unknown): void {
    if (!isTypeCompatible(value, type)) {
      throw new ScriptLangError(
        "ENGINE_TYPE_MISMATCH",
        `Variable "${name}" does not match declared type.`
      );
    }
  }

  private createScriptRootScope(scriptName: string, argValues: Record<string, unknown>): {
    scope: Record<string, unknown>;
    varTypes: Record<string, ScriptType>;
  } {
    const script = this.scripts[scriptName];
    if (!script) throw new ScriptLangError("ENGINE_SCRIPT_NOT_FOUND", `Script "${scriptName}" is not registered.`);
    const scope: Record<string, unknown> = {};
    const varTypes = this.buildParamTypeMap(scriptName);
    for (let i = 0; i < script.params.length; i += 1) {
      const param = script.params[i];
      const value = defaultValueFromType(param.type);
      this.assertType(param.name, param.type, value);
      scope[param.name] = value;
    }

    for (const [name, value] of Object.entries(argValues)) {
      if (!(name in scope)) {
        throw new ScriptLangError(
          "ENGINE_CALL_ARG_UNKNOWN",
          `Call argument "${name}" is not declared in target script args.`
        );
      }
      const normalized = this.normalizeStoredValue(value);
      if (normalized === undefined) {
        throw new ScriptLangError(
          "ENGINE_CALL_ARG_UNDEFINED",
          `Call argument "${name}" cannot be undefined.`
        );
      }
      this.assertType(name, varTypes[name], normalized);
      scope[name] = normalized;
    }
    return { scope, varTypes };
  }

  private executeVarDeclaration(decl: { name: string; type: ScriptType; initialValueExpr: string | null }): void {
    const frame = this.frames[this.frames.length - 1];
    if (!frame) {
      throw new ScriptLangError("ENGINE_VAR_FRAME", "No frame available for var declaration.");
    }
    if (decl.name in frame.scope) {
      throw new ScriptLangError(
        "ENGINE_VAR_DUPLICATE",
        `Variable "${decl.name}" is already declared in the current block scope.`
      );
    }
    let value = defaultValueFromType(decl.type);
    if (decl.initialValueExpr) {
      value = this.evalExpression(decl.initialValueExpr);
    }
    const normalized = this.normalizeStoredValue(value);
    if (normalized === undefined) {
      throw new ScriptLangError(
        "ENGINE_VAR_UNDEFINED",
        `Initial value for "${decl.name}" cannot be undefined.`
      );
    }
    this.assertType(decl.name, decl.type, normalized);
    frame.scope[decl.name] = normalized;
    frame.varTypes[decl.name] = decl.type;
  }

  private requireSnapshotTopFrame(): RuntimeFrame {
    const top = this.frames[this.frames.length - 1];
    if (!top) throw new ScriptLangError("SNAPSHOT_EMPTY", "Snapshot contains no runtime frames.");
    return top;
  }

  private requirePendingChoiceNode(top: RuntimeFrame, pendingChoiceNodeId: string | null): ChoiceNode {
    const group = this.groupLookup[top.groupId].group;
    const node = group.nodes[top.nodeIndex];
    if (!node || node.kind !== "choice" || node.id !== pendingChoiceNodeId) {
      throw new ScriptLangError("SNAPSHOT_PENDING_CHOICE", "Pending choice node cannot be reconstructed.");
    }
    return node;
  }

  private endExecution(): void {
    this.ended = true;
    this.frames = [];
  }

  private normalizeRandomSeed(seedValue: number | undefined): number {
    const resolved = seedValue ?? DEFAULT_RANDOM_SEED;
    if (!isUint32Integer(resolved)) {
      throw new ScriptLangError(
        "ENGINE_RANDOM_SEED_INVALID",
        "randomSeed must be an integer in [0, 4294967295]."
      );
    }
    return resolved >>> 0;
  }

  private nextRandomUint32(): number {
    let next = (this.rngState + 0x6d2b79f5) >>> 0;
    this.rngState = next;
    next = Math.imul(next ^ (next >>> 15), next | 1);
    next ^= next + Math.imul(next ^ (next >>> 7), next | 61);
    return (next ^ (next >>> 14)) >>> 0;
  }

  private evalRandomBuiltin(args: unknown[]): number {
    if (args.length !== 0) {
      throw new ScriptLangError("ENGINE_RANDOM_ARITY", "random() expects zero arguments.");
    }
    return this.nextRandomUint32();
  }

  private assertSnapshotPendingChoiceItems(value: unknown): asserts value is ChoiceItem[] {
    if (!Array.isArray(value)) {
      throw new ScriptLangError(
        "SNAPSHOT_PENDING_CHOICE_ITEMS",
        "Snapshot pendingChoiceItems must be an array."
      );
    }
    for (let i = 0; i < value.length; i += 1) {
      const item = value[i];
      if (
        !item ||
        typeof item !== "object" ||
        !Number.isInteger((item as { index?: unknown }).index) ||
        typeof (item as { id?: unknown }).id !== "string" ||
        typeof (item as { text?: unknown }).text !== "string"
      ) {
        throw new ScriptLangError(
          "SNAPSHOT_PENDING_CHOICE_ITEMS",
          "Snapshot pendingChoiceItems contains invalid choice item payload."
        );
      }
    }
  }

  private assertSnapshotPendingChoicePromptText(value: unknown): asserts value is string | null | undefined {
    if (value === undefined || value === null || typeof value === "string") {
      return;
    }
    throw new ScriptLangError(
      "SNAPSHOT_PENDING_CHOICE_PROMPT_TEXT",
      "Snapshot pendingChoicePromptText must be string, null, or undefined."
    );
  }

  private toChoiceOutput(options: ChoiceItem[], promptText: string | null): EngineOutput {
    if (promptText === null) {
      return { kind: "choices", items: options };
    }
    return { kind: "choices", items: options, promptText };
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
    return script.runInContext(sandbox);
  }

  private runCode(code: string): void {
    const sandbox = this.buildSandbox([]);
    const script = new vm.Script(`"use strict";\n${code}`);
    script.runInContext(sandbox);
  }

  private buildSandbox(extraScopes: Array<Record<string, unknown>>): vm.Context {
    const scriptName = this.resolveCurrentScriptName();
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
    if (scriptName) {
      const visibleJson = this.visibleJsonByScript[scriptName];
      if (visibleJson) {
        for (const name of visibleJson) {
          variableNames.add(name);
        }
      }
    }

    const sandbox: Record<string, unknown> = Object.create(null);
    for (const name of variableNames) {
      Object.defineProperty(sandbox, name, {
        configurable: false,
        enumerable: true,
        get: () => this.readVariable(name, extraScopes, scriptName),
        set: (value: unknown) => {
          if (value === undefined) {
            throw new ScriptLangError(
              "ENGINE_UNDEFINED_ASSIGN",
              `Variable "${name}" cannot be assigned undefined.`
            );
          }
          this.writeVariable(name, value, extraScopes, scriptName);
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
    if (!Object.hasOwn(sandbox, "random")) {
      Object.defineProperty(sandbox, "random", {
        configurable: false,
        enumerable: true,
        writable: false,
        value: (...args: unknown[]) => this.evalRandomBuiltin(args),
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

  private readVariable(
    name: string,
    extraScopes: Array<Record<string, unknown>> = [],
    scriptName: string | null = this.resolveCurrentScriptName()
  ): unknown {
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
    if (this.isVisibleJsonGlobal(scriptName, name)) {
      return this.globalJsonReadonly[name];
    }
    throw new ScriptLangError("ENGINE_VAR_READ", `Variable "${name}" is not defined.`);
  }

  private writeVariable(
    name: string,
    value: unknown,
    extraScopes: Array<Record<string, unknown>> = [],
    scriptName: string | null = this.resolveCurrentScriptName()
  ): void {
    for (let i = extraScopes.length - 1; i >= 0; i -= 1) {
      if (name in extraScopes[i]) {
        extraScopes[i][name] = this.normalizeStoredValue(value);
        return;
      }
    }
    for (let i = this.frames.length - 1; i >= 0; i -= 1) {
      if (name in this.frames[i].scope) {
        const type = this.frames[i].varTypes[name];
        const normalized = this.normalizeStoredValue(value);
        if (type) {
          this.assertType(name, type, normalized);
        }
        this.frames[i].scope[name] = normalized;
        return;
      }
    }
    if (this.isVisibleJsonGlobal(scriptName, name)) {
      this.throwReadonlyGlobalMutation(name);
      return;
    }
    throw new ScriptLangError(
      "ENGINE_VAR_WRITE",
      `Variable "${name}" is not declared in visible group scopes.`
    );
  }
}
