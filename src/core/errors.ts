import type { SourceSpan } from "./types.js";

export class ScriptLangError extends Error {
  readonly code: string;
  readonly span?: SourceSpan;

  constructor(code: string, message: string, span?: SourceSpan) {
    super(message);
    this.name = "ScriptLangError";
    this.code = code;
    this.span = span;
  }
}
