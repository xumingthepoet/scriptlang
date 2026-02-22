# Security Model

## Code Execution
- `<code>` execution runs in a `node:vm` sandbox.
- Only whitelisted host functions are exposed.
- Execution timeouts are mandatory.

## Input Handling
- XML is parsed into typed structures before semantic execution.
- Unsupported or deprecated nodes fail compilation.

## Persistence
- Snapshot payload is treated as untrusted input on resume.
- Resume validates schema version and required fields before use.

