export type PrimitiveTypeName = "number" | "string" | "boolean" | "null";

export type ScriptType =
  | { kind: "primitive"; name: PrimitiveTypeName }
  | { kind: "array"; elementType: ScriptType }
  | { kind: "record"; valueType: ScriptType }
  | { kind: "map"; keyType: "string"; valueType: ScriptType };

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

interface BaseNode {
  id: string;
  kind: string;
  location: SourceSpan;
}

export interface TextNode extends BaseNode {
  kind: "text";
  value: string;
}

export interface CodeNode extends BaseNode {
  kind: "code";
  code: string;
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
  groupId: string;
  once: boolean;
  location: SourceSpan;
}

export interface ChoiceNode extends BaseNode {
  kind: "choice";
  options: ChoiceOption[];
}

export interface CallArgument {
  name: string;
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
}

export type ScriptNode =
  | TextNode
  | CodeNode
  | IfNode
  | WhileNode
  | ChoiceNode
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
  rootGroupId: string;
  groups: Record<string, ImplicitGroup>;
  vars: VarDeclaration[];
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

export interface SnapshotFrameV1 {
  frameId: number;
  groupId: string;
  nodeIndex: number;
  scope: Record<string, unknown>;
  completion:
    | { kind: "none" }
    | { kind: "whileBody" }
    | { kind: "resumeAfterChild" };
  scriptRoot: boolean;
  returnContinuation: ContinuationFrame | null;
}

export interface SnapshotV1 {
  schemaVersion: "snapshot.v1";
  compilerVersion: string;
  cursor: {
    groupPath: string[];
    nodeIndex: number;
  };
  scopeChain: RuntimeScopeFrame[];
  continuations: ContinuationFrame[];
  runtimeFrames: SnapshotFrameV1[];
  waitingChoice: boolean;
  pendingChoiceNodeId: string | null;
  selectedChoices: string[];
}

export interface ChoiceItem {
  index: number;
  id: string;
  text: string;
}

export type EngineOutput =
  | { kind: "text"; text: string }
  | { kind: "choices"; items: ChoiceItem[] }
  | { kind: "end" };

