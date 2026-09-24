# AGENTS.md

## Project

**Name:** Refrain

**Purpose:** Refrain is a desktop application that mirrors Spotify playlists, Liked Songs, and saved albums to a normalized local music library.

## Project-Specific Instructions

### Current State

Milestones 1 through 5 of the v0.1.0 development line are implemented and verified. The Saved Albums scope extension, its virtualized track-list regression fix, and the native desktop resize correction are merged and manually verified on macOS.

Milestone 6, v0.1 Hardening and Release, is active. The current release-hardening work adds native bundle metadata, cross-platform Tauri build smoke checks, tag-triggered GitHub Release packaging, stable setup/testing/release references, and the initial release changelog.

The repository currently includes:

- the Tauri desktop scaffold and Svelte frontend
- the Rust backend foundation and baseline CI
- SQLite persistence for v0.1 source state and application settings
- Spotify Client ID configuration
- Authorization Code with PKCE
- the fixed loopback OAuth callback at `http://127.0.0.1:43817/callback`
- OS credential-store persistence for Spotify refresh credentials
- in-memory Spotify access tokens with refresh and reconnect behavior
- Spotify profile, Liked Songs, saved-album, playlist, and playlist-item synchronization
- collection-level transactional source persistence with playlist snapshot reuse
- saved albums represented as ordered `saved_album` source collections that reuse shared Spotify track identities
- inaccessible/unavailable Spotify item preservation
- bounded Spotify retry/rate-limit handling, manual refresh progress, and cancellation
- persisted browse projections for Spotify collections and entries
- v0.1 desktop navigation for Liked Songs, Saved Albums, Playlists, and Settings
- ordered saved-album and playlist detail, with intentional playlist duplicate positions preserved
- dense virtualized track rows with lazy Spotify album artwork
- viewport-bound native window sizing with panel-local scrolling
- Windows, macOS, and Linux Tauri build smoke checks
- Tauri desktop bundle configuration and version-tag release automation

The real Spotify authentication, source-refresh, desktop browsing, Saved Albums refresh/browse, and native resize smoke tests have passed on macOS for the v0.1 development line.

Do not invent dependencies, commands, modules, or implementation details that do not exist in the repository or approved project documentation.

### Stack

- Desktop runtime: Tauri 2
- Native backend: Rust
- Frontend: Svelte 5 + TypeScript + Vite
- Styling: Tailwind CSS 4
- Package manager: npm

### Commands

```text
Install:       npm install
Dev:           npm run tauri dev
Frontend:      npm run dev
Build:         npm run build
Desktop build: npm run tauri build -- --no-bundle
Lint:          npm run lint
Typecheck:     npm run check
Test:          npm test
Rust format:   npm run rust:fmt
Rust lint:     npm run rust:clippy
Rust test:     npm run rust:test
```

### Important Paths

Keep this short. Detailed navigation belongs in `docs/reference/codebase-map.md`.

```text
AGENTS.md                         Repository-wide agent instructions
src/                              Svelte frontend
src-tauri/                        Tauri/Rust desktop backend
src-tauri/src/source_sync.rs      Spotify source synchronization
src-tauri/src/saved_albums.rs     Spotify saved-album synchronization
src-tauri/src/db/source_browse.rs Persisted Spotify browse projections
src-tauri/tauri.conf.json         Desktop bundle/application configuration
.github/workflows/ci.yml          Baseline and cross-platform build CI
.github/workflows/release.yml     Tag-triggered GitHub Release packaging
CHANGELOG.md                      Released v0.1+ change history
docs/                             Project documentation
docs/BRIEF.md                     Product brief and project boundaries
docs/SPEC.md                      Product behavior and acceptance criteria
docs/TDD.md                       Technical design
docs/IMPLEMENTATION.md            Implementation sequence
docs/STATUS.md                    Current project context
docs/reference/codebase-map.md    Source navigation map
docs/reference/setup.md           Local development setup
docs/reference/testing.md         Validation strategy and smoke tests
docs/reference/release.md         Release workflow and packaging
docs/reference/spotify-auth.md    Spotify authentication setup and smoke test
```

Update this section only when important repository locations actually exist.

### Project Conventions

- Treat Spotify as the source of desired library state.
- Keep collection membership separate from audio-file identity.
- A track referenced by multiple Spotify collections should normally resolve to one logical library track and one canonical local copy.
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

## Documentation

Project documentation currently uses the following layout:

```text
docs/
├── BRIEF.md
├── SPEC.md
├── TDD.md
├── IMPLEMENTATION.md
├── STATUS.md
│
├── decisions/
│   ├── 001-*.md
│   └── 002-*.md
│
└── reference/
    ├── spotify-auth.md
    ├── codebase-map.md
    ├── setup.md
    ├── testing.md
    ├── release.md
    ├── api.md
    └── ui.md
```

Not every optional decision or reference document must exist before it is useful.

Do not invent files solely to make the documentation tree complete.

## Context Loading

Do not ingest every document before every task.

### Normal substantial work

Read:

1. this `AGENTS.md`
2. `docs/STATUS.md` when it exists
3. `docs/reference/codebase-map.md` when it exists
4. only the planning, ADR, or reference documents relevant to the requested work
5. directly related source, tests, and configuration

### Small isolated work

For trivial or clearly localized changes, read only the minimum context necessary.

### Broad or unfamiliar work

For:

- major features
- architecture changes
- cross-cutting changes
- substantial planning
- unfamiliar areas of the repository

read the relevant planning documents more broadly before proceeding.

Do not load unrelated documentation for completeness.

## Current Project Context

`docs/STATUS.md`, when present, is the project's current working memory.

It should remain concise and contain:

```markdown
# Project Status

## Current State

## Active Work

## Recent Changes

## Known Issues

## Next

## Blockers

## Open Decisions

## Relevant Context
```

Use it to understand what is happening in the repository now.

After meaningful work, update it when the project's state materially changes.

### Status rules

- Keep recent changes only while they remain useful to future work.
- Do not preserve a complete chronological history.
- Do not duplicate the full backlog.
- Do not copy planning documents into STATUS.
- Link to issues or ADRs where appropriate.
- Remove stale context.

Historical implementation detail belongs in Git and pull requests.

Individual work items belong in the issue tracker.

Durable decision rationale belongs in `docs/decisions/`.

Released changes belong in `CHANGELOG.md` when release history exists.

## Codebase Map

`docs/reference/codebase-map.md`, when present, is the project's navigation index.

Consult it before broadly searching the repository.

It should identify, as useful:

- major directories
- application entry points
- modules or domains
- routes
- services
- schemas and models
- important configuration
- test locations
- common task areas

Use the map to find likely files, then inspect those files directly.

Do not assume the map is authoritative if source code disagrees.

If the map does not identify what you need:

1. perform targeted search
2. inspect the relevant source
3. update the map if the discovery is useful for future work

Keep the map concise.

Do not list every file.

## Project Lifecycle

Use:

**Brief → Spec → Technical Design → Implementation Plan → Issues → Implementation → Verification → Release**

Work only on the stage relevant to the project.

### Brief

`docs/BRIEF.md`

Defines:

- problem and users
- proposed solution
- goals
- scope and non-goals
- constraints
- success criteria
- unresolved questions

**Complete when:** the project's purpose and boundaries are clear.

### Product Spec

`docs/SPEC.md`

Defines, as relevant:

- users and needs
- core flows
- features
- expected behavior
- important states and business rules
- significant edge cases
- acceptance criteria
- unresolved product decisions

**Complete when:** implementation should not require inventing major product behavior.

### Technical Design

`docs/TDD.md`

Defines, as relevant:

- architecture
- technology choices
- important data models
- APIs and interfaces
- integrations and background jobs
- authentication and security
- deployment
- testing strategy
- major tradeoffs
- unresolved technical decisions

Prefer the simplest design that satisfies the requirements.

Do not add infrastructure, dependencies, abstractions, or services without concrete need.

**Complete when:** implementation should not require inventing major architectural decisions.

### Implementation Plan

`docs/IMPLEMENTATION.md`

Defines:

- relevant current state
- milestones or implementation phases
- dependencies and ordering constraints
- verification expectations
- blockers or risks

Do not duplicate the Spec or TDD.

**Complete when:** work can proceed incrementally without rediscovering the overall sequence.

### Issues

Use the issue tracker for individual implementation work.

Issues should normally define:

- objective
- necessary context
- requirements
- acceptance criteria

Reference project documentation instead of copying it.

If an issue requires an unresolved major product or architecture decision, update the appropriate planning document first.

### Implementation

Before making consequential changes, use the relevant sources of truth:

1. this `AGENTS.md`
2. `docs/STATUS.md` when it exists
3. relevant planning documents
4. relevant ADRs
5. current issue or request
6. source and tests

Do not silently invent consequential decisions.

Minor implementation details do not require documentation.

### Verification

Do not treat code completion as task completion.

Verify as relevant:

- acceptance criteria
- static checks
- targeted tests
- integration behavior
- user-visible flows
- important edge cases
- build output
- regressions

Validation should be proportional to the change.

### Release

Create `CHANGELOG.md` when the project has meaningful released changes to preserve.

Do not create or maintain a changelog solely for unreleased milestone history. Until a release exists, use Git, pull requests, and `docs/STATUS.md` for implementation history and current context.

Do not create unnecessary release overhead.

## Architecture Decisions

Store significant decisions in:

```text
docs/decisions/
```

Create an ADR only when the reasoning is worth preserving.

Keep ADRs concise:

- context
- decision
- reason
- consequences

Do not create ADRs for routine implementation choices.

## Reference Documentation

Use `docs/reference/` for information agents need to look up while working.

Examples:

- `codebase-map.md`
- `setup.md`
- `testing.md`
- `api.md`
- `ui.md`

Create only what this project actually needs.

## Work Intake

For requested work:

1. understand the concrete goal
2. read current project context
3. use the codebase map when available to identify likely affected areas
4. inspect the relevant source
5. identify the smallest safe change
6. preserve existing behavior unless change is required

If ambiguity is minor, make the safest reasonable assumption.

If ambiguity affects consequential behavior or architecture, do not hide it behind an assumption.

## Implementation Rules

Make the smallest safe change that satisfies the task.

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

## Git Operations

- Use the `conventional-commits` skill whenever the user requests a commit, amend, or push.
- Do not infer authorization to commit or push.
- Commit or push only when explicitly requested.
- Do not create, switch, rename, merge, rebase, delete, publish, push, set upstreams for, or otherwise modify branches unless explicitly requested.
- Do not infer branch authorization from requests to implement, finish, commit, publish, or open a pull request.

## Testing Strategy

Use the lowest level of validation that gives credible confidence.

Typical order:

1. static checks
2. targeted unit tests
3. integration tests
4. E2E tests
5. build checks

Prefer focused validation over automatically running everything.

Run broader validation when changes affect shared infrastructure, routing, authentication, data models, build configuration, test infrastructure, or other wide surfaces.

Reuse existing testing frameworks and fixtures.

Do not claim verification that was not performed.

## Documentation Updates

Documentation review is part of completing meaningful work.

Before considering a substantial task, milestone, or release complete, review the repository documentation and update any document whose owned knowledge changed.

At minimum:

- update `docs/STATUS.md` whenever current state, active work, recent meaningful changes, known issues, next work, blockers, open decisions, or relevant context changed
- update `AGENTS.md` when repository-wide instructions, commands, stack, important paths, or current high-level implementation state changed
- update the codebase map when navigation-relevant structure changes
- update planning docs when documented product or technical decisions change
- create or update ADRs for significant durable decisions
- update reference docs when their subject changes
- update `CHANGELOG.md` only when meaningful released changes exist

Do not leave documentation knowingly stale after implementation work.

Do not update documentation merely because files were touched.

Avoid duplicating the same information across documents.

## Done Criteria

A task is complete when:

1. requested behavior is implemented
2. relevant validation passed, or skipped validation is explained
3. a final documentation review was performed and necessary documentation is updated
4. current project context is accurate
5. no required follow-up is hidden
6. completion reporting accurately describes the work

## Communication

Keep reports concise.

Report:

- what changed
- important files affected
- validation performed
- required follow-up, if any

Do not present speculative improvements as required work.
