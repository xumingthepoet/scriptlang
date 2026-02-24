import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import v8 from "node:v8";

import { test } from "vitest";

import { PLAYER_COMPILER_VERSION, startScenario } from "../../../../src/cli/core/engine-runner.js";
import { loadSourceByScriptsDir, makeScriptsDirScenarioId } from "../../../../src/cli/core/source-loader.js";
import {
  PLAYER_STATE_SCHEMA,
  createPlayerState,
  loadPlayerState,
  savePlayerState,
} from "../../../../src/cli/core/state-store.js";

const scriptsDir = (id: string): string => path.resolve("examples", "scripts", id);
const makeValidSnapshot = () => ({
  schemaVersion: "snapshot.v2" as const,
  compilerVersion: PLAYER_COMPILER_VERSION,
  rngState: 1,
  pendingBoundary: {
    kind: "choice" as const,
    nodeId: "node-1",
    items: [],
    promptText: null,
  },
});

test("state store save and load roundtrip", () => {
  const scenario = loadSourceByScriptsDir(scriptsDir("06-snapshot-flow"));
  const started = startScenario(scenario, PLAYER_COMPILER_VERSION);
  const snapshot = started.engine.snapshot();

  const state = createPlayerState(scenario.id, PLAYER_COMPILER_VERSION, snapshot);
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-state-"));
  const statePath = path.join(dir, "nested", "save.bin");
  savePlayerState(statePath, state);

  const loaded = loadPlayerState(statePath);
  assert.equal(loaded.schemaVersion, PLAYER_STATE_SCHEMA);
  assert.equal(loaded.scenarioId, scenario.id);
  assert.equal(loaded.snapshot.schemaVersion, "snapshot.v2");
});

test("state store validation errors", () => {
  const scenarioId = makeScriptsDirScenarioId(path.resolve(scriptsDir("06-snapshot-flow")));
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
      scenarioId,
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: makeValidSnapshot(),
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
      snapshot: makeValidSnapshot(),
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
      scenarioId,
      compilerVersion: "",
      snapshot: makeValidSnapshot(),
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
      scenarioId,
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: {},
    })
  );
  assert.throws(() => loadPlayerState(badSnapshotPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const invalidPayloadPath = path.join(dir, "invalid-payload.bin");
  fs.writeFileSync(invalidPayloadPath, v8.serialize("not-an-object"));
  assert.throws(() => loadPlayerState(invalidPayloadPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const nullSnapshotPath = path.join(dir, "null-snapshot.bin");
  fs.writeFileSync(
    nullSnapshotPath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId,
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: null,
    })
  );
  assert.throws(() => loadPlayerState(nullSnapshotPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const missingRngStatePath = path.join(dir, "missing-rng-state.bin");
  fs.writeFileSync(
    missingRngStatePath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId,
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: {
        schemaVersion: "snapshot.v2",
        compilerVersion: PLAYER_COMPILER_VERSION,
        pendingBoundary: {
          kind: "choice",
          nodeId: "node-1",
          items: [],
          promptText: null,
        },
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
      scenarioId,
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: {
        schemaVersion: "snapshot.v2",
        compilerVersion: PLAYER_COMPILER_VERSION,
        rngState: 1,
        pendingBoundary: {
          kind: "choice",
          nodeId: "node-1",
          promptText: null,
        },
      },
    })
  );
  assert.throws(() => loadPlayerState(missingPendingItemsPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const badPromptTypePath = path.join(dir, "bad-prompt-type.bin");
  fs.writeFileSync(
    badPromptTypePath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId,
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: {
        schemaVersion: "snapshot.v2",
        compilerVersion: PLAYER_COMPILER_VERSION,
        rngState: 1,
        pendingBoundary: {
          kind: "choice",
          nodeId: "node-1",
          items: [],
          promptText: 1,
        },
      },
    })
  );
  assert.throws(() => loadPlayerState(badPromptTypePath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const badPendingItemsPath = path.join(dir, "bad-pending-items.bin");
  fs.writeFileSync(
    badPendingItemsPath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId,
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: {
        schemaVersion: "snapshot.v2",
        compilerVersion: PLAYER_COMPILER_VERSION,
        rngState: 1,
        pendingBoundary: {
          kind: "choice",
          nodeId: "node-1",
          items: [{ index: 0, id: 1, text: "x" }],
          promptText: null,
        },
      },
    })
  );
  assert.throws(() => loadPlayerState(badPendingItemsPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const inputPendingPath = path.join(dir, "input-pending.bin");
  fs.writeFileSync(
    inputPendingPath,
    v8.serialize({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId,
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot: {
        schemaVersion: "snapshot.v2",
        compilerVersion: PLAYER_COMPILER_VERSION,
        rngState: 1,
        pendingBoundary: {
          kind: "input",
          nodeId: "node-2",
          targetVar: "name",
          promptText: "Name",
          defaultText: "Traveler",
        },
      },
    })
  );
  const loadedInputPending = loadPlayerState(inputPendingPath);
  assert.equal(loadedInputPending.snapshot.pendingBoundary.kind, "input");
});
