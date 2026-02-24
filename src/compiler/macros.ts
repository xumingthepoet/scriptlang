import { ScriptLangError } from "../core/errors.js";
import type { XmlElementNode, XmlNode } from "./xml-types.js";

interface MacroExpansionContext {
  usedVarNames: Set<string>;
  loopCounter: number;
}

type MacroHandler = (node: XmlElementNode, context: MacroExpansionContext) => XmlElementNode[];

const LOOP_TEMP_VAR_PREFIX = "__sl_loop_";

const collectDeclaredVarNames = (node: XmlElementNode, names: Set<string>): void => {
  if (node.name === "var") {
    const name = node.attributes.name;
    if (name && name.length > 0) {
      names.add(name);
    }
  }
  for (const child of node.children) {
    if (child.kind !== "element") {
      continue;
    }
    collectDeclaredVarNames(child, names);
  }
};

const nextLoopTempVarName = (context: MacroExpansionContext): string => {
  while (true) {
    const candidate = `${LOOP_TEMP_VAR_PREFIX}${context.loopCounter}_remaining`;
    context.loopCounter += 1;
    if (context.usedVarNames.has(candidate)) {
      continue;
    }
    context.usedVarNames.add(candidate);
    return candidate;
  }
};

const parseLoopTimesExpr = (node: XmlElementNode): string => {
  const raw = node.attributes.times;
  if (raw === undefined || raw === "") {
    throw new ScriptLangError("XML_MISSING_ATTR", 'Missing required attribute "times" on <loop>.', node.location);
  }
  if (raw.trim().length === 0) {
    throw new ScriptLangError("XML_EMPTY_ATTR", 'Attribute "times" on <loop> cannot be empty.', node.location);
  }
  const trimmed = raw.trim();
  if (trimmed.startsWith("${") && trimmed.endsWith("}")) {
    throw new ScriptLangError(
      "XML_LOOP_TIMES_TEMPLATE_UNSUPPORTED",
      'Attribute "times" on <loop> must use expression syntax without ${...} wrapper.',
      node.location
    );
  }
  return raw;
};

const expandChildren = (nodes: XmlNode[], context: MacroExpansionContext): XmlNode[] => {
  const expanded: XmlNode[] = [];
  for (const child of nodes) {
    if (child.kind !== "element") {
      expanded.push(child);
      continue;
    }
    expanded.push(...expandElementWithMacros(child, context));
  }
  return expanded;
};

const expandLoopMacro: MacroHandler = (node: XmlElementNode, context: MacroExpansionContext): XmlElementNode[] => {
  const timesExpr = parseLoopTimesExpr(node);
  const tempVarName = nextLoopTempVarName(context);
  const bodyChildren = expandChildren(node.children, context);

  const decrementCode: XmlElementNode = {
    kind: "element",
    name: "code",
    attributes: {},
    children: [
      {
        kind: "text",
        value: `${tempVarName} = ${tempVarName} - 1;`,
        location: node.location,
      },
    ],
    location: node.location,
  };

  const loopVar: XmlElementNode = {
    kind: "element",
    name: "var",
    attributes: {
      name: tempVarName,
      type: "number",
      value: timesExpr,
    },
    children: [],
    location: node.location,
  };

  const loopWhile: XmlElementNode = {
    kind: "element",
    name: "while",
    attributes: {
      when: `${tempVarName} > 0`,
    },
    children: [decrementCode, ...bodyChildren],
    location: node.location,
  };

  return [loopVar, loopWhile];
};

const macroHandlers: Record<string, MacroHandler> = {
  loop: expandLoopMacro,
};

const expandElementWithMacros = (node: XmlElementNode, context: MacroExpansionContext): XmlElementNode[] => {
  const handler = macroHandlers[node.name];
  if (!handler) {
    return [
      {
        ...node,
        children: expandChildren(node.children, context),
      },
    ];
  }
  return handler(node, context);
};

export const expandScriptMacros = (
  root: XmlElementNode,
  options: { reservedVarNames: string[] }
): XmlElementNode => {
  const usedVarNames = new Set(options.reservedVarNames);
  collectDeclaredVarNames(root, usedVarNames);

  return {
    ...root,
    children: expandChildren(root.children, {
      usedVarNames,
      loopCounter: 0,
    }),
  };
};
