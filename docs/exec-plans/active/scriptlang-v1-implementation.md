# ScriptLang V1 Implementation

## Objective
- Deliver a working ScriptLang V1 compiler/runtime with implicit group execution, code-node-driven mutation, and snapshot/restore.

## Scope
- In scope:
  - XML parser + typed IR generation.
  - Implicit group graph (`script` root + control-flow child groups).
  - Runtime engine (`next`, `choose`, `snapshot`, `resume`).
  - `<code>` execution via VM sandbox and host whitelist.
  - `call/ref/return` semantics with tail-position stack compaction.
- Out of scope:
  - Built-in random/seed primitives.
  - Async host functions.
  - Cross-version snapshot migration guarantees.

## Interfaces / Contracts Affected
- Public APIs:
  - `Engine.next()`
  - `Engine.choose(index)`
  - `Engine.snapshot()`
  - `Engine.resume(snapshot)`
- Schema:
  - `SnapshotV1` (group-path based cursor and scope chain).

## Implementation Steps
1. Define AST/IR/types and diagnostic model.
2. Implement parser and semantic analyzer with group ID generation.
3. Implement runtime stack machine and control-flow execution.
4. Implement VM-based code-node execution and host function whitelist.
5. Implement snapshot/restore and compatibility checks.
6. Add tests covering parser/runtime/snapshot call-flow scenarios.

## Verification
- Unit tests for AST/validation and expression typing.
- Integration tests for branching, call/return, and snapshot/resume.
- Negative tests for unsupported nodes (`set/push/remove`) and invalid snapshots.

## Risks and Mitigations
- Risk: unstable group IDs break restore.
  - Mitigation: deterministic ID generation from path + structural index.
- Risk: sandbox escape in code node execution.
  - Mitigation: strict VM context and whitelist-only host bridge.

## Done Criteria
- [x] Product spec updated with final syntax examples
- [x] Runtime APIs implemented with tests
- [x] Snapshot schema documented and validated
- [ ] Plan moved to `/docs/exec-plans/completed/`
