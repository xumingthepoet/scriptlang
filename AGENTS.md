# AGENTS

This repository is set up for agent-first engineering of `ScriptLang`.

## Startup Checklist
1. Read `/README.md` for project intent and repo map.
2. Read `/docs/HARNESS.md` for canonical delivery workflow.
3. Read `/docs/design-docs/core-beliefs.md` for non-negotiable engineering principles.
4. Read `/ARCHITECTURE.md` before writing implementation code.
5. If behavior changes, update `/docs/product-specs/index.md` first.
6. For non-trivial changes, create or update an execution plan in `/docs/exec-plans/active/`.

## Required Workflow
1. **Spec first**: product behavior belongs in `/docs/product-specs/`.
2. **Plan next**: implementation steps belong in `/docs/exec-plans/active/`.
3. **Code after plan**: implementation must follow the approved plan (for non-trivial changes).
4. **Follow authority doc**: use `/docs/HARNESS.md` for delivery sequence, gate, and commit policy.

## Boundaries
- Keep parser, compiler, runtime, and host integration isolated.
- Do not mix XML parsing concerns with runtime execution logic.
- Do not bypass type checks with ad hoc runtime mutation paths.
- Use `/ARCHITECTURE.md` as boundary authority.

## Quality Gates
- Gate order and commit requirements are defined in `/docs/HARNESS.md`.
- Test and coverage mechanics are defined in `/docs/TEST_WORKFLOW.md`.

## Definition of Done
- Product spec and architecture docs are consistent with code.
- For non-trivial changes, execution plan exists and matches delivered behavior.
- Delivery follows `/docs/HARNESS.md` end-to-end.
