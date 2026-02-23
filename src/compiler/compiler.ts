import path from "node:path";

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
  ScriptParam,
  ScriptType,
  SourceSpan,
  TextNode,
  VarDeclaration,
  VarNode,
  WhileNode,
} from "../core/types.js";
import { parseXmlDocument } from "./xml.js";
import type { XmlElementNode, XmlNode } from "./xml-types.js";

const REMOVED_NODES = new Set(["vars", "step", "set", "push", "remove"]);
const INCLUDE_DIRECTIVE = /^<!--\s*include:\s*(.+?)\s*-->$/;
const CUSTOM_TYPE_NAME_PATTERN = /^[A-Za-z_][A-Za-z0-9_]*$/;

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
    const isRef = segment.endsWith(":ref");
    const normalized = isRef ? segment.slice(0, -4).trim() : segment;
    const separator = normalized.indexOf(":");
    if (separator <= 0 || separator >= normalized.length - 1) {
      throw new ScriptLangError(
        "SCRIPT_ARGS_PARSE_ERROR",
        `Invalid script args segment: "${segment}".`,
        root.location
      );
    }
    const name = normalized.slice(0, separator).trim();
    const typeSource = normalized.slice(separator + 1).trim();
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
  resolveNamedType?: NamedTypeResolver
): void => {
  const nodes: ScriptNode[] = [];
  builder.groups[groupId] = {
    groupId,
    parentGroupId,
    entryNodeId: null,
    nodes,
  };

  for (const child of asElements(container.children)) {
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
      compileGroup(thenGroupId, groupId, thenContainer, builder, resolveNamedType);
      if (elseNode) {
        compileGroup(elseGroupId, groupId, elseNode, builder, resolveNamedType);
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
      compileGroup(bodyGroupId, groupId, child, builder, resolveNamedType);
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
        compileGroup(optionGroupId, groupId, option, builder, resolveNamedType);
        options.push({
          id: builder.nextChoiceId(),
          text: getAttr(option, "text", true) as string,
          whenExpr: getAttr(option, "when", false),
          groupId: optionGroupId,
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
      const callNode: CallNode = {
        id: builder.nextNodeId("call"),
        kind: "call",
        targetScript: getAttr(child, "script", true) as string,
        args: parseArgs(getAttr(child, "args", false)),
        location: child.location,
      };
      compiled = callNode;
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
    const includes = parseIncludeDirectives(source);
    for (let i = 0; i < includes.length; i += 1) {
      const includeTarget = resolveIncludePath(filePath, includes[i]);
      if (!(includeTarget in xmlByPath)) {
        throw new ScriptLangError(
          "XML_INCLUDE_MISSING",
          `Included file "${includes[i]}" not found from "${filePath}".`
        );
      }
      visit(includeTarget);
    }
    stack.pop();
    visited.add(filePath);
  };

  visit(mainRoots[0]);

  return Array.from(visited).sort();
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
  const builder = new GroupBuilder(scriptPath);
  const rootGroupId = builder.nextGroupId();
  compileGroup(rootGroupId, null, root, builder, options.resolveNamedType);

  return {
    scriptPath,
    scriptName,
    params,
    rootGroupId,
    groups: builder.groups,
  };
};

export const compileProjectScriptsFromXmlMap = (
  xmlByPath: Record<string, string>
): Record<string, ScriptIR> => {
  const reachablePaths = collectReachablePaths(xmlByPath);
  if (reachablePaths.length === 0) {
    return {};
  }

  const parsedRoots: Record<string, XmlElementNode> = {};
  const typeDecls: ParsedTypeDecl[] = [];

  for (let i = 0; i < reachablePaths.length; i += 1) {
    const filePath = reachablePaths[i];
    const source = xmlByPath[filePath];
    if (source === undefined) {
      throw new ScriptLangError("XML_INCLUDE_MISSING", `Included file not found: ${filePath}.`);
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
  const resolveNamedType = (name: string, span: SourceSpan): ScriptType => {
    const resolved = resolvedTypes[name];
    if (!resolved) {
      throw new ScriptLangError("TYPE_UNKNOWN", `Unknown type "${name}".`, span);
    }
    return resolved;
  };

  const compiled: Record<string, ScriptIR> = {};
  for (let i = 0; i < reachablePaths.length; i += 1) {
    const filePath = reachablePaths[i];
    if (parsedRoots[filePath].name !== "script") {
      continue;
    }
    const ir = compileScript(xmlByPath[filePath], filePath, {
      resolveNamedType,
    });
    if (compiled[ir.scriptName]) {
      throw new ScriptLangError(
        "API_DUPLICATE_SCRIPT_NAME",
        `Duplicate script name "${ir.scriptName}" found across XML inputs.`
      );
    }
    compiled[ir.scriptName] = ir;
  }

  return compiled;
};
