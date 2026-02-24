import assert from "node:assert/strict";
import fs from "node:fs";
import os from "node:os";
import path from "node:path";

import { test } from "vitest";

import { createEngineFromXml, resumeEngineFromXml } from "../../../../src/api.js";
import { PLAYER_COMPILER_VERSION, startScenario } from "../../../../src/cli/core/engine-runner.js";
import { loadSourceByScriptsDir, makeScriptsDirScenarioId } from "../../../../src/cli/core/source-loader.js";
import {
  PLAYER_STATE_SCHEMA,
  createPlayerState,
  loadPlayerState,
  savePlayerState,
} from "../../../../src/cli/core/state-store.js";

const scriptsDir = (id: string): string => path.resolve("examples", "scripts", id);
const PORTABLE_TYPE_KEY = "__scriptlang_portable_type__";
const writeJson = (statePath: string, value: unknown): void => {
  fs.writeFileSync(statePath, JSON.stringify(value), "utf8");
};

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
  const statePath = path.join(dir, "nested", "save.json");
  savePlayerState(statePath, state);

  const persistedText = fs.readFileSync(statePath, "utf8");
  assert.equal(persistedText.startsWith("{"), true);

  const loaded = loadPlayerState(statePath);
  assert.equal(loaded.schemaVersion, PLAYER_STATE_SCHEMA);
  assert.equal(loaded.scenarioId, scenario.id);
  assert.equal(loaded.snapshot.schemaVersion, "snapshot.v2");
});

test("state store preserves map and non-finite number values through portable json", () => {
  const scriptsXml = {
    "main.script.xml": `
<script name="main">
  <var name="scores" type="Map&lt;string,number&gt;" value="new Map([['nan', NaN], ['inf', Infinity], ['ninf', -Infinity]])"/>
  <choice text="Pick">
    <option text="Show"><text>\${scores.get('nan')}|\${scores.get('inf')}|\${scores.get('ninf')}</text></option>
  </choice>
</script>
`,
  };
  const scenarioId = makeScriptsDirScenarioId(path.resolve("/tmp/portable-state"));
  const engine = createEngineFromXml({
    scriptsXml,
    compilerVersion: PLAYER_COMPILER_VERSION,
  });
  const first = engine.next();
  assert.equal(first.kind, "choices");

  const state = createPlayerState(scenarioId, PLAYER_COMPILER_VERSION, engine.snapshot());
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-state-map-"));
  const statePath = path.join(dir, "portable.json");
  savePlayerState(statePath, state);
  const loaded = loadPlayerState(statePath);
  const resumed = resumeEngineFromXml({
    scriptsXml,
    snapshot: loaded.snapshot,
    compilerVersion: PLAYER_COMPILER_VERSION,
  });
  resumed.choose(0);
  const afterChoose = resumed.next();
  assert.equal(afterChoose.kind, "text");
  assert.equal(afterChoose.text, "NaN|Infinity|-Infinity");
});

test("state store portable codec edge paths", () => {
  const scenarioId = makeScriptsDirScenarioId(path.resolve("/tmp/portable-edge"));
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-state-portable-edge-"));
  const makeState = (snapshot: unknown): ReturnType<typeof createPlayerState> =>
    ({
      schemaVersion: PLAYER_STATE_SCHEMA,
      scenarioId,
      compilerVersion: PLAYER_COMPILER_VERSION,
      snapshot,
    }) as unknown as ReturnType<typeof createPlayerState>;

  const undefinedRoundtripPath = path.join(dir, "undefined-roundtrip.json");
  savePlayerState(
    undefinedRoundtripPath,
    ({
      ...makeState(makeValidSnapshot()),
      meta: undefined,
    }) as unknown as ReturnType<typeof createPlayerState>
  );
  const undefinedRoundtripLoaded = loadPlayerState(undefinedRoundtripPath) as {
    meta?: unknown;
  };
  assert.equal(undefinedRoundtripLoaded.meta, undefined);

  const unsupportedTypePath = path.join(dir, "unsupported-type.json");
  assert.throws(
    () => savePlayerState(unsupportedTypePath, makeState({ ...makeValidSnapshot(), extra: 1n })),
    (error: unknown) => {
      assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
      return true;
    }
  );

  const circularArray: unknown[] = [];
  circularArray.push(circularArray);
  const circularArrayPath = path.join(dir, "circular-array.json");
  assert.throws(
    () => savePlayerState(circularArrayPath, makeState({ ...makeValidSnapshot(), extra: circularArray })),
    (error: unknown) => {
      assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
      return true;
    }
  );

  const circularMap = new Map<string, unknown>();
  circularMap.set("self", circularMap);
  const circularMapPath = path.join(dir, "circular-map.json");
  assert.throws(
    () => savePlayerState(circularMapPath, makeState({ ...makeValidSnapshot(), extra: circularMap })),
    (error: unknown) => {
      assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
      return true;
    }
  );

  const badMapKeyPath = path.join(dir, "bad-map-key.json");
  assert.throws(
    () =>
      savePlayerState(
        badMapKeyPath,
        makeState({ ...makeValidSnapshot(), extra: new Map<unknown, unknown>([[1, "x"]]) })
      ),
    (error: unknown) => {
      assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
      return true;
    }
  );

  const circularObject: Record<string, unknown> = {};
  circularObject.self = circularObject;
  const circularObjectPath = path.join(dir, "circular-object.json");
  assert.throws(
    () => savePlayerState(circularObjectPath, makeState({ ...makeValidSnapshot(), extra: circularObject })),
    (error: unknown) => {
      assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
      return true;
    }
  );

  const invalidUndefinedPayloadPath = path.join(dir, "invalid-undefined-payload.json");
  writeJson(invalidUndefinedPayloadPath, {
    [PORTABLE_TYPE_KEY]: "undefined",
    extra: true,
  });
  assert.throws(() => loadPlayerState(invalidUndefinedPayloadPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const invalidNumberPayloadTypePath = path.join(dir, "invalid-number-payload-type.json");
  writeJson(invalidNumberPayloadTypePath, {
    [PORTABLE_TYPE_KEY]: "number",
    value: 1,
  });
  assert.throws(() => loadPlayerState(invalidNumberPayloadTypePath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const invalidNumberPayloadValuePath = path.join(dir, "invalid-number-payload-value.json");
  writeJson(invalidNumberPayloadValuePath, {
    [PORTABLE_TYPE_KEY]: "number",
    value: "INVALID",
  });
  assert.throws(() => loadPlayerState(invalidNumberPayloadValuePath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const invalidMapPayloadShapePath = path.join(dir, "invalid-map-payload-shape.json");
  writeJson(invalidMapPayloadShapePath, {
    [PORTABLE_TYPE_KEY]: "map",
    entries: {},
  });
  assert.throws(() => loadPlayerState(invalidMapPayloadShapePath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const invalidMapEntryPayloadPath = path.join(dir, "invalid-map-entry-payload.json");
  writeJson(invalidMapEntryPayloadPath, {
    [PORTABLE_TYPE_KEY]: "map",
    entries: [[1, "x"]],
  });
  assert.throws(() => loadPlayerState(invalidMapEntryPayloadPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const unknownPortableTagPath = path.join(dir, "unknown-portable-tag.json");
  writeJson(unknownPortableTagPath, {
    [PORTABLE_TYPE_KEY]: "mystery",
  });
  assert.throws(() => loadPlayerState(unknownPortableTagPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });
});

test("state store validation errors", () => {
  const scenarioId = makeScriptsDirScenarioId(path.resolve(scriptsDir("06-snapshot-flow")));
  const dir = fs.mkdtempSync(path.join(os.tmpdir(), "scriptlang-state-invalid-"));

  const missing = path.join(dir, "missing.json");
  assert.throws(() => loadPlayerState(missing), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_NOT_FOUND");
    return true;
  });

  const invalidJsonPath = path.join(dir, "invalid-json.json");
  fs.writeFileSync(invalidJsonPath, "{invalid-json", "utf8");
  assert.throws(() => loadPlayerState(invalidJsonPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const oldBinaryPath = path.join(dir, "legacy.bin");
  fs.writeFileSync(oldBinaryPath, Buffer.from([0xff, 0x00, 0x01]));
  assert.throws(() => loadPlayerState(oldBinaryPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const wrongSchemaPath = path.join(dir, "wrong-schema.json");
  writeJson(wrongSchemaPath, {
    schemaVersion: "old",
    scenarioId,
    compilerVersion: PLAYER_COMPILER_VERSION,
    snapshot: makeValidSnapshot(),
  });
  assert.throws(() => loadPlayerState(wrongSchemaPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_SCHEMA");
    return true;
  });

  const badScenarioPath = path.join(dir, "bad-scenario.json");
  writeJson(badScenarioPath, {
    schemaVersion: PLAYER_STATE_SCHEMA,
    scenarioId: "",
    compilerVersion: PLAYER_COMPILER_VERSION,
    snapshot: makeValidSnapshot(),
  });
  assert.throws(() => loadPlayerState(badScenarioPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const badCompilerPath = path.join(dir, "bad-compiler.json");
  writeJson(badCompilerPath, {
    schemaVersion: PLAYER_STATE_SCHEMA,
    scenarioId,
    compilerVersion: "",
    snapshot: makeValidSnapshot(),
  });
  assert.throws(() => loadPlayerState(badCompilerPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const badSnapshotPath = path.join(dir, "bad-snapshot.json");
  writeJson(badSnapshotPath, {
    schemaVersion: PLAYER_STATE_SCHEMA,
    scenarioId,
    compilerVersion: PLAYER_COMPILER_VERSION,
    snapshot: {},
  });
  assert.throws(() => loadPlayerState(badSnapshotPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const invalidPayloadPath = path.join(dir, "invalid-payload.json");
  writeJson(invalidPayloadPath, "not-an-object");
  assert.throws(() => loadPlayerState(invalidPayloadPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const nullSnapshotPath = path.join(dir, "null-snapshot.json");
  writeJson(nullSnapshotPath, {
    schemaVersion: PLAYER_STATE_SCHEMA,
    scenarioId,
    compilerVersion: PLAYER_COMPILER_VERSION,
    snapshot: null,
  });
  assert.throws(() => loadPlayerState(nullSnapshotPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const missingRngStatePath = path.join(dir, "missing-rng-state.json");
  writeJson(missingRngStatePath, {
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
  });
  assert.throws(() => loadPlayerState(missingRngStatePath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const missingPendingItemsPath = path.join(dir, "missing-pending-items.json");
  writeJson(missingPendingItemsPath, {
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
  });
  assert.throws(() => loadPlayerState(missingPendingItemsPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const badPromptTypePath = path.join(dir, "bad-prompt-type.json");
  writeJson(badPromptTypePath, {
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
  });
  assert.throws(() => loadPlayerState(badPromptTypePath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const badPendingItemsPath = path.join(dir, "bad-pending-items.json");
  writeJson(badPendingItemsPath, {
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
  });
  assert.throws(() => loadPlayerState(badPendingItemsPath), (error: unknown) => {
    assert.equal((error as { code?: string }).code, "CLI_STATE_INVALID");
    return true;
  });

  const inputPendingPath = path.join(dir, "input-pending.json");
  writeJson(inputPendingPath, {
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
  });
  const loadedInputPending = loadPlayerState(inputPendingPath);
  assert.equal(loadedInputPending.snapshot.pendingBoundary.kind, "input");
});
