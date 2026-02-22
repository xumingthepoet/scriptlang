# Architecture

This document defines strict module boundaries for `ScriptLang`.

## Layered System
1. **XML Parser Layer**
   - Input: `.script.xml` source text.
   - Output: raw syntax tree with location metadata.
   - No runtime behavior decisions.
2. **Semantic Analyzer Layer**
   - Input: raw syntax tree.
   - Output: typed IR with implicit `group` graph and stable IDs.
   - Performs validation and type checks.
3. **Runtime Engine Layer**
   - Input: typed IR and registered host functions.
   - Output: `next()/choose()` progression outputs, snapshot, restore.
   - Maintains group stack and scope chain.
4. **Host Integration Layer**
   - Script registration, host function whitelist, persistence I/O.
   - No parser/compiler logic.

## Core Boundaries
- Parser cannot execute code.
- Runtime cannot re-parse XML.
- Snapshot format is owned by runtime only.
- Host functions are accessed only through an explicit whitelist adapter.

## Data Contracts (V1)
- `ScriptIR`: root implicit group + node graph.
- `GroupFrame`: group id path + scope object + instruction pointer.
- `SnapshotV1`: execution cursor path, ancestor scopes, continuation frames, schema version.

## Stability Rules
- Internal IDs must be stable for identical source structure.
- Cross-version restore is best-effort only when schema compatibility is declared.
- New features must be added behind explicit IR node extensions, not ad hoc flags.

