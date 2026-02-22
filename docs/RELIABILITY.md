# Reliability Checklist

- Snapshot schema version is explicit.
- Snapshot restore fails fast on incompatible schema.
- Runtime reports node/group location on errors.
- `waitingChoice` state transitions are deterministic.
- Host function failures are surfaced with context, never silently ignored.

