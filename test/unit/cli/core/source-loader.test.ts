import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { test, vi } from "vitest";

import {
  getExamplesScriptsRoot,
  loadSourceByRef,
  loadSourceByScriptsDir,
  makeScriptsDirScenarioId,
} from "../../../../src/cli/core/source-loader.js";

const scriptsDir = (id: string): string => path.resolve("examples", "scripts", id);

test("source loader reads script directories including bundled examples", () => {
  const loaded = loadSourceByScriptsDir(scriptsDir("04-call-ref-return"));
  assert.equal(loaded.entryScript, "main");
  assert.ok(loaded.scriptsXml["main.script.xml"].includes('<call script="buff"'));
  assert.ok(loaded.scriptsXml["buff.script.xml"].includes("target = target + amount"));

  const battle = loadSourceByScriptsDir(scriptsDir("07-battle-duel"));
  assert.equal(battle.entryScript, "main");
  assert.ok(battle.scriptsXml["main.script.xml"].includes("<call"));
  assert.ok(battle.scriptsXml["battle-loop.script.xml"].includes("<while"));

  const jsonGlobals = loadSourceByScriptsDir(scriptsDir("08-json-globals"));
  assert.equal(jsonGlobals.entryScript, "main");
  assert.ok(jsonGlobals.scriptsXml["main.script.xml"].includes("include: game.json"));
  assert.ok(jsonGlobals.scriptsXml["game.json"].includes('"title": "JSON Globals Demo"'));

  const randomBuiltin = loadSourceByScriptsDir(scriptsDir("09-random"));
  assert.equal(randomBuiltin.entryScript, "main");
  assert.ok(randomBuiltin.scriptsXml["main.script.xml"].includes("random(100)"));
});

test("source root detection failure path", () => {
  const existsSpy = vi.spyOn(fs, "existsSync").mockImplementation(() => false);
  try {
    assert.throws(() => getExamplesScriptsRoot(), (error: unknown) => {
      assert.equal((error as { code?: string }).code, "CLI_PROJECT_ROOT");
      return true;
    });
  } finally {
    existsSpy.mockRestore();
  }
});

test("source root resolves from project layout", () => {
  const root = getExamplesScriptsRoot();
  assert.equal(path.basename(root), "scripts");
  assert.equal(path.basename(path.dirname(root)), "examples");
});

test("scripts-dir loading and ref resolution", () => {
  const externalDir = scriptsDir("06-snapshot-flow");
  const loaded = loadSourceByScriptsDir(externalDir);
  assert.equal(loaded.entryScript, "main");
  assert.equal(loaded.id, makeScriptsDirScenarioId(path.resolve(externalDir)));
  assert.ok(loaded.scriptsXml["main.script.xml"].includes('<choice text="Choose">'));

  const viaRef = loadSourceByRef(loaded.id);
  assert.equal(viaRef.id, loaded.id);
  assert.equal(viaRef.entryScript, "main");

  assert.throws(() => loadSourceByRef("06-snapshot-flow"), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });
});

test("scripts-dir includes .types.xml files", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-types-dir-"));
  fs.writeFileSync(
    path.join(dir, "main.script.xml"),
    `<!-- include: game.types.xml -->
<script name="main"><text>ok</text></script>`
  );
  fs.writeFileSync(
    path.join(dir, "game.types.xml"),
    `<types name="game"><type name="Actor"><field name="hp" type="number"/></type></types>`
  );

  const loaded = loadSourceByScriptsDir(dir);
  assert.ok(loaded.scriptsXml["main.script.xml"]);
  assert.ok(loaded.scriptsXml["game.types.xml"]);
});

test("scripts-dir includes .json files", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-json-dir-"));
  fs.writeFileSync(
    path.join(dir, "main.script.xml"),
    `<!-- include: game.json -->
<script name="main"><text>\${game.player.name}</text></script>`
  );
  fs.writeFileSync(path.join(dir, "game.json"), `{"player":{"name":"Hero"}}`);

  const loaded = loadSourceByScriptsDir(dir);
  assert.ok(loaded.scriptsXml["main.script.xml"]);
  assert.ok(loaded.scriptsXml["game.json"]);
});

test("scripts-dir error paths", () => {
  const missingDir = path.join(os.tmpdir(), `scriptlang-missing-${Date.now()}`);
  assert.throws(() => loadSourceByScriptsDir(missingDir), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_SCRIPTS_DIR_NOT_FOUND");
    return true;
  });

  const filePath = path.join(os.tmpdir(), `scriptlang-file-${Date.now()}.txt`);
  fs.writeFileSync(filePath, "x");
  assert.throws(() => loadSourceByScriptsDir(filePath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_SCRIPTS_DIR_NOT_FOUND");
    return true;
  });

  const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-empty-scripts-"));
  assert.throws(() => loadSourceByScriptsDir(emptyDir), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_SCRIPTS_DIR_EMPTY");
    return true;
  });

  assert.throws(() => loadSourceByRef("scripts-dir:"), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });
});
