import path from "node:path";

import { ScriptLangError } from "../core/errors.js";
import type {
  BreakNode,
  CallArgument,
  CallNode,
  ChoiceNode,
  ChoiceOption,
  CodeNode,
  ContinueNode,
  IfNode,
  ImplicitGroup,
  ReturnNode,
  ScriptIR,
  ScriptNode,
  ScriptParam,
  ScriptType,
  SourceSpan,
  TextNode,
  VarDeclaration,
  VarNode,
  WhileNode,
} from "../core/types.js";
import { expandScriptMacros } from "./macros.js";
import { parseXmlDocument } from "./xml.js";
import type { XmlElementNode, XmlNode } from "./xml-types.js";

const REMOVED_NODES = new Set(["vars", "step", "set", "push", "remove"]);
const INCLUDE_DIRECTIVE = /^<!--\s*include:\s*(.+?)\s*-->$/;
const CUSTOM_TYPE_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;
const JSON_GLOBAL_NAME_PATTERN = /^[$A-Za-z_][$0-9A-Za-z_]*$/;

type NamedTypeResolver = ((name: string, span: SourceSpan) => ScriptType) | undefined;

type ParsedTypeExpr =
  | { kind: "primitive"; name: "number" | "string" | "boolean" }
  | { kind: "array"; elementType: ParsedTypeExpr }
  | { kind: "map"; valueType: ParsedTypeExpr }
  | { kind: "custom"; name: string };

interface ParsedTypeFieldDecl {
  name: string;
  typeExpr: ParsedTypeExpr;
  location: SourceSpan;
}

interface ParsedTypeDecl {
  name: string;
  fields: ParsedTypeFieldDecl[];
  location: SourceSpan;
  sourcePath: string;
  collectionName: string;
}

export interface CompileScriptOptions {
  resolveNamedType?: (name: string, span: SourceSpan) => ScriptType;
  visibleJsonGlobals?: string[];
}

export interface CompileProjectBundleFromXmlMapResult {
  scripts: Record<string, ScriptIR>;
  globalJson: Record<string, unknown>;
}

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

const getRequiredNonEmptyAttr = (node: XmlElementNode, name: string): string => {
  const raw = node.attributes[name];
  if (raw === undefined || raw === "") {
    throw new ScriptLangError(
      "XML_MISSING_ATTR",
      `Missing required attribute "${name}" on <${node.name}>.`,
      node.location
    );
  }
  if (raw.trim().length === 0) {
    throw new ScriptLangError(
      "XML_EMPTY_ATTR",
      `Attribute "${name}" on <${node.name}> cannot be empty.`,
      node.location
    );
  }
  return raw;
};

const parseBooleanAttr = (node: XmlElementNode, name: string, defaultValue = false): boolean => {
  const raw = node.attributes[name];
  if (raw === undefined) {
    return defaultValue;
  }
  const normalized = raw.trim();
  if (normalized === "true") {
    return true;
  }
  if (normalized === "false") {
    return false;
  }
  throw new ScriptLangError(
    "XML_ATTR_BOOL_INVALID",
    `Attribute "${name}" on <${node.name}> must be "true" or "false".`,
    node.location
  );
};

const asElements = (nodes: XmlNode[]): XmlElementNode[] => {
  return nodes.filter((n): n is XmlElementNode => n.kind === "element");
};

const stableBase = (scriptPath: string): string => scriptPath.replace(/[^\w./-]+/g, "_");

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

const parseTypeExpr = (raw: string, span: SourceSpan): ParsedTypeExpr => {
  const source = raw.trim();
  if (source === "number" || source === "string" || source === "boolean") {
    return { kind: "primitive", name: source };
  }
  if (source.endsWith("[]")) {
    return { kind: "array", elementType: parseTypeExpr(source.slice(0, -2), span) };
  }
  const mapMatch = source.match(/^Map<string,\s*(.+)>$/);
  if (mapMatch) {
    return { kind: "map", valueType: parseTypeExpr(mapMatch[1], span) };
  }
  if (CUSTOM_TYPE_NAME_PATTERN.test(source)) {
    return { kind: "custom", name: source };
  }
  throw new ScriptLangError("TYPE_PARSE_ERROR", `Unsupported type syntax: "${raw}".`, span);
};

const resolveTypeExpr = (
  expr: ParsedTypeExpr,
  span: SourceSpan,
  resolveNamedType?: (name: string, span: SourceSpan) => ScriptType
): ScriptType => {
  if (expr.kind === "primitive") {
    return { kind: "primitive", name: expr.name };
  }
  if (expr.kind === "array") {
    return { kind: "array", elementType: resolveTypeExpr(expr.elementType, span, resolveNamedType) };
  }
  if (expr.kind === "map") {
    return { kind: "map", keyType: "string", valueType: resolveTypeExpr(expr.valueType, span, resolveNamedType) };
  }
  if (!resolveNamedType) {
    throw new ScriptLangError("TYPE_PARSE_ERROR", `Unsupported type syntax: "${expr.name}".`, span);
  }
  return resolveNamedType(expr.name, span);
};

const parseType = (
  raw: string,
  span: SourceSpan,
  resolveNamedType?: (name: string, span: SourceSpan) => ScriptType
): ScriptType => {
  return resolveTypeExpr(parseTypeExpr(raw, span), span, resolveNamedType);
};

const splitByTopLevelComma = (raw: string): string[] => {
  const parts: string[] = [];
  let current = "";
  let angleDepth = 0;
  let parenDepth = 0;
  let bracketDepth = 0;
  let braceDepth = 0;
  let quote: "\"" | "'" | null = null;

  for (let i = 0; i < raw.length; i += 1) {
    const ch = raw[i];
    if (quote) {
      current += ch;
      if (ch === quote && raw[i - 1] !== "\\") {
        quote = null;
      }
      continue;
    }
    if (ch === "\"" || ch === "'") {
      quote = ch;
      current += ch;
      continue;
    }
    if (ch === "<") angleDepth += 1;
    if (ch === ">" && angleDepth > 0) angleDepth -= 1;
    if (ch === "(") parenDepth += 1;
    if (ch === ")" && parenDepth > 0) parenDepth -= 1;
    if (ch === "[") bracketDepth += 1;
    if (ch === "]" && bracketDepth > 0) bracketDepth -= 1;
    if (ch === "{") braceDepth += 1;
    if (ch === "}" && braceDepth > 0) braceDepth -= 1;

    if (
      ch === "," &&
      angleDepth === 0 &&
      parenDepth === 0 &&
      bracketDepth === 0 &&
      braceDepth === 0
    ) {
      parts.push(current.trim());
      current = "";
      continue;
    }
    current += ch;
  }
  if (current.trim().length > 0) {
    parts.push(current.trim());
  }
  return parts;
};

const parseScriptArgs = (root: XmlElementNode, resolveNamedType?: NamedTypeResolver): ScriptParam[] => {
  const raw = getAttr(root, "args", false);
  if (!raw || raw.trim().length === 0) {
    return [];
  }
  const segments = splitByTopLevelComma(raw).filter(Boolean);

  const params: ScriptParam[] = [];
  const names = new Set<string>();

  for (let i = 0; i < segments.length; i += 1) {
    const segment = segments[i];
    const isRef = segment.startsWith("ref:");
    const normalized = isRef ? segment.slice("ref:".length).trim() : segment;
    const separator = normalized.indexOf(":");
    if (separator <= 0 || separator >= normalized.length - 1) {
      throw new ScriptLangError(
        "SCRIPT_ARGS_PARSE_ERROR",
        `Invalid script args segment: "${segment}".`,
        root.location
      );
    }
    const typeSource = normalized.slice(0, separator).trim();
    const name = normalized.slice(separator + 1).trim();
    if (names.has(name)) {
      throw new ScriptLangError(
        "SCRIPT_ARGS_DUPLICATE",
        `Script arg "${name}" is declared more than once.`,
        root.location
      );
    }
    names.add(name);
    params.push({
      name,
      type: parseType(typeSource, root.location, resolveNamedType),
      isRef,
      location: root.location,
    });
  }

  return params;
};

const parseArgs = (raw: string | null): CallArgument[] => {
  if (!raw || raw.trim().length === 0) {
    return [];
  }
  return splitByTopLevelComma(raw)
    .map((part) => part.trim())
    .filter(Boolean)
    .map((part) => {
      const isRef = part.startsWith("ref:");
      const normalized = isRef ? part.slice("ref:".length).trim() : part;
      if (normalized.length === 0) {
        throw new ScriptLangError("CALL_ARGS_PARSE_ERROR", `Invalid call arg segment: "${part}".`);
      }
      return { valueExpr: normalized, isRef };
    });
};

const inlineTextContent = (node: XmlElementNode): string =>
  node.children
    .filter((child): child is Extract<XmlNode, { kind: "text" }> => child.kind === "text")
    .map((x) => x.value)
    .join("\n")
    .trim();

const parseInlineRequired = (node: XmlElementNode): string => {
  if (Object.hasOwn(node.attributes, "value")) {
    throw new ScriptLangError(
      "XML_ATTR_NOT_ALLOWED",
      `Attribute "value" is not allowed on <${node.name}>. Use inline content instead.`,
      node.location
    );
  }
  const content = inlineTextContent(node);
  if (content.length === 0) {
    throw new ScriptLangError(
      "XML_EMPTY_NODE_CONTENT",
      `<${node.name}> requires non-empty inline content.`,
      node.location
    );
  }
  return content;
};

const parseVarDeclaration = (
  node: XmlElementNode,
  resolveNamedType?: NamedTypeResolver
): VarDeclaration => {
  const name = getAttr(node, "name", true) as string;
  const typeSource = getAttr(node, "type", true) as string;
  const initialValueExpr = getAttr(node, "value", false);
  return {
    name,
    type: parseType(typeSource, node.location, resolveNamedType),
    initialValueExpr,
    location: node.location,
  };
};

const compileGroup = (
  groupId: string,
  parentGroupId: string | null,
  container: XmlElementNode,
  builder: GroupBuilder,
  resolveNamedType?: NamedTypeResolver,
  whileDepth = 0,
  allowOptionDirectContinue = false
): void => {
  const nodes: ScriptNode[] = [];
  builder.groups[groupId] = {
    groupId,
    parentGroupId,
    entryNodeId: null,
    nodes,
  };

  for (const child of asElements(container.children)) {
    if (Object.hasOwn(child.attributes, "once") && child.name !== "text") {
      throw new ScriptLangError(
        "XML_ATTR_NOT_ALLOWED",
        'Attribute "once" is only allowed on <text> and <option>.',
        child.location
      );
    }

    if (REMOVED_NODES.has(child.name)) {
      throw new ScriptLangError(
        "XML_REMOVED_NODE",
        `<${child.name}> is removed in ScriptLang V2. Use direct script-body nodes and <script args="..."> + <var .../> instead.`,
        child.location
      );
    }

    let compiled: ScriptNode;

    if (child.name === "var") {
      const varNode: VarNode = {
        id: builder.nextNodeId("var"),
        kind: "var",
        declaration: parseVarDeclaration(child, resolveNamedType),
        location: child.location,
      };
      compiled = varNode;
    } else if (child.name === "text") {
      const textNode: TextNode = {
        id: builder.nextNodeId("text"),
        kind: "text",
        value: parseInlineRequired(child),
        once: parseBooleanAttr(child, "once"),
        location: child.location,
      };
      compiled = textNode;
    } else if (child.name === "code") {
      const codeNode: CodeNode = {
        id: builder.nextNodeId("code"),
        kind: "code",
        code: parseInlineRequired(child),
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
      compileGroup(
        thenGroupId,
        groupId,
        thenContainer,
        builder,
        resolveNamedType,
        whileDepth,
        false
      );
      if (elseNode) {
        compileGroup(elseGroupId, groupId, elseNode, builder, resolveNamedType, whileDepth, false);
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
      compileGroup(bodyGroupId, groupId, child, builder, resolveNamedType, whileDepth + 1, false);
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
      const fallOverOptionPositions: Array<{ index: number; location: SourceSpan }> = [];
      const optionElements = asElements(child.children);
      for (let optionIndex = 0; optionIndex < optionElements.length; optionIndex += 1) {
        const option = optionElements[optionIndex];
        if (option.name !== "option") {
          throw new ScriptLangError(
            "XML_CHOICE_OPTION_INVALID",
            `<choice> only accepts <option>, got <${option.name}>.`,
            option.location
          );
        }
        const whenExpr = getAttr(option, "when", false);
        const once = parseBooleanAttr(option, "once");
        const fallOver = parseBooleanAttr(option, "fall_over");
        if (fallOver) {
          if (whenExpr !== null) {
            throw new ScriptLangError(
              "XML_OPTION_FALL_OVER_WHEN_FORBIDDEN",
              "<option fall_over=\"true\"> cannot declare when.",
              option.location
            );
          }
          fallOverOptionPositions.push({ index: optionIndex, location: option.location });
        }
        const optionGroupId = builder.nextGroupId();
        compileGroup(optionGroupId, groupId, option, builder, resolveNamedType, whileDepth, true);
        options.push({
          id: builder.nextChoiceId(),
          text: getAttr(option, "text", true) as string,
          whenExpr,
          once,
          fallOver,
          groupId: optionGroupId,
          location: option.location,
        });
      }
      if (fallOverOptionPositions.length > 1) {
        throw new ScriptLangError(
          "XML_OPTION_FALL_OVER_DUPLICATE",
          "<choice> can only contain one <option fall_over=\"true\">.",
          fallOverOptionPositions[1].location
        );
      }
      if (fallOverOptionPositions.length === 1 && fallOverOptionPositions[0].index !== optionElements.length - 1) {
        throw new ScriptLangError(
          "XML_OPTION_FALL_OVER_NOT_LAST",
          "<option fall_over=\"true\"> must be the last option in <choice>.",
          fallOverOptionPositions[0].location
        );
      }
      const choiceNode: ChoiceNode = {
        id: builder.nextNodeId("choice"),
        kind: "choice",
        promptText: getRequiredNonEmptyAttr(child, "text"),
        options,
        location: child.location,
      };
      compiled = choiceNode;
    } else if (child.name === "break") {
      if (whileDepth <= 0) {
        throw new ScriptLangError(
          "XML_BREAK_OUTSIDE_WHILE",
          "<break> is only allowed inside <while>.",
          child.location
        );
      }
      const breakNode: BreakNode = {
        id: builder.nextNodeId("break"),
        kind: "break",
        location: child.location,
      };
      compiled = breakNode;
    } else if (child.name === "continue") {
      let target: "while" | "choice" | null = null;
      if (allowOptionDirectContinue) {
        target = "choice";
      } else if (whileDepth > 0) {
        target = "while";
      }
      if (target === null) {
        throw new ScriptLangError(
          "XML_CONTINUE_OUTSIDE_WHILE_OR_OPTION",
          "<continue> is only allowed inside <while> or as a direct child of <option>.",
          child.location
        );
      }
      const continueNode: ContinueNode = {
        id: builder.nextNodeId("continue"),
        kind: "continue",
        target,
        location: child.location,
      };
      compiled = continueNode;
    } else if (child.name === "call") {
      const callNode: CallNode = {
        id: builder.nextNodeId("call"),
        kind: "call",
        targetScript: getAttr(child, "script", true) as string,
        args: parseArgs(getAttr(child, "args", false)),
        location: child.location,
      };
      compiled = callNode;
    } else if (child.name === "return") {
      const targetScript = getAttr(child, "script", false);
      const args = parseArgs(getAttr(child, "args", false));
      if (!targetScript && args.length > 0) {
        throw new ScriptLangError(
          "XML_RETURN_ARGS_WITHOUT_TARGET",
          "<return> with args requires script attribute.",
          child.location
        );
      }
      if (args.some((arg) => arg.isRef)) {
        throw new ScriptLangError(
          "XML_RETURN_REF_UNSUPPORTED",
          "<return> args must be value-only; ref mode is not supported.",
          child.location
        );
      }
      const returnNode: ReturnNode = {
        id: builder.nextNodeId("return"),
        kind: "return",
        targetScript,
        args,
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

const parseTypeDeclarationsFromRoot = (root: XmlElementNode, sourcePath: string): ParsedTypeDecl[] => {
  const collectionName = getAttr(root, "name", true) as string;
  const declarations: ParsedTypeDecl[] = [];

  for (const child of asElements(root.children)) {
    if (child.name !== "type") {
      throw new ScriptLangError(
        "XML_TYPES_NODE_INVALID",
        `<types> only accepts <type>, got <${child.name}>.`,
        child.location
      );
    }
    const typeName = getAttr(child, "name", true) as string;
    const fields: ParsedTypeFieldDecl[] = [];
    const fieldNames = new Set<string>();
    for (const fieldNode of asElements(child.children)) {
      if (fieldNode.name !== "field") {
        throw new ScriptLangError(
          "XML_TYPES_FIELD_INVALID",
          `<type> only accepts <field>, got <${fieldNode.name}>.`,
          fieldNode.location
        );
      }
      const fieldName = getAttr(fieldNode, "name", true) as string;
      if (fieldNames.has(fieldName)) {
        throw new ScriptLangError(
          "TYPE_FIELD_DUPLICATE",
          `Field "${fieldName}" is declared more than once in type "${typeName}".`,
          fieldNode.location
        );
      }
      fieldNames.add(fieldName);
      const fieldTypeRaw = getAttr(fieldNode, "type", true) as string;
      fields.push({
        name: fieldName,
        typeExpr: parseTypeExpr(fieldTypeRaw, fieldNode.location),
        location: fieldNode.location,
      });
    }
    declarations.push({
      name: typeName,
      fields,
      location: child.location,
      sourcePath,
      collectionName,
    });
  }

  return declarations;
};

const resolveTypeDeclarations = (decls: ParsedTypeDecl[]): Record<string, ScriptType> => {
  const byName = new Map<string, ParsedTypeDecl>();
  for (let i = 0; i < decls.length; i += 1) {
    const decl = decls[i];
    if (byName.has(decl.name)) {
      throw new ScriptLangError(
        "TYPE_DECL_DUPLICATE",
        `Type "${decl.name}" is declared more than once.`,
        decl.location
      );
    }
    byName.set(decl.name, decl);
  }

  const resolved = new Map<string, ScriptType>();
  const resolving: string[] = [];

  const resolveByName = (typeName: string, span: SourceSpan): ScriptType => {
    const cached = resolved.get(typeName);
    if (cached) {
      return cached;
    }

    const decl = byName.get(typeName);
    if (!decl) {
      throw new ScriptLangError("TYPE_UNKNOWN", `Unknown type "${typeName}".`, span);
    }

    if (resolving.includes(typeName)) {
      const cycle = [...resolving, typeName].join(" -> ");
      throw new ScriptLangError(
        "TYPE_RECURSIVE",
        `Recursive custom type reference detected: ${cycle}.`,
        decl.location
      );
    }

    resolving.push(typeName);
    const fields: Record<string, ScriptType> = {};
    for (let i = 0; i < decl.fields.length; i += 1) {
      const field = decl.fields[i];
      fields[field.name] = resolveTypeExpr(field.typeExpr, field.location, resolveByName);
    }

    resolving.pop();
    const objectType: ScriptType = {
      kind: "object",
      typeName,
      fields,
    };
    resolved.set(typeName, objectType);
    return objectType;
  };

  for (const [name, decl] of byName.entries()) {
    resolveByName(name, decl.location);
  }

  return Object.fromEntries(resolved.entries());
};

const parseIncludeDirectives = (xmlSource: string): string[] => {
  const includes: string[] = [];
  const lines = xmlSource.split(/\r?\n/);
  for (let i = 0; i < lines.length; i += 1) {
    const line = lines[i].trim();
    if (line.length === 0) {
      continue;
    }
    const match = line.match(INCLUDE_DIRECTIVE);
    if (match) {
      includes.push(match[1].trim());
      continue;
    }
    break;
  }
  return includes;
};

const resolveIncludePath = (currentPath: string, includeSource: string): string => {
  const normalizedInput = includeSource.replace(/\\/g, "/").trim();
  if (normalizedInput.length === 0) {
    throw new ScriptLangError("XML_INCLUDE_INVALID", "Include path cannot be empty.");
  }
  if (path.posix.isAbsolute(normalizedInput)) {
    throw new ScriptLangError(
      "XML_INCLUDE_INVALID",
      `Include path must be relative: "${includeSource}".`
    );
  }
  return path.posix.normalize(path.posix.join(path.posix.dirname(currentPath), normalizedInput));
};

const collectIncludeTargets = (filePath: string, xmlByPath: Record<string, string>): string[] => {
  const source = xmlByPath[filePath];
  if (source === undefined) {
    throw new ScriptLangError("XML_INCLUDE_MISSING", `Included file not found: ${filePath}.`);
  }
  const includes = parseIncludeDirectives(source);
  const targets: string[] = [];
  for (let i = 0; i < includes.length; i += 1) {
    const includeTarget = resolveIncludePath(filePath, includes[i]);
    if (!(includeTarget in xmlByPath)) {
      throw new ScriptLangError(
        "XML_INCLUDE_MISSING",
        `Included file "${includes[i]}" not found from "${filePath}".`
      );
    }
    targets.push(includeTarget);
  }
  return targets;
};

const collectReachablePaths = (xmlByPath: Record<string, string>): string[] => {
  const scriptFiles = Object.keys(xmlByPath)
    .filter((filePath) => filePath.endsWith(".script.xml"))
    .sort();
  if (scriptFiles.length === 0) {
    return [];
  }

  const mainRoots: string[] = [];
  for (let i = 0; i < scriptFiles.length; i += 1) {
    const filePath = scriptFiles[i];
    const source = xmlByPath[filePath];
    if (source === undefined) {
      throw new ScriptLangError("XML_INCLUDE_MISSING", `Included file not found: ${filePath}.`);
    }
    const root = parseXmlDocument(source).root;
    if (root.name !== "script") {
      throw new ScriptLangError(
        "XML_INVALID_ROOT",
        `Expected <script> as root but got <${root.name}>.`,
        root.location
      );
    }
    const scriptName = getAttr(root, "name", true) as string;
    if (scriptName === "main") {
      mainRoots.push(filePath);
    }
  }

  if (mainRoots.length === 0) {
    return [];
  }
  if (mainRoots.length > 1) {
    throw new ScriptLangError(
      "API_DUPLICATE_SCRIPT_NAME",
      'Duplicate script name "main" found across XML inputs.'
    );
  }

  const visited = new Set<string>();
  const stack: string[] = [];

  const visit = (filePath: string): void => {
    if (visited.has(filePath)) {
      return;
    }
    const cycleStart = stack.indexOf(filePath);
    if (cycleStart >= 0) {
      const cycle = [...stack.slice(cycleStart), filePath].join(" -> ");
      throw new ScriptLangError("XML_INCLUDE_CYCLE", `Include cycle detected: ${cycle}.`);
    }

    const source = xmlByPath[filePath];
    if (source === undefined) {
      throw new ScriptLangError("XML_INCLUDE_MISSING", `Included file not found: ${filePath}.`);
    }

    stack.push(filePath);
    const includeTargets = collectIncludeTargets(filePath, xmlByPath);
    for (let i = 0; i < includeTargets.length; i += 1) {
      const includeTarget = includeTargets[i];
      visit(includeTarget);
    }
    stack.pop();
    visited.add(filePath);
  };

  visit(mainRoots[0]);

  return Array.from(visited).sort();
};

const buildIncludeGraphForReachablePaths = (
  reachablePaths: string[],
  xmlByPath: Record<string, string>
): Record<string, string[]> => {
  const reachableSet = new Set(reachablePaths);
  const includeGraph: Record<string, string[]> = {};
  for (let i = 0; i < reachablePaths.length; i += 1) {
    const filePath = reachablePaths[i];
    includeGraph[filePath] = collectIncludeTargets(filePath, xmlByPath).filter((target) =>
      reachableSet.has(target)
    );
  }
  return includeGraph;
};

const collectScriptVisibleTypeNames = (
  scriptPath: string,
  includeGraph: Record<string, string[]>,
  typeNamesByPath: Record<string, string[]>
): Set<string> => {
  const visibleTypeNames = new Set<string>();
  const visited = new Set<string>();
  const stack = [scriptPath];

  while (stack.length > 0) {
    const current = stack.pop() as string;
    if (visited.has(current)) {
      continue;
    }
    visited.add(current);

    const currentTypeNames = typeNamesByPath[current] ?? [];
    for (let i = 0; i < currentTypeNames.length; i += 1) {
      visibleTypeNames.add(currentTypeNames[i]);
    }

    const includes = includeGraph[current];
    for (let i = 0; i < includes.length; i += 1) {
      stack.push(includes[i]);
    }
  }

  return visibleTypeNames;
};

const collectScriptVisibleJsonNames = (
  scriptPath: string,
  includeGraph: Record<string, string[]>,
  jsonSymbolByPath: Record<string, string>
): Set<string> => {
  const visibleJsonNames = new Set<string>();
  const visited = new Set<string>();
  const stack = [scriptPath];

  while (stack.length > 0) {
    const current = stack.pop() as string;
    if (visited.has(current)) {
      continue;
    }
    visited.add(current);

    const jsonSymbol = jsonSymbolByPath[current];
    if (jsonSymbol) {
      visibleJsonNames.add(jsonSymbol);
    }

    const includes = includeGraph[current];
    for (let i = 0; i < includes.length; i += 1) {
      stack.push(includes[i]);
    }
  }

  return visibleJsonNames;
};

const isJsonAssetPath = (filePath: string): boolean => filePath.endsWith(".json");

const parseJsonGlobalSymbol = (filePath: string): string => {
  const symbol = path.posix.basename(filePath, ".json");
  if (!JSON_GLOBAL_NAME_PATTERN.test(symbol)) {
    throw new ScriptLangError(
      "JSON_SYMBOL_INVALID",
      `Invalid JSON global symbol "${symbol}" derived from "${filePath}". JSON basename must be a valid identifier.`
    );
  }
  return symbol;
};

const parseJsonGlobalValue = (source: string, filePath: string): unknown => {
  try {
    return JSON.parse(source);
  } catch (error) {
    const message = error instanceof Error ? error.message : "Unknown JSON parse error.";
    throw new ScriptLangError(
      "JSON_PARSE_ERROR",
      `Failed to parse JSON include "${filePath}": ${message}`
    );
  }
};

export const compileScript = (
  xmlSource: string,
  scriptPath: string,
  options: CompileScriptOptions = {}
): ScriptIR => {
  const document = parseXmlDocument(xmlSource);
  const root = document.root;
  if (root.name !== "script") {
    throw new ScriptLangError(
      "XML_INVALID_ROOT",
      `Expected <script> as root but got <${root.name}>.`,
      root.location
    );
  }

  const scriptName = getAttr(root, "name", true) as string;
  const params = parseScriptArgs(root, options.resolveNamedType);
  const expandedRoot = expandScriptMacros(root, {
    reservedVarNames: params.map((param) => param.name),
  });
  const builder = new GroupBuilder(scriptPath);
  const rootGroupId = builder.nextGroupId();
  compileGroup(rootGroupId, null, expandedRoot, builder, options.resolveNamedType, 0, false);

  return {
    scriptPath,
    scriptName,
    params,
    rootGroupId,
    groups: builder.groups,
    visibleJsonGlobals: options.visibleJsonGlobals ? [...options.visibleJsonGlobals] : undefined,
  };
};

export const compileProjectBundleFromXmlMap = (
  xmlByPath: Record<string, string>
): CompileProjectBundleFromXmlMapResult => {
  const reachablePaths = collectReachablePaths(xmlByPath);
  if (reachablePaths.length === 0) {
    return { scripts: {}, globalJson: {} };
  }

  const parsedRoots: Record<string, XmlElementNode> = {};
  const typeDecls: ParsedTypeDecl[] = [];
  const jsonSymbolByPath: Record<string, string> = {};
  const globalJson: Record<string, unknown> = {};
  const jsonPathBySymbol: Record<string, string> = {};

  for (let i = 0; i < reachablePaths.length; i += 1) {
    const filePath = reachablePaths[i];
    const source = xmlByPath[filePath];
    if (source === undefined) {
      throw new ScriptLangError("XML_INCLUDE_MISSING", `Included file not found: ${filePath}.`);
    }

    if (isJsonAssetPath(filePath)) {
      const symbol = parseJsonGlobalSymbol(filePath);
      if (jsonPathBySymbol[symbol]) {
        throw new ScriptLangError(
          "JSON_SYMBOL_DUPLICATE",
          `Duplicate JSON global symbol "${symbol}" from "${filePath}" and "${jsonPathBySymbol[symbol]}".`
        );
      }
      jsonPathBySymbol[symbol] = filePath;
      jsonSymbolByPath[filePath] = symbol;
      globalJson[symbol] = parseJsonGlobalValue(source, filePath);
      continue;
    }

    const document = parseXmlDocument(source);
    const root = document.root;
    parsedRoots[filePath] = root;

    if (root.name === "types") {
      typeDecls.push(...parseTypeDeclarationsFromRoot(root, filePath));
      continue;
    }
    if (root.name !== "script") {
      throw new ScriptLangError(
        "XML_INVALID_ROOT",
        `Expected <script> or <types> as root but got <${root.name}>.`,
        root.location
      );
    }
  }

  const resolvedTypes = resolveTypeDeclarations(typeDecls);
  const includeGraph = buildIncludeGraphForReachablePaths(reachablePaths, xmlByPath);
  const typeNamesByPath: Record<string, string[]> = {};
  for (let i = 0; i < typeDecls.length; i += 1) {
    const decl = typeDecls[i];
    if (!typeNamesByPath[decl.sourcePath]) {
      typeNamesByPath[decl.sourcePath] = [];
    }
    typeNamesByPath[decl.sourcePath].push(decl.name);
  }

  const compiled: Record<string, ScriptIR> = {};
  for (let i = 0; i < reachablePaths.length; i += 1) {
    const filePath = reachablePaths[i];
    if (isJsonAssetPath(filePath)) {
      continue;
    }
    if (parsedRoots[filePath].name !== "script") {
      continue;
    }
    const visibleTypeNames = collectScriptVisibleTypeNames(filePath, includeGraph, typeNamesByPath);
    const resolveNamedTypeForScript = (name: string, span: SourceSpan): ScriptType => {
      if (!visibleTypeNames.has(name)) {
        throw new ScriptLangError(
          "TYPE_UNKNOWN",
          `Unknown type "${name}" in "${filePath}". Include the corresponding .types.xml file from this script's include closure.`,
          span
        );
      }
      return resolvedTypes[name] as ScriptType;
    };
    const visibleJsonGlobals = Array.from(
      collectScriptVisibleJsonNames(filePath, includeGraph, jsonSymbolByPath)
    ).sort();
    const ir = compileScript(xmlByPath[filePath], filePath, {
      resolveNamedType: resolveNamedTypeForScript,
      visibleJsonGlobals,
    });
    if (compiled[ir.scriptName]) {
      throw new ScriptLangError(
        "API_DUPLICATE_SCRIPT_NAME",
        `Duplicate script name "${ir.scriptName}" found across XML inputs.`
      );
    }
    compiled[ir.scriptName] = ir;
  }

  return {
    scripts: compiled,
    globalJson,
  };
};

export const compileProjectScriptsFromXmlMap = (
  xmlByPath: Record<string, string>
): Record<string, ScriptIR> => {
  return compileProjectBundleFromXmlMap(xmlByPath).scripts;
};
