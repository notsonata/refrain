# AGENTS.md

## Purpose

This file defines durable repository-wide behavior for AI coding agents working on Refrain.

Keep changing project state and detailed project knowledge in the documents that own them. Do not duplicate that information here.

## Sources of Truth

Use the existing documentation by responsibility:

| Source | Owns |
| --- | --- |
| `docs/STATUS.md` | Current project state, active work, recent changes, known issues, blockers, open decisions, release state, and next work |
| `docs/BRIEF.md` | Product purpose, goals, scope, non-goals, constraints, and success criteria |
| `docs/SPEC.md` | Product behavior, user flows, business rules, edge cases, and acceptance criteria |
| `docs/TDD.md` | Architecture, technology choices, data model, interfaces, integrations, security, and technical design |
| `docs/IMPLEMENTATION.md` | Milestones, implementation order, dependencies, verification expectations, and release sequence |
| `docs/decisions/` | Durable architecture or product decisions whose rationale should be preserved |
| `docs/reference/codebase-map.md` | Repository navigation, entry points, modules, configuration, tests, and common task areas |
| `docs/reference/setup.md` | Requirements, install/run commands, local setup, environment details, and common commands |
| `docs/reference/testing.md` | Validation strategy, automated checks, smoke tests, and release validation |
| `docs/reference/release.md` | Versioning, packaging, tagging, signing, and release workflow |
| `docs/reference/spotify-auth.md` | Spotify application setup, credential handling, and authentication smoke testing |
| `CHANGELOG.md` | Released user-visible changes |

Source code and configuration are authoritative for implemented behavior. If source and docs disagree, verify the implementation and update the stale document as part of the work.

Do not invent dependencies, commands, modules, behavior, infrastructure, or decisions that are absent from the repository or approved project documentation.

## Context Loading

Load only the context needed for the task.

For normal project work:

1. Read this `AGENTS.md`.
2. Read `docs/STATUS.md` when current project context matters.
3. Read `docs/reference/codebase-map.md` before broad source searching.
4. Read only the planning, decision, or reference documents relevant to the requested work.
5. Inspect the directly related source, tests, and configuration before editing.

For a small isolated change, use the minimum relevant context. For major features, architecture changes, cross-cutting work, substantial planning, or unfamiliar areas, read the relevant planning and decision documents more broadly.

Do not ingest unrelated documentation for completeness.

## Work Intake

For requested work:

1. Identify the concrete goal.
2. Check current project context when relevant.
3. Use the codebase map to locate likely affected areas.
4. Inspect the relevant source before editing.
5. Make the smallest safe change that satisfies the request.
6. Preserve existing behavior unless the task requires changing it.

If ambiguity is minor, make the safest reasonable assumption. If it affects consequential product behavior or architecture, resolve it from the appropriate source of truth or surface the unresolved decision.

Do not silently invent consequential decisions.

## Implementation Rules

Prefer:

- existing patterns
- localized changes
- explicit code
- simple interfaces
- reuse over new abstraction
- focused validation
- integration or E2E coverage for user-visible behavior

Avoid:

- unrelated cleanup
- unnecessary renames
- unjustified dependencies
- speculative abstractions
- hypothetical future-proofing
- broad refactors without concrete need
- mixed cleanup and feature work
- public behavior changes unless requested
- low-value tests

Add complexity only when current requirements justify it.

Use the issue tracker for individual implementation work when work is tracked there. Keep project-wide planning in `docs/IMPLEMENTATION.md`, current context in `docs/STATUS.md`, and durable decision rationale in `docs/decisions/`.

## Validation

Follow `docs/reference/testing.md` and use the lowest level of validation that gives credible confidence for the change.

Do not claim verification that was not performed. If relevant validation cannot be run, state exactly what was skipped and why.

## Documentation Updates

Documentation review is required for every completed project task, regardless of whether the work used a pull request, direct local changes, milestone work, release work, or another workflow.

After implementation and validation, perform a final documentation sync before reporting completion. Update only documents whose owned knowledge changed.

At minimum:

- update `docs/STATUS.md` whenever current state, active work, recent meaningful changes, known issues, next work, blockers, open decisions, release state, milestone state, or relevant context changed
- update `docs/BRIEF.md` when product purpose, scope, constraints, or success criteria change
- update `docs/SPEC.md` when product behavior, flows, rules, edge cases, or acceptance criteria change
- update `docs/TDD.md` when architecture, data models, interfaces, integrations, security, or technical design change
- update `docs/IMPLEMENTATION.md` when milestone scope, ordering, dependencies, verification expectations, or implementation assumptions change
- create or update an ADR in `docs/decisions/` when significant decision rationale should be preserved
- update `docs/reference/codebase-map.md` when navigation-relevant structure, entry points, or common task areas change
- update the relevant reference document when setup, testing, release, or Spotify authentication procedures change
- update `CHANGELOG.md` when a release is published or released user-visible behavior changes
- update this `AGENTS.md` only when durable agent workflow or repository-wide operating rules change

Do not leave documentation knowingly stale. Completed or merged work must not remain described as active in `docs/STATUS.md`.

Do not update a document merely because related files were touched. If its owned knowledge did not change, leave it unchanged.

## Git Operations

- Use the `conventional-commits` skill whenever the user requests a commit, amend, or push.
- Do not infer authorization to commit or push. Perform those actions only when explicitly requested.
- Do not create, switch, rename, merge, rebase, delete, publish, push, set an upstream for, or otherwise modify a branch unless the user explicitly requests that branch operation.
- Do not infer branch authorization from requests to implement, finish, commit, publish, or open a pull request.

## Done Criteria

A project task is complete when:

1. The requested behavior is implemented.
2. Relevant validation passed, or skipped validation is explained.
3. A final documentation review was performed after implementation and validation.
4. Every affected source-of-truth document reflects the completed state.
5. `docs/STATUS.md` does not present completed work as active when project state changed.
6. No required follow-up is hidden.
7. The completion report accurately describes the work and validation.

## Communication

Keep reports concise. Report what changed, important files affected, validation performed, and required follow-up if any.

Do not present speculative improvements as required work.
