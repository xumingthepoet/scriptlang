# AGENTS

This repository is set up for agent-first engineering of `ScriptLang`.

## Startup Checklist
1. Read `/README.md` for project intent and repo map.
2. Read `/docs/design-docs/core-beliefs.md` for non-negotiable engineering principles.
3. Read `/ARCHITECTURE.md` before writing implementation code.
4. If behavior changes, update `/docs/product-specs/index.md` first.
5. Create or update an execution plan in `/docs/exec-plans/active/`.

## Required Workflow
1. **Spec first**: product behavior belongs in `/docs/product-specs/`.
2. **Plan next**: implementation steps belong in `/docs/exec-plans/active/`.
3. **Code after plan**: implementation must follow the approved plan.
4. **Move plan to completed in the same commit** when delivering the implementation (no merge step required).

## Boundaries
- Keep parser, compiler, runtime, and host integration isolated.
- Do not mix XML parsing concerns with runtime execution logic.
- Do not bypass type checks with ad hoc runtime mutation paths.
- Keep snapshot/restore format versioned and backward-aware.

## Quality Gates
- Run `npm run validate:docs` before commit.
- Run `npm run lint` before commit.
- Add/adjust tests for parser, runtime control-flow, and snapshot behavior with every behavior change.
- Tests run on Vitest (`npm run test:unit`).
- `npm test` always executes a strict pretest gate (`validate:docs`, `lint`, `typecheck`, and 100% Vitest coverage check) before running unit tests.
- If coverage is below 100%, add tests until coverage is exactly 100% for lines/branches/functions/statements, then rerun.
- Include exact file paths in implementation notes and reviews.

## Definition of Done
- Product spec and architecture docs are consistent with code.
- Execution plan exists and matches delivered behavior.
- Docs validation passes.
- New behavior is covered by tests or explicitly deferred in plan and tech debt tracker.
