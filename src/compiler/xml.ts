import { SaxesParser } from "saxes";

import { ScriptLangError } from "../core/errors.js";
import type { XmlDocument, XmlElementNode, XmlTextNode } from "./xml-types.js";

const normalizeLoc = (line: number, column: number) => {
  return {
    line: Math.max(1, line),
    column: Math.max(1, column),
  };
};

export const parseXmlDocument = (source: string): XmlDocument => {
  if (source.trim().length === 0) {
    throw new ScriptLangError("XML_EMPTY", "XML document has no root element.");
  }
  const parser = new SaxesParser({ xmlns: false });
  const stack: XmlElementNode[] = [];
  let root: XmlElementNode | null = null;

  parser.on("opentag", (tag) => {
    const start = normalizeLoc(parser.line, parser.column);
    const node: XmlElementNode = {
      kind: "element",
      name: tag.name,
      attributes: Object.fromEntries(
        Object.entries(tag.attributes).map(([k, v]) => [k, String(v)])
      ),
      children: [],
      location: {
        start,
        end: start,
      },
    };
    stack.push(node);
  });

  parser.on("text", (value) => {
    if (stack.length === 0) {
      return;
    }
    if (value.trim().length === 0) {
      return;
    }
    const end = normalizeLoc(parser.line, parser.column);
    const textNode: XmlTextNode = {
      kind: "text",
      value,
      location: {
        start: end,
        end,
      },
    };
    stack[stack.length - 1].children.push(textNode);
  });

  parser.on("closetag", () => {
    const node = stack.pop() as XmlElementNode;
    node.location.end = normalizeLoc(parser.line, parser.column);
    if (stack.length === 0) {
      root = node;
      return;
    }
    stack[stack.length - 1].children.push(node);
  });

  try {
    parser.write(source).close();
  } catch (error) {
    throw new ScriptLangError("XML_PARSE_ERROR", String(error));
  }

  return { root: root! };
};
