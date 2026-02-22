# Agent-First Engineering References

These references informed the initialization scaffold for this repository:

1. [Harness engineering](https://openai.com/index/harness-engineering/)
2. [Architecture as a maintained contract](https://matklad.github.io/2021/02/06/ARCHITECTURE.md.html)
3. [Execution plans for coding agents](https://cookbook.openai.com/examples/codex/execution_plans)
4. [Parse, don't validate](https://lexi-lambda.github.io/blog/2019/11/05/parse-don-t-validate/)
5. [Strict boundaries](https://www.seangoedecke.com/strict-boundaries/)

## How We Apply Them Here
- Use docs as enforceable constraints, not optional notes.
- Maintain explicit boundaries between parser/compiler/runtime/host layers.
- Treat plans as executable specs for non-trivial changes.
- Prefer typed intermediate structures and fail early on invalid states.

