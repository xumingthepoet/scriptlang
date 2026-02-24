export type PrimitiveTypeName = "number" | "string" | "boolean";

export type ScriptType =
  | { kind: "primitive"; name: PrimitiveTypeName }
  | { kind: "array"; elementType: ScriptType }
  | { kind: "map"; keyType: "string"; valueType: ScriptType }
  | { kind: "object"; typeName: string; fields: Record<string, ScriptType> };

export interface SourceLocation {
  line: number;
  column: number;
}

export interface SourceSpan {
  start: SourceLocation;
  end: SourceLocation;
}

export interface VarDeclaration {
  name: string;
  type: ScriptType;
  initialValueExpr: string | null;
  location: SourceSpan;
}

export interface ScriptParam {
  name: string;
  type: ScriptType;
  isRef: boolean;
  location: SourceSpan;
}

export interface FunctionParam {
  name: string;
  type: ScriptType;
  location: SourceSpan;
}

export interface FunctionReturn {
  name: string;
  type: ScriptType;
  location: SourceSpan;
}

export interface FunctionDecl {
  name: string;
  params: FunctionParam[];
  returnBinding: FunctionReturn;
  code: string;
  location: SourceSpan;
}

interface BaseNode {
  id: string;
  kind: string;
  location: SourceSpan;
}

export interface TextNode extends BaseNode {
  kind: "text";
  value: string;
  once: boolean;
}

export interface CodeNode extends BaseNode {
  kind: "code";
  code: string;
}

export interface VarNode extends BaseNode {
  kind: "var";
  declaration: VarDeclaration;
}

export interface IfNode extends BaseNode {
  kind: "if";
  whenExpr: string;
  thenGroupId: string;
  elseGroupId: string | null;
}

export interface WhileNode extends BaseNode {
  kind: "while";
  whenExpr: string;
  bodyGroupId: string;
}

export interface ChoiceOption {
  id: string;
  text: string;
  whenExpr: string | null;
  once: boolean;
  fallOver: boolean;
  groupId: string;
  location: SourceSpan;
}

export interface ChoiceNode extends BaseNode {
  kind: "choice";
  promptText: string;
  options: ChoiceOption[];
}

export interface InputNode extends BaseNode {
  kind: "input";
  targetVar: string;
  promptText: string;
}

export interface BreakNode extends BaseNode {
  kind: "break";
}

export interface ContinueNode extends BaseNode {
  kind: "continue";
  target: "while" | "choice";
}

export interface CallArgument {
  valueExpr: string;
  isRef: boolean;
}

export interface CallNode extends BaseNode {
  kind: "call";
  targetScript: string;
  args: CallArgument[];
}

export interface ReturnNode extends BaseNode {
  kind: "return";
  targetScript: string | null;
  args: CallArgument[];
}

export type ScriptNode =
  | VarNode
  | TextNode
  | CodeNode
  | IfNode
  | WhileNode
  | ChoiceNode
  | InputNode
  | BreakNode
  | ContinueNode
  | CallNode
  | ReturnNode;

export interface ImplicitGroup {
  groupId: string;
  parentGroupId: string | null;
  entryNodeId: string | null;
  nodes: ScriptNode[];
}

export interface ScriptIR {
  scriptPath: string;
  scriptName: string;
  params: ScriptParam[];
  rootGroupId: string;
  groups: Record<string, ImplicitGroup>;
  visibleJsonGlobals?: string[];
  visibleFunctions?: Record<string, FunctionDecl>;
}

export interface RuntimeScopeFrame {
  groupId: string;
  values: Record<string, unknown>;
}

export interface ContinuationFrame {
  resumeFrameId: number;
  nextNodeIndex: number;
  refBindings: Record<string, string>;
}

export interface SnapshotFrameV2 {
  frameId: number;
  groupId: string;
  nodeIndex: number;
  scope: Record<string, unknown>;
  varTypes?: Record<string, ScriptType>;
  completion:
    | { kind: "none" }
    | { kind: "whileBody" }
    | { kind: "resumeAfterChild" };
  scriptRoot: boolean;
  returnContinuation: ContinuationFrame | null;
}

export interface PendingChoiceBoundaryV2 {
  kind: "choice";
  nodeId: string;
  items: ChoiceItem[];
  promptText: string | null;
}

export interface PendingInputBoundaryV2 {
  kind: "input";
  nodeId: string;
  targetVar: string;
  promptText: string;
  defaultText: string;
}

export type PendingBoundaryV2 = PendingChoiceBoundaryV2 | PendingInputBoundaryV2;

export interface SnapshotV2 {
  schemaVersion: "snapshot.v2";
  compilerVersion: string;
  cursor: {
    groupPath: string[];
    nodeIndex: number;
  };
  scopeChain: RuntimeScopeFrame[];
  continuations: ContinuationFrame[];
  runtimeFrames: SnapshotFrameV2[];
  rngState: number;
  pendingBoundary: PendingBoundaryV2;
  onceStateByScript?: Record<string, string[]>;
}

export interface ChoiceItem {
  index: number;
  id: string;
  text: string;
}

export type EngineOutput =
  | { kind: "text"; text: string }
  | { kind: "choices"; items: ChoiceItem[]; promptText?: string }
  | { kind: "input"; promptText: string; defaultText: string }
  | { kind: "end" };
