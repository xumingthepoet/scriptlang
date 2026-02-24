import fs from "node:fs";
import path from "node:path";

import type { SnapshotV2 } from "../../core/types.js";

export const PLAYER_STATE_SCHEMA = "player-state.v2";

const PORTABLE_TYPE_KEY = "__scriptlang_portable_type__";
const PORTABLE_TYPE_MAP = "map";
const PORTABLE_TYPE_NUMBER = "number";
const PORTABLE_TYPE_UNDEFINED = "undefined";

interface PortableTaggedMap {
  [PORTABLE_TYPE_KEY]: typeof PORTABLE_TYPE_MAP;
  entries: [string, PortableJsonValue][];
}

interface PortableTaggedNumber {
  [PORTABLE_TYPE_KEY]: typeof PORTABLE_TYPE_NUMBER;
  value: "NaN" | "Infinity" | "-Infinity";
}

interface PortableTaggedUndefined {
  [PORTABLE_TYPE_KEY]: typeof PORTABLE_TYPE_UNDEFINED;
}

type PortableJsonValue =
  | null
  | boolean
  | string
  | number
  | PortableJsonValue[]
  | { [key: string]: PortableJsonValue }
  | PortableTaggedMap
  | PortableTaggedNumber
  | PortableTaggedUndefined;

export interface PlayerStateV2 {
  schemaVersion: typeof PLAYER_STATE_SCHEMA;
  scenarioId: string;
  compilerVersion: string;
  snapshot: SnapshotV2;
}

const makeCliError = (code: string, message: string): Error & { code: string } => {
  const error = new Error(message) as Error & { code: string };
  error.code = code;
  return error;
};

const isMapValue = (value: unknown): value is Map<unknown, unknown> => {
  return Object.prototype.toString.call(value) === "[object Map]";
};

const encodeNonFiniteNumber = (value: number): PortableTaggedNumber => {
  if (Number.isNaN(value)) {
    return {
      [PORTABLE_TYPE_KEY]: PORTABLE_TYPE_NUMBER,
      value: "NaN",
    };
  }
  if (value === Infinity) {
    return {
      [PORTABLE_TYPE_KEY]: PORTABLE_TYPE_NUMBER,
      value: "Infinity",
    };
  }
  return {
    [PORTABLE_TYPE_KEY]: PORTABLE_TYPE_NUMBER,
    value: "-Infinity",
  };
};

const encodePortableValue = (value: unknown, seen: WeakSet<object>): PortableJsonValue => {
  if (value === null) {
    return null;
  }
  if (value === undefined) {
    return { [PORTABLE_TYPE_KEY]: PORTABLE_TYPE_UNDEFINED };
  }
  if (typeof value === "boolean" || typeof value === "string") {
    return value;
  }
  if (typeof value === "number") {
    if (Number.isFinite(value)) {
      return value;
    }
    return encodeNonFiniteNumber(value);
  }
  if (Array.isArray(value)) {
    if (seen.has(value)) {
      throw makeCliError("CLI_STATE_INVALID", "State payload contains circular array references.");
    }
    seen.add(value);
    const encoded = value.map((item) => encodePortableValue(item, seen));
    seen.delete(value);
    return encoded;
  }
  if (isMapValue(value)) {
    if (seen.has(value as object)) {
      throw makeCliError("CLI_STATE_INVALID", "State payload contains circular map references.");
    }
    seen.add(value as object);
    const entries: [string, PortableJsonValue][] = [];
    for (const [key, entryValue] of value.entries()) {
      if (typeof key !== "string") {
        throw makeCliError("CLI_STATE_INVALID", "State payload map keys must be strings.");
      }
      entries.push([key, encodePortableValue(entryValue, seen)]);
    }
    seen.delete(value as object);
    return {
      [PORTABLE_TYPE_KEY]: PORTABLE_TYPE_MAP,
      entries,
    };
  }
  if (typeof value === "object") {
    const source = value as Record<string, unknown>;
    if (seen.has(source)) {
      throw makeCliError("CLI_STATE_INVALID", "State payload contains circular object references.");
    }
    seen.add(source);
    const encoded: Record<string, PortableJsonValue> = {};
    for (const [key, entryValue] of Object.entries(source)) {
      encoded[key] = encodePortableValue(entryValue, seen);
    }
    seen.delete(source);
    return encoded;
  }
  throw makeCliError("CLI_STATE_INVALID", "State payload contains unsupported value types.");
};

const decodePortableNumber = (value: string): number => {
  if (value === "NaN") {
    return Number.NaN;
  }
  if (value === "Infinity") {
    return Number.POSITIVE_INFINITY;
  }
  if (value === "-Infinity") {
    return Number.NEGATIVE_INFINITY;
  }
  throw makeCliError("CLI_STATE_INVALID", "State file number payload is invalid.");
};

const decodePortableValue = (value: unknown): unknown => {
  if (Array.isArray(value)) {
    return value.map((item) => decodePortableValue(item));
  }
  if (!value || typeof value !== "object") {
    return value;
  }

  const source = value as Record<string, unknown>;
  const taggedType = source[PORTABLE_TYPE_KEY];
  if (taggedType === PORTABLE_TYPE_UNDEFINED) {
    if (Object.keys(source).length !== 1) {
      throw makeCliError("CLI_STATE_INVALID", "State file undefined payload is invalid.");
    }
    return undefined;
  }
  if (taggedType === PORTABLE_TYPE_NUMBER) {
    if (Object.keys(source).length !== 2 || typeof source.value !== "string") {
      throw makeCliError("CLI_STATE_INVALID", "State file number payload is invalid.");
    }
    return decodePortableNumber(source.value);
  }
  if (taggedType === PORTABLE_TYPE_MAP) {
    if (Object.keys(source).length !== 2 || !Array.isArray(source.entries)) {
      throw makeCliError("CLI_STATE_INVALID", "State file map payload is invalid.");
    }
    const entries = source.entries;
    const result = new Map<string, unknown>();
    for (let i = 0; i < entries.length; i += 1) {
      const entry = entries[i];
      if (!Array.isArray(entry) || entry.length !== 2 || typeof entry[0] !== "string") {
        throw makeCliError("CLI_STATE_INVALID", "State file map entry payload is invalid.");
      }
      result.set(entry[0], decodePortableValue(entry[1]));
    }
    return result;
  }
  if (typeof taggedType === "string") {
    throw makeCliError("CLI_STATE_INVALID", `State file contains unknown portable type tag "${taggedType}".`);
  }

  const decoded: Record<string, unknown> = {};
  for (const [key, entryValue] of Object.entries(source)) {
    decoded[key] = decodePortableValue(entryValue);
  }
  return decoded;
};

export const createPlayerState = (
  scenarioId: string,
  compilerVersion: string,
  snapshot: SnapshotV2
): PlayerStateV2 => ({
  schemaVersion: PLAYER_STATE_SCHEMA,
  scenarioId,
  compilerVersion,
  snapshot,
});

export const savePlayerState = (statePath: string, state: PlayerStateV2): void => {
  const dir = path.dirname(statePath);
  fs.mkdirSync(dir, { recursive: true });
  const payload = encodePortableValue(state, new WeakSet<object>());
  fs.writeFileSync(statePath, JSON.stringify(payload), "utf8");
};

const isSnapshotV2 = (value: unknown): value is SnapshotV2 => {
  if (!value || typeof value !== "object") {
    return false;
  }
  const candidate = value as Partial<SnapshotV2>;
  const pendingBoundary = candidate.pendingBoundary as
    | {
        kind?: unknown;
        nodeId?: unknown;
        items?: unknown;
        promptText?: unknown;
        targetVar?: unknown;
        defaultText?: unknown;
      }
    | undefined;
  let pendingBoundaryValid = false;
  if (pendingBoundary && pendingBoundary.kind === "choice") {
    const items = pendingBoundary.items;
    pendingBoundaryValid =
      typeof pendingBoundary.nodeId === "string" &&
      Array.isArray(items) &&
      (pendingBoundary.promptText === null || typeof pendingBoundary.promptText === "string") &&
      items.every(
        (item) =>
          !!item &&
          typeof item === "object" &&
          typeof (item as { index?: unknown }).index === "number" &&
          Number.isInteger((item as { index: number }).index) &&
          typeof (item as { id?: unknown }).id === "string" &&
          typeof (item as { text?: unknown }).text === "string"
      );
  } else if (pendingBoundary && pendingBoundary.kind === "input") {
    pendingBoundaryValid =
      typeof pendingBoundary.nodeId === "string" &&
      typeof pendingBoundary.targetVar === "string" &&
      typeof pendingBoundary.promptText === "string" &&
      typeof pendingBoundary.defaultText === "string";
  }
  return (
    candidate.schemaVersion === "snapshot.v2" &&
    typeof candidate.compilerVersion === "string" &&
    typeof candidate.rngState === "number" &&
    Number.isInteger(candidate.rngState) &&
    candidate.rngState >= 0 &&
    candidate.rngState <= 0xffffffff &&
    pendingBoundaryValid
  );
};

export const loadPlayerState = (statePath: string): PlayerStateV2 => {
  if (!fs.existsSync(statePath)) {
    throw makeCliError("CLI_STATE_NOT_FOUND", `State file does not exist: ${statePath}`);
  }
  const raw = fs.readFileSync(statePath, "utf8");
  let parsedJson: unknown;
  try {
    parsedJson = JSON.parse(raw) as unknown;
  } catch {
    throw makeCliError("CLI_STATE_INVALID", "State file is invalid.");
  }
  const parsed = decodePortableValue(parsedJson) as Partial<PlayerStateV2>;
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
  if (!isSnapshotV2(parsed.snapshot)) {
    throw makeCliError("CLI_STATE_INVALID", "State snapshot payload is invalid.");
  }
  return parsed as PlayerStateV2;
};
