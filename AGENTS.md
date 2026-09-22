# AGENTS.md

## Project

**Name:** Refrain

**Purpose:** Refrain is a desktop application that mirrors Spotify playlists and Liked Songs to a normalized local music library.

## Project-Specific Instructions

### Current State

The repository is currently documentation-first. Application code has not been scaffolded yet.

Do not invent package scripts, source directories, dependencies, build commands, or implementation details that do not exist in the repository or approved project documentation.

### Repository Structure

Document only important locations.

```text
README.md       Repository entry point
docs/           Project documentation
docs/BRIEF.md   Product brief and current project boundaries
```

Update this section only when important repository locations actually exist.

### Project Conventions

- Treat Spotify as the source of desired library state.
- Keep playlist membership separate from audio-file identity.
- A track referenced by multiple playlists should normally resolve to one logical library track and one canonical local copy.
- Keep logical library tracks separate from physical local files.
- Matching must be conservative, explainable, and prefer unresolved results over incorrect automatic matches.
- Persist user-confirmed match and rejection decisions so the same ambiguity is not repeatedly surfaced.
- Downloads must pass through staging and verification before entering the canonical library.
- Refrain owns synchronization, reconciliation, normalization, and library state.
- Acquisition backends must remain modular. Sockseek is the initial provider, not the sync engine.
- Refrain always normalizes managed library paths and filenames.
- Playlist exports and mirrored libraries should use portable relative paths where applicable.
- Automatic deletion must never remove unmanaged pre-existing user files.
- Keep implementation focused on current requirements. Do not add speculative mobile, cloud, media-server, or multi-provider infrastructure without an approved requirement.

### Project Constraints

- Desktop only for now: Windows, macOS, and Linux.
- Refrain must remain usable without Plex, Jellyfin, Navidrome, Lidarr, or another media server.
- Refrain does not ship shared Spotify credentials. Users provide their own Spotify application credentials.
- Spotify authentication must be suitable for a desktop application and must not require embedding a client secret.
- Local-library normalization is required behavior.
- Acquisition must remain provider-agnostic so additional providers can be added without changing core sync logic.
- Tracks that cannot be acquired or confidently matched must remain visible as unresolved or failed state rather than being silently substituted.
- Mobile support is out of scope until explicitly added to project documentation.

## Project Documentation

Project documentation lives in `docs/`.

Use the following lifecycle for substantial project work:

**Brief → Spec → Technical Design → Implementation Plan → Issues → Implementation → Verification → Release**

Do not create every artifact automatically.

Work on the document or stage appropriate to the project's current state.

Keep documentation proportional to project complexity.

## Lifecycle

### 1. Brief

`docs/BRIEF.md`

Defines:

- problem and target users
- proposed solution
- goals
- scope and non-goals
- important constraints
- success criteria
- unresolved questions

Keep the Brief product-focused.

Do not introduce detailed technical design unless it is a real project constraint.

**Complete when:** the project purpose, boundaries, and intended outcome are clear.

### 2. Product Spec

`docs/SPEC.md`

Defines, as relevant:

- users and their needs
- core flows
- features
- expected behavior
- important states and business rules
- significant edge cases
- acceptance criteria
- unresolved product decisions

Do not invent functionality for completeness.

**Complete when:** implementation should not require inventing major product behavior.

### 3. Technical Design

`docs/TDD.md`

Defines, as relevant:

- architecture
- technology choices
- important data models
- APIs and component interfaces
- integrations and background jobs
- authentication and security
- deployment
- testing strategy
- major tradeoffs
- unresolved technical decisions

Prefer the simplest design that satisfies documented requirements.

Do not introduce technologies, abstractions, services, infrastructure, or dependencies without a concrete need.

**Complete when:** implementation should not require inventing major architectural decisions.

### 4. Implementation Plan

`docs/IMPLEMENTATION.md`

Turns the approved design into an ordered sequence of work.

It should describe:

- relevant current state
- milestones or logical phases
- dependencies and ordering constraints
- verification expectations
- known blockers or risks

Do not repeat the Spec or TDD.

Reference them when deeper context is required.

**Complete when:** implementation can proceed incrementally without rediscovering the overall sequence.

### 5. Issues

Use GitHub Issues for individual units of implementation work.

An issue should normally define:

- objective
- necessary context
- requirements
- acceptance criteria

Reference project documentation instead of duplicating it.

If an issue requires a major product or architecture decision that is not documented, resolve that decision in the appropriate lifecycle document first.

### 6. Implementation

Before making a significant change, use the relevant sources of truth:

1. this `AGENTS.md`
2. `docs/BRIEF.md` for project purpose and boundaries
3. `docs/SPEC.md` for product behavior
4. `docs/TDD.md` for technical design
5. accepted ADRs
6. `docs/IMPLEMENTATION.md` for sequencing
7. the current issue or request
8. existing source and tests

Do not read all of these automatically.

Read only what is relevant to the task.

Do not silently resolve consequential ambiguity.

If a missing decision materially affects product behavior, architecture, compatibility, security, or future implementation, identify it as unresolved and update the appropriate documentation once resolved.

Minor implementation details do not require additional documentation.

### 7. Verification

Do not treat code completion as task completion.

Verify as relevant:

- acceptance criteria
- static checks
- targeted tests
- integration behavior
- user-visible flows
- important edge cases
- build output
- obvious regressions

Verification should be proportional to the change.

### 8. Release

A release represents verified work ready for actual use.

Use `CHANGELOG.md` when meaningful release history is useful.

Do not create unnecessary release-process overhead.

## Living Documentation

### `docs/STATUS.md`

Keep a concise snapshot of:

- current work
- recently completed work
- next major work
- blockers
- unresolved decisions

Do not turn `STATUS.md` into a history log or task database.

### Architecture Decision Records

Use:

```text
docs/decisions/
```

Create an ADR only for significant product or technical decisions whose reasoning is worth preserving.

Keep ADRs concise:

- context
- decision
- reason
- consequences

Do not create ADRs for routine implementation details.

### Optional Reference Docs

Create focused reference docs only when useful.

Examples:

- `docs/setup.md`
- `docs/testing.md`
- `docs/api.md`
- `docs/ui.md`
- `docs/codebase-map.md`

Do not create the full set by default.

## Codebase Inspection

Prefer targeted inspection over broad repository scans.

When starting work:

1. Read this file.
2. Read only documentation relevant to the task.
3. Search for related symbols, routes, components, schemas, commands, tests, or errors.
4. Inspect directly related source files.
5. Follow dependencies only as needed.

Avoid unrelated directories.

Do not inspect generated files, build output, dependency directories, lockfile contents, or large data files unless required.

Use `docs/codebase-map.md` as a navigation aid when present.

## Work Intake

For requested work:

1. Convert the request into a concrete goal.
2. Inspect relevant context before editing.
3. Identify the smallest safe change.
4. Preserve existing behavior unless the task requires changing it.
5. Avoid broad rewrites unless explicitly requested.

If ambiguity is minor, make the safest reasonable assumption and proceed.

If ambiguity affects an important product or technical decision, do not hide it behind an assumption.

## Implementation Rules

Make the smallest safe change that satisfies the task.

Prefer:

- existing patterns
- localized changes
- explicit code
- simple interfaces
- reuse over new abstraction
- focused validation
- integration or E2E tests for user-visible behavior

Avoid:

- unrelated cleanup
- unnecessary renames
- unjustified dependencies
- speculative abstractions
- hypothetical future-proofing
- broad refactors without a concrete need
- mixed cleanup and feature work
- public behavior changes unless requested
- low-value tests

Do not overengineer.

Add complexity only when current requirements justify it.

## Git Operations

- Use the `conventional-commits` skill whenever the user requests a commit, amend, or push, and apply it to every commit created or amended in that workflow.
- Do not infer authorization to commit or push.
- Commit or push only when explicitly requested.
- Do not create, switch, rename, merge, rebase, delete, publish, push, set an upstream for, or otherwise modify branches unless explicitly requested.
- Do not infer branch authorization from a request to implement, finish, commit, publish, or open a pull request.

## Testing Strategy

Use the lowest level of validation that gives credible confidence.

Typical order:

1. Static checks for affected areas
2. Targeted unit tests for complex logic, regressions, or edge cases
3. Integration tests for API, database, service, provider, or job boundaries
4. E2E tests for user-visible flows
5. Build checks for production, routing, packaging, or deployment changes

Prefer focused validation over running the full suite.

Run the full suite when:

- shared infrastructure changed
- routing, authentication, data models, build configuration, or test setup changed
- targeted validation suggests wider breakage
- the suite is inexpensive
- the user requests it

Reuse existing frameworks and fixtures.

Mock paid, rate-limited, nondeterministic, or external services at appropriate boundaries.

Do not test implementation details when user-observable behavior can be tested.

If validation cannot be run, state what was not run and why.

Do not claim verification that was not performed.

## Documentation Updates

Update documentation only when implementation changes documented behavior, architecture, setup, API contracts, UI conventions, testing strategy, developer workflow, or project state.

Keep each piece of information in the document that owns it.

Avoid duplicating the same information across documents.

If implementation changes a documented decision, update the relevant document.

Documentation should describe the project that actually exists or has been explicitly decided.

## Done Criteria

A task is complete when:

1. The requested behavior is implemented.
2. Relevant validation passed, or skipped validation is clearly explained.
3. Relevant documentation is updated where necessary.
4. No known required follow-up is hidden.
5. The completion report accurately describes the work and validation.

## Communication

Keep completion reports concise.

Report:

- what changed
- important files affected
- validation performed
- required follow-up, if any

Do not present speculative improvements as required work.
