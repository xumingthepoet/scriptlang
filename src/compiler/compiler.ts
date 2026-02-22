import { ScriptLangError } from "../core/errors.js";
import type {
  CallArgument,
  CallNode,
  ChoiceNode,
  ChoiceOption,
  CodeNode,
  IfNode,
  ImplicitGroup,
  ReturnNode,
  ScriptIR,
  ScriptNode,
  ScriptType,
  SourceSpan,
  TextNode,
  VarDeclaration,
  WhileNode,
} from "../core/types.js";
import { parseXmlDocument } from "./xml.js";
import type { XmlElementNode, XmlNode } from "./xml-types.js";

const UNSUPPORTED_NODES = new Set(["set", "push", "remove"]);
const PRIMITIVE_TYPES = new Set(["number", "string", "boolean", "null"]);
function getAttr(node: XmlElementNode, name: string, required = false): string | null {
  const value = node.attributes[name];
  if (required && (value === undefined || value === "")) {
    throw new ScriptLangError(
      "XML_MISSING_ATTR",
      `Missing required attribute "${name}" on <${node.name}>.`,
      node.location
    );
  }
  return value ?? null;
}

const asElements = (nodes: XmlNode[]): XmlElementNode[] => {
  return nodes.filter((n): n is XmlElementNode => n.kind === "element");
};

const stableBase = (scriptPath: string): string =>
  scriptPath.replace(/[^\w./-]+/g, "_");

class GroupBuilder {
  private groupCounter = 0;
  private nodeCounter = 0;
  private choiceCounter = 0;
  readonly groups: Record<string, ImplicitGroup> = {};

  constructor(private readonly scriptPath: string) {}

  nextGroupId(): string {
    const id = `${stableBase(this.scriptPath)}::g${this.groupCounter}`;
    this.groupCounter += 1;
    return id;
  }

  nextNodeId(kind: string): string {
    const id = `${stableBase(this.scriptPath)}::n${this.nodeCounter}:${kind}`;
    this.nodeCounter += 1;
    return id;
  }

  nextChoiceId(): string {
    const id = `${stableBase(this.scriptPath)}::c${this.choiceCounter}`;
    this.choiceCounter += 1;
    return id;
  }
}

const parseType = (raw: string, span: SourceSpan): ScriptType => {
  const source = raw.trim();
  if (PRIMITIVE_TYPES.has(source)) {
    return { kind: "primitive", name: source as "number" | "string" | "boolean" | "null" };
  }
  if (source.endsWith("[]")) {
    return { kind: "array", elementType: parseType(source.slice(0, -2), span) };
  }
  const recordMatch = source.match(/^Record<string,\s*(.+)>$/);
  if (recordMatch) {
    return { kind: "record", valueType: parseType(recordMatch[1], span) };
  }
  const mapMatch = source.match(/^Map<string,\s*(.+)>$/);
  if (mapMatch) {
    return { kind: "map", keyType: "string", valueType: parseType(mapMatch[1], span) };
  }
  throw new ScriptLangError("TYPE_PARSE_ERROR", `Unsupported type syntax: "${raw}".`, span);
};

const parseVars = (varsNode: XmlElementNode | null): VarDeclaration[] => {
  if (!varsNode) {
    return [];
  }
  const vars: VarDeclaration[] = [];
  const names = new Set<string>();
  for (const node of asElements(varsNode.children)) {
    if (node.name !== "var") {
      throw new ScriptLangError(
        "XML_INVALID_VAR_NODE",
        `Only <var> is allowed inside <vars>, got <${node.name}>.`,
        node.location
      );
    }
    const name = getAttr(node, "name", true) as string;
    if (names.has(name)) {
      throw new ScriptLangError(
        "XML_DUPLICATE_VAR",
        `Variable "${name}" is declared more than once in <vars>.`,
        node.location
      );
    }
    names.add(name);
    const typeSource = getAttr(node, "type", true) as string;
    const initialValueExpr = getAttr(node, "value", false);
    vars.push({
      name,
      type: parseType(typeSource, node.location),
      initialValueExpr,
      location: node.location,
    });
  }
  return vars;
};

const parseArgs = (raw: string | null): CallArgument[] => {
  if (!raw || raw.trim().length === 0) {
    return [];
  }
  return raw
    .split(",")
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const separator = part.indexOf(":");
      if (separator <= 0 || separator >= part.length - 1) {
        throw new ScriptLangError("CALL_ARGS_PARSE_ERROR", `Invalid call arg segment: "${part}".`);
      }
      const name = part.slice(0, separator).trim();
      const value = part.slice(separator + 1).trim();
      if (value.startsWith("ref:")) {
        return { name, valueExpr: value.slice("ref:".length), isRef: true };
      }
      return { name, valueExpr: value, isRef: false };
    });
};

const inlineTextContent = (node: XmlElementNode): string =>
  node.children
    .filter((child): child is Extract<XmlNode, { kind: "text" }> => child.kind === "text")
    .map((x) => x.value)
    .join("\n")
    .trim();

const compileGroup = (
  groupId: string,
  parentGroupId: string | null,
  container: XmlElementNode,
  builder: GroupBuilder
): void => {
  const nodes: ScriptNode[] = [];
  builder.groups[groupId] = {
    groupId,
    parentGroupId,
    entryNodeId: null,
    nodes,
  };

  for (const child of asElements(container.children)) {
    if (UNSUPPORTED_NODES.has(child.name)) {
      throw new ScriptLangError(
        "XML_UNSUPPORTED_NODE",
        `<${child.name}> is removed in ScriptLang V1; use <code> instead.`,
        child.location
      );
    }
    let compiled: ScriptNode | null = null;

    if (child.name === "text") {
      const textNode: TextNode = {
        id: builder.nextNodeId("text"),
        kind: "text",
        value: getAttr(child, "value", false) ?? inlineTextContent(child),
        location: child.location,
      };
      compiled = textNode;
    } else if (child.name === "code") {
      const codeNode: CodeNode = {
        id: builder.nextNodeId("code"),
        kind: "code",
        code: getAttr(child, "value", false) ?? inlineTextContent(child),
        location: child.location,
      };
      compiled = codeNode;
    } else if (child.name === "if") {
      const thenGroupId = builder.nextGroupId();
      const elseGroupId = builder.nextGroupId();
      const elseNode = asElements(child.children).find((x) => x.name === "else") ?? null;
      const thenContainer: XmlElementNode = {
        ...child,
        children: asElements(child.children).filter((x) => x.name !== "else"),
      };
      compileGroup(thenGroupId, groupId, thenContainer, builder);
      if (elseNode) {
        compileGroup(elseGroupId, groupId, elseNode, builder);
      } else {
        builder.groups[elseGroupId] = {
          groupId: elseGroupId,
          parentGroupId: groupId,
          entryNodeId: null,
          nodes: [],
        };
      }
      const ifNode: IfNode = {
        id: builder.nextNodeId("if"),
        kind: "if",
        whenExpr: getAttr(child, "when", true) as string,
        thenGroupId,
        elseGroupId,
        location: child.location,
      };
      compiled = ifNode;
    } else if (child.name === "while") {
      const bodyGroupId = builder.nextGroupId();
      compileGroup(bodyGroupId, groupId, child, builder);
      const whileNode: WhileNode = {
        id: builder.nextNodeId("while"),
        kind: "while",
        whenExpr: getAttr(child, "when", true) as string,
        bodyGroupId,
        location: child.location,
      };
      compiled = whileNode;
    } else if (child.name === "choice") {
      const options: ChoiceOption[] = [];
      for (const option of asElements(child.children)) {
        if (option.name !== "option") {
          throw new ScriptLangError(
            "XML_CHOICE_OPTION_INVALID",
            `<choice> only accepts <option>, got <${option.name}>.`,
            option.location
          );
        }
        const optionGroupId = builder.nextGroupId();
        compileGroup(optionGroupId, groupId, option, builder);
        options.push({
          id: builder.nextChoiceId(),
          text: getAttr(option, "text", true) as string,
          whenExpr: getAttr(option, "when", false),
          groupId: optionGroupId,
          once: getAttr(option, "once", false) === "true",
          location: option.location,
        });
      }
      const choiceNode: ChoiceNode = {
        id: builder.nextNodeId("choice"),
        kind: "choice",
        options,
        location: child.location,
      };
      compiled = choiceNode;
    } else if (child.name === "call") {
      compiled = {
        id: builder.nextNodeId("call"),
        kind: "call",
        targetScript: getAttr(child, "script", true) as string,
        args: parseArgs(getAttr(child, "args", false)),
        location: child.location,
      };
    } else if (child.name === "return") {
      const returnNode: ReturnNode = {
        id: builder.nextNodeId("return"),
        kind: "return",
        targetScript: getAttr(child, "script", false),
        location: child.location,
      };
      compiled = returnNode;
    } else {
      throw new ScriptLangError(
        "XML_UNKNOWN_NODE",
        `Unknown node <${child.name}> in executable section.`,
        child.location
      );
    }

    nodes.push(compiled);
    if (!builder.groups[groupId].entryNodeId) {
      builder.groups[groupId].entryNodeId = compiled.id;
    }
  }
};

const findFirstChildByName = (
  parent: XmlElementNode,
  name: string
): XmlElementNode | null => {
  return asElements(parent.children).find((child) => child.name === name) ?? null;
};

export const compileScript = (xmlSource: string, scriptPath: string): ScriptIR => {
  const document = parseXmlDocument(xmlSource);
  const root = document.root;
  if (root.name !== "script") {
    throw new ScriptLangError(
      "XML_INVALID_ROOT",
      `Expected <script> as root but got <${root.name}>.`,
      root.location
    );
  }

  const builder = new GroupBuilder(scriptPath);
  const rootGroupId = builder.nextGroupId();
  const varsNode = findFirstChildByName(root, "vars");
  const stepNode = findFirstChildByName(root, "step");
  const syntheticStep: XmlElementNode = {
    kind: "element",
    name: "step",
    attributes: {},
    children: stepNode ? stepNode.children : [],
    location: root.location,
  };
  compileGroup(rootGroupId, null, syntheticStep, builder);

  return {
    scriptPath,
    rootGroupId,
    groups: builder.groups,
    vars: parseVars(varsNode),
  };
};
