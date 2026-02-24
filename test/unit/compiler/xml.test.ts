import assert from "node:assert/strict";
import { test } from "vitest";

import { ScriptLangError } from "../../../src/core/errors.js";
import { parseXmlDocument } from "../../../src/compiler/xml.js";

const expectCode = (fn: () => unknown, code: string): void => {
  assert.throws(fn, (error: unknown) => {
    assert.ok(error instanceof ScriptLangError);
    assert.equal(error.code, code);
    return true;
  });
};

test("parseXmlDocument parses simple script root", () => {
  const doc = parseXmlDocument(`<script name="main"><text>x</text></script>`);
  assert.equal(doc.root.name, "script");
  assert.equal(doc.root.attributes.name, "main");
});

test("parseXmlDocument throws parse and empty errors", () => {
  expectCode(() => parseXmlDocument("<script"), "XML_PARSE_ERROR");
  expectCode(() => parseXmlDocument(""), "XML_EMPTY");
  expectCode(() => parseXmlDocument("<!-- only-comment -->"), "XML_PARSE_ERROR");
});
