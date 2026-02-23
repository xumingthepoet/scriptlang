import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import v8 from "node:v8";

import { test, vi } from "vitest";

import {
  PLAYER_COMPILER_VERSION,
  chooseAndContinue,
  resumeScenario,
  startScenario,
} from "../src/cli/core/engine-runner.js";
import {
  getScenarioScriptsRoot,
  listScenarios,
  loadScenarioById,
  loadScenarioByRef,
  loadScenarioByScriptsDir,
  makeExternalScenarioId,
} from "../src/cli/core/scenario-registry.js";
import {
  PLAYER_STATE_SCHEMA,
  createPlayerState,
  loadPlayerState,
  savePlayerState,
} from "../src/cli/core/state-store.js";

test("scenario registry lists and loads scenarios", () => {
  const scenarios = listScenarios();
  assert.equal(scenarios.length, 8);
  assert.equal(scenarios[0].id, "01-text-code");
  assert.ok(scenarios.some((scenario) => scenario.id === "07-battle-duel"));
  assert.ok(scenarios.some((scenario) => scenario.id === "08-json-globals"));

  const loaded = loadScenarioById("04-call-ref-return");
  assert.equal(loaded.entryScript, "main");
  assert.ok(loaded.scriptsXml["main.script.xml"].includes("<call script=\"buff\""));
  assert.ok(loaded.scriptsXml["buff.script.xml"].includes("target = target + amount"));

  const battle = loadScenarioById("07-battle-duel");
  assert.equal(battle.entryScript, "main");
  assert.ok(battle.scriptsXml["main.script.xml"].includes("<call"));
  assert.ok(battle.scriptsXml["battle-loop.script.xml"].includes("<while"));

  const jsonGlobals = loadScenarioById("08-json-globals");
  assert.equal(jsonGlobals.entryScript, "main");
  assert.ok(jsonGlobals.scriptsXml["main.script.xml"].includes("include: game.json"));
  assert.ok(jsonGlobals.scriptsXml["game.json"].includes("\"title\": \"JSON Globals Demo\""));
});

test("scenario registry error paths", () => {
  assert.throws(() => loadScenarioById("missing-id"), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_SCENARIO_NOT_FOUND");
    return true;
  });

  const originalExistsSync = fs.existsSync.bind(fs);
  const existsSpy = vi.spyOn(fs, "existsSync");
  existsSpy.mockImplementation((target) => {
    const asString = String(target);
    if (asString.endsWith(`${path.sep}main.script.xml`)) {
      return false;
    }
    return originalExistsSync(target);
  });
  try {
    assert.throws(() => loadScenarioById("01-text-code"), (error: unknown) => {
      assert.equal((error as { code?: string }).code, "CLI_SCENARIO_FILE_MISSING");
      return true;
    });
  } finally {
    existsSpy.mockRestore();
  }
});

test("scenario root detection failure path", () => {
  const existsSpy = vi.spyOn(fs, "existsSync").mockImplementation(() => false);
  try {
    assert.throws(() => getScenarioScriptsRoot(), (error: unknown) => {
      assert.equal((error as { code?: string }).code, "CLI_PROJECT_ROOT");
      return true;
    });
  } finally {
    existsSpy.mockRestore();
  }
});

test("external scripts-dir loading and ref resolution", () => {
  const externalDir = path.resolve("examples", "scripts", "06-snapshot-flow");
  const loaded = loadScenarioByScriptsDir(externalDir);
  assert.equal(loaded.entryScript, "main");
  assert.equal(loaded.id, makeExternalScenarioId(path.resolve(externalDir)));
  assert.ok(loaded.scriptsXml["main.script.xml"].includes("<choice>"));

  const viaRef = loadScenarioByRef(loaded.id);
  assert.equal(viaRef.id, loaded.id);
  assert.equal(viaRef.entryScript, "main");

  const exampleViaRef = loadScenarioByRef("01-text-code");
  assert.equal(exampleViaRef.id, "01-text-code");
});

test("external scripts-dir includes .types.xml files", () => {
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

  const loaded = loadScenarioByScriptsDir(dir);
  assert.ok(loaded.scriptsXml["main.script.xml"]);
  assert.ok(loaded.scriptsXml["game.types.xml"]);
});

test("external scripts-dir includes .json files", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-json-dir-"));
  fs.writeFileSync(
    path.join(dir, "main.script.xml"),
    `<!-- include: game.json -->
<script name="main"><text>\${game.player.name}</text></script>`
  );
  fs.writeFileSync(path.join(dir, "game.json"), `{"player":{"name":"Hero"}}`);

  const loaded = loadScenarioByScriptsDir(dir);
  assert.ok(loaded.scriptsXml["main.script.xml"]);
  assert.ok(loaded.scriptsXml["game.json"]);
});

test("loadScenarioById auto-includes discovered .types.xml files", () => {
  const scenarioDir = path.join(getScenarioScriptsRoot(), "01-text-code");
  const injectedName = "extra.types.xml";
  const injectedContent =
    `<types name="extra"><type name="X"><field name="hp" type="number"/></type></types>`;
  const injectedPath = path.join(scenarioDir, injectedName);

  try {
    fs.writeFileSync(injectedPath, injectedContent);
    const loaded = loadScenarioById("01-text-code");
    assert.ok(loaded.scriptsXml[injectedName]);
  } finally {
    if (fs.existsSync(injectedPath)) {
      fs.unlinkSync(injectedPath);
    }
  }
});

test("loadScenarioById auto-includes discovered .json files", () => {
  const scenarioDir = path.join(getScenarioScriptsRoot(), "01-text-code");
  const injectedName = "extra.json";
  const injectedContent = `{"bonus":1}`;
  const injectedPath = path.join(scenarioDir, injectedName);

  try {
    fs.writeFileSync(injectedPath, injectedContent);
    const loaded = loadScenarioById("01-text-code");
    assert.ok(loaded.scriptsXml[injectedName]);
  } finally {
    if (fs.existsSync(injectedPath)) {
      fs.unlinkSync(injectedPath);
    }
  }
});

test("external scripts-dir error paths", () => {
  const missingDir = path.join(os.tmpdir(), `scriptlang-missing-${Date.now()}`);
  assert.throws(() => loadScenarioByScriptsDir(missingDir), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_SCRIPTS_DIR_NOT_FOUND");
    return true;
  });

  const filePath = path.join(os.tmpdir(), `scriptlang-file-${Date.now()}.txt`);
  fs.writeFileSync(filePath, "x");
  assert.throws(() => loadScenarioByScriptsDir(filePath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_SCRIPTS_DIR_NOT_FOUND");
    return true;
  });

  const emptyDir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-empty-scripts-"));
  assert.throws(() => loadScenarioByScriptsDir(emptyDir), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_SCRIPTS_DIR_EMPTY");
    return true;
  });

  assert.throws(() => loadScenarioByRef("scripts-dir:"), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });
});

test("engine runner start choose and resume flows", () => {
  const scenario = loadScenarioById("06-snapshot-flow");
  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);

  assert.equal(started.boundary.event, "CHOICES");
  assert.deepEqual(started.boundary.texts, ["before 10"]);
  assert.equal(started.boundary.choices.length, 2);

  const snapshot = started.engine.snapshot();
  const resumed = resumeScenario(scenario, snapshot, PLAYER_COMPILER_VERSION);
  assert.equal(resumed.boundary.event, "CHOICES");

  const ended = chooseAndContinue(resumed.engine, 0);
  assert.equal(ended.event, "END");
  assert.deepEqual(ended.texts, ["after 15"]);
});

test("state store save and load roundtrip", () => {
  const scenario = loadScenarioById("06-snapshot-flow");
  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);
  const snapshot = started.engine.snapshot();

  const state = createPlayerState(scenario.id, PLAYER_COMPILER_VERSION, snapshot);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-state-"));
  const statePath = path.join(dir, "nested", "save.bin");
  savePlayerState(statePath, state);

  const loaded = loadPlayerState(statePath);
  assert.equal(loaded.schemaVersion, PLAYER_STATE_SCHEMA);
  assert.equal(loaded.scenarioId, scenario.id);
  assert.equal(loaded.snapshot.schemaVersion, "snapshot.v1");
});

test("state store validation errors", () => {
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-state-invalid-"));

  const missing = path.join(dir, "missing.bin");
  assert.throws(() => loadPlayerState(missing), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_NOT_FOUND");
    return true;
  });

  const wrongSchemaPath = path.join(dir, "wrong-schema.bin");
  fs.writeFileSync(
    wrongSchemaPath,
    v8.serialize({
      schemaVersion: "old",
      scenarioId: "06-snapshot-flow",
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: { schemaVersion: "snapshot.v1", compilerVersion: PLAYER_COMPILER_VERSION, waitingChoice: true },
    })
  );
  assert.throws(() => loadPlayerState(wrongSchemaPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_SCHEMA");
    return true;
  });

  const badScenarioPath = path.join(dir, "bad-scenario.bin");
  fs.writeFileSync(
    badScenarioPath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId: "",
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: { schemaVersion: "snapshot.v1", compilerVersion: PLAYER_COMPILER_VERSION, waitingChoice: true },
    })
  );
  assert.throws(() => loadPlayerState(badScenarioPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const badCompilerPath = path.join(dir, "bad-compiler.bin");
  fs.writeFileSync(
    badCompilerPath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId: "06-snapshot-flow",
      compilerVersion: "",
      snapshot: { schemaVersion: "snapshot.v1", compilerVersion: PLAYER_COMPILER_VERSION, waitingChoice: true },
    })
  );
  assert.throws(() => loadPlayerState(badCompilerPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const badSnapshotPath = path.join(dir, "bad-snapshot.bin");
  fs.writeFileSync(
    badSnapshotPath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId: "06-snapshot-flow",
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: {},
    })
  );
  assert.throws(() => loadPlayerState(badSnapshotPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const missingRngStatePath = path.join(dir, "missing-rng-state.bin");
  fs.writeFileSync(
    missingRngStatePath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId: "06-snapshot-flow",
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: {
        schemaVersion: "snapshot.v1",
        compilerVersion: PLAYER_COMPILER_VERSION,
        waitingChoice: true,
        pendingChoiceItems: [],
      },
    })
  );
  assert.throws(() => loadPlayerState(missingRngStatePath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const missingPendingItemsPath = path.join(dir, "missing-pending-items.bin");
  fs.writeFileSync(
    missingPendingItemsPath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId: "06-snapshot-flow",
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: {
        schemaVersion: "snapshot.v1",
        compilerVersion: PLAYER_COMPILER_VERSION,
        waitingChoice: true,
        rngState: 1,
      },
    })
  );
  assert.throws(() => loadPlayerState(missingPendingItemsPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const nullSnapshotPath = path.join(dir, "null-snapshot.bin");
  fs.writeFileSync(
    nullSnapshotPath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId: "06-snapshot-flow",
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: null,
    })
  );
  assert.throws(() => loadPlayerState(nullSnapshotPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const nonObjectPath = path.join(dir, "non-object.bin");
  fs.writeFileSync(nonObjectPath, v8.serialize("bad"));
  assert.throws(() => loadPlayerState(nonObjectPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });
});
