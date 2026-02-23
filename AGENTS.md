# AGENTS

This repository is set up for agent-first engineering of `ScriptLang`.

## Startup Checklist
1. Read `/README.md` for project intent and repo map.
2. Read `/docs/design-docs/core-beliefs.md` for non-negotiable engineering principles.
3. Read `/ARCHITECTURE.md` before writing implementation code.
4. If behavior changes, update `/docs/product-specs/index.md` first.
5. For non-trivial changes, create or update an execution plan in `/docs/exec-plans/active/`.

## Required Workflow
1. **Spec first**: product behavior belongs in `/docs/product-specs/`.
2. **Plan next**: implementation steps belong in `/docs/exec-plans/active/`.
3. **Code after plan**: implementation must follow the approved plan.
4. **Approval source is explicit**: plan approval is user confirmation in the current thread (or an existing accepted plan reference in repo history).
5. **Move plan to completed in the same commit** when delivering the implementation (no merge step required).
6. **Commit before handoff**: after `npm test` passes, create a `git commit` before ending the conversation.

## Development-Phase Compatibility Policy
- This project is in active development; changes do **not** need to preserve compatibility with older syntax formats or legacy behavior.
- Remove legacy traces, compatibility shims, and migration paths during implementation unless a task explicitly requires keeping them.
- Prefer clean replacement over transitional coexistence: implement as if old behavior never existed.

## Boundaries
- Keep parser, compiler, runtime, and host integration isolated.
- Do not mix XML parsing concerns with runtime execution logic.
- Do not bypass type checks with ad hoc runtime mutation paths.
- Keep snapshot/restore format versioned; do not keep backward-compat layers unless explicitly required by the task.

## Quality Gates
- **Sync docs before gates**: update `/README.md`, `/ARCHITECTURE.md`, and `/docs/` so they reflect the latest code behavior before running any gate commands.
- **Sync exec plans during doc sync**: verify each item in `/docs/exec-plans/active/`; if a plan is truly complete (code/docs/tests/gates all satisfied), mark its checklist done and move it to `/docs/exec-plans/completed/` in the same delivery commit.
- **Real completion evidence required**: do not move plans based on intent or partial progress; move only after implementation landed, docs synced, tests updated, and full gate passed.
- Run `npm run validate:docs` before commit.
- Run `npm run lint` before commit.
- Add/adjust tests for parser, runtime control-flow, and snapshot behavior with every behavior change.
- Tests run on Vitest (`npm run test:unit`).
- `npm test` always executes a strict pretest gate (`validate:docs`, `lint`, `typecheck`, and 100% Vitest coverage check) before running unit tests.
- If coverage is below 100%, add tests until coverage is exactly 100% for lines/branches/functions/statements, then rerun.
- After gate/tests pass, commit directly without asking for an extra confirmation round.
- Include exact file paths in implementation notes and reviews.

## Definition of Done
- Product spec and architecture docs are consistent with code.
- For non-trivial changes, execution plan exists and matches delivered behavior.
- Docs validation passes.
- New behavior is covered by tests or explicitly deferred in plan and tech debt tracker.
