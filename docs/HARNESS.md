# Harness Workflow Authority

This document is the single source of truth for agent delivery workflow in this repository.

## Why This Exists
- Harness guidance from [OpenAI: Harness Engineering](https://openai.com/index/harness-engineering/) emphasizes reusable scaffolding, context externalization, and continuous entropy cleanup.
- To avoid doc drift, workflow rules are centralized here and referenced elsewhere.

## Responsibility Map

| Document | Owns |
| --- | --- |
| `/docs/HARNESS.md` | End-to-end workflow, doc-sync order, gate sequence, commit policy |
| `/AGENTS.md` | Agent startup checklist and pointers to authority docs |
| `/docs/TEST_WORKFLOW.md` | Test and coverage mechanics only |
| `/docs/exec-plans/README.md` | Execution-plan file lifecycle and template usage |
| `/docs/design-docs/core-beliefs.md` | Principles, not step-by-step procedure |
| `/README.md` | Project overview and entry links |

## Canonical Delivery Workflow
1. Read context:
   - `/README.md`
   - `/docs/design-docs/core-beliefs.md`
   - `/ARCHITECTURE.md`
2. Spec first:
   - if behavior changes, update `/docs/product-specs/` first.
3. Plan next:
   - for non-trivial work, create/update plan in `/docs/exec-plans/active/`.
4. Implement:
   - follow architecture boundaries.
5. Sync docs before gates:
   - update `/README.md`, `/ARCHITECTURE.md`, and impacted files in `/docs/`.
   - audit `/docs/exec-plans/active/`; move only truly completed plans.
6. Run full gate:
   - `npm test` (includes `validate:docs`, `lint`, `typecheck`, `coverage:strict`, then unit tests).
7. Commit before handoff:
   - if `npm test` passes, create a `git commit` before ending the conversation.
   - no extra approval round is required once harness gates pass.
8. Finish:
   - for non-trivial changes, move completed plan to `/docs/exec-plans/completed/` in the same delivery commit.

## Completion Evidence Rule
- A plan is "truly completed" only when all are true:
  - implementation landed.
  - docs synced to current behavior.
  - tests updated as needed.
  - full gate passed.
  - done checklist is fully checked.

## Development-Phase Compatibility Policy
- The project is in active development.
- By default, do not preserve backward compatibility for legacy syntax/behavior unless explicitly required by task.
- Remove legacy shims/traces instead of keeping transitional paths.

## Change Routing (Anti-Drift)
- Change process order or approval/commit policy: edit `/docs/HARNESS.md`.
- Change test command details: edit `/docs/TEST_WORKFLOW.md`.
- Change execution-plan folder rules: edit `/docs/exec-plans/README.md`.
- Change engineering principles: edit `/docs/design-docs/core-beliefs.md`.
- Change project overview links: edit `/README.md`.
