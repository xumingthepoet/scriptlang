# Core Beliefs

This project follows an agent-first harness mindset:

1. **Tight feedback loops over one-shot implementation**
   - Build workflow and checks first.
   - Keep behavior observable and debuggable at every layer.
2. **Docs are executable constraints**
   - Product spec, architecture, and execution plan must exist before major code changes.
   - Changes without updated docs are incomplete.
3. **Strict boundaries and predictable structure**
   - Parser, analyzer, runtime, and host integration are separate modules.
   - Control-flow semantics live in runtime, not parser.
4. **Parse into typed structures, then validate**
   - Convert XML to explicit AST/IR types.
   - Reject invalid states as early as possible.
5. **Execution plans are first-class artifacts**
   - Every non-trivial change starts from a decision-complete plan.
   - Plans are moved from `active` to `completed` after delivery.
6. **Architecture docs are living references**
   - Architecture is not static prose; it is maintained alongside behavior changes.
7. **Deterministic core behavior**
   - Runtime behavior must be replayable from snapshot state.
   - V1 avoids hidden nondeterminism in language built-ins.

## Practical Rules for ScriptLang
- No builtin random in language semantics.
- Variable mutation happens in `<code>` nodes only.
- Snapshot/restore is group-stack based, not script-name based.
- `call` semantics mirror entering a child group with continuation behavior.

