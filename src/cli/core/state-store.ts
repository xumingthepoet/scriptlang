import fs from "node:fs";
import path from "node:path";
import v8 from "node:v8";

import type { SnapshotV1 } from "../../core/types.js";

export const PLAYER_STATE_SCHEMA = "player-state.v1";

export interface PlayerStateV1 {
  schemaVersion: typeof PLAYER_STATE_SCHEMA;
  scenarioId: string;
  compilerVersion: string;
  snapshot: SnapshotV1;
}

const makeCliError = (code: string, message: string): Error & { code: string } => {
  const error = new Error(message) as Error & { code: string };
  error.code = code;
  return error;
};

export const createPlayerState = (
  scenarioId: string,
  compilerVersion: string,
  snapshot: SnapshotV1
): PlayerStateV1 => ({
  schemaVersion: PLAYER_STATE_SCHEMA,
  scenarioId,
  compilerVersion,
  snapshot,
});

export const savePlayerState = (statePath: string, state: PlayerStateV1): void => {
  const dir = path.dirname(statePath);
  fs.mkdirSync(dir, { recursive: true });
  const payload = v8.serialize(state);
  fs.writeFileSync(statePath, payload);
};

const isSnapshotV1 = (value: unknown): value is SnapshotV1 => {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<SnapshotV1>;
  const pendingChoiceItems = candidate.pendingChoiceItems;
  const pendingChoiceItemsValid =
    Array.isArray(pendingChoiceItems) &&
    pendingChoiceItems.every(
      (item) =>
        !!item &&
        typeof item === "object" &&
        typeof (item as { index?: unknown }).index === "number" &&
        Number.isInteger((item as { index: number }).index) &&
        typeof (item as { id?: unknown }).id === "string" &&
        typeof (item as { text?: unknown }).text === "string"
    );
  return (
    candidate.schemaVersion === "snapshot.v1" &&
    typeof candidate.compilerVersion === "string" &&
    typeof candidate.waitingChoice === "boolean" &&
    typeof candidate.rngState === "number" &&
    Number.isInteger(candidate.rngState) &&
    candidate.rngState >= 0 &&
    candidate.rngState <= 0xffffffff &&
    pendingChoiceItemsValid
  );
};

export const loadPlayerState = (statePath: string): PlayerStateV1 => {
  if (!fs.existsSync(statePath)) {
    throw makeCliError("CLI_STATE_NOT_FOUND", `State file does not exist: ${statePath}`);
  }
  const raw = fs.readFileSync(statePath);
  const parsed = v8.deserialize(raw) as Partial<PlayerStateV1>;
  if (!parsed || typeof parsed !== "object") {
    throw makeCliError("CLI_STATE_INVALID", "State file is invalid.");
  }
  if (parsed.schemaVersion !== PLAYER_STATE_SCHEMA) {
    throw makeCliError(
      "CLI_STATE_SCHEMA",
      `Unsupported player state schema: ${String(parsed.schemaVersion)}`
    );
  }
  if (typeof parsed.scenarioId !== "string" || parsed.scenarioId.length === 0) {
    throw makeCliError("CLI_STATE_INVALID", "State is missing scenarioId.");
  }
  if (typeof parsed.compilerVersion !== "string" || parsed.compilerVersion.length === 0) {
    throw makeCliError("CLI_STATE_INVALID", "State is missing compilerVersion.");
  }
  if (!isSnapshotV1(parsed.snapshot)) {
    throw makeCliError("CLI_STATE_INVALID", "State snapshot payload is invalid.");
  }
  return parsed as PlayerStateV1;
};
