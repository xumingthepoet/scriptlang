import type { SourceSpan } from "../core/types.js";

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
