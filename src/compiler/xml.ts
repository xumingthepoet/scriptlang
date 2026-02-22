import { SaxesParser } from "saxes";

import type { SourceSpan } from "../core/types";
import { ScriptLangError } from "../core/errors";

export interface XmlTextNode {
  kind: "text";
  value: string;
  location: SourceSpan;
}

export interface XmlElementNode {
  kind: "element";
  name: string;
  attributes: Record<string, string>;
  children: XmlNode[];
  location: SourceSpan;
}

export type XmlNode = XmlElementNode | XmlTextNode;

export interface XmlDocument {
  root: XmlElementNode;
}

interface MutableElement {
  kind: "element";
  name: string;
  attributes: Record<string, string>;
  children: XmlNode[];
  location: SourceSpan;
}

const normalizeLoc = (line: number, column: number) => {
  return {
    line: Math.max(1, line),
    column: Math.max(1, column),
  };
};

export const parseXmlDocument = (source: string): XmlDocument => {
  const parser = new SaxesParser({ xmlns: false });
  const stack: MutableElement[] = [];
  let root: MutableElement | null = null;
  let parseErrorMessage: string | null = null;

  parser.on("error", (error) => {
    parseErrorMessage = String(error);
  });

  parser.on("opentag", (tag) => {
    const start = normalizeLoc(parser.line, parser.column);
    const node: MutableElement = {
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
    const node = stack.pop();
    if (!node) {
      return;
    }
    node.location.end = normalizeLoc(parser.line, parser.column);
    if (stack.length === 0) {
      root = node;
      return;
    }
    stack[stack.length - 1].children.push(node);
  });

  parser.write(source).close();

  if (parseErrorMessage) {
    throw new ScriptLangError("XML_PARSE_ERROR", parseErrorMessage);
  }

  if (!root) {
    throw new ScriptLangError("XML_EMPTY", "XML document has no root element.");
  }

  return { root };
};
