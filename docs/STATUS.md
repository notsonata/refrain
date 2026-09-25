# Project Status

## Current State

Milestones 1 through 8 are complete. Refrain v0.1.0 was released on 2026-09-25 as the first Spotify-only desktop release, and the v1 development line now includes the observational Local Library Index, deterministic Matching Engine, and a Milestone 9 Reconciliation Core implementation on the current development branch.

The application includes:

- the released v0.1 Spotify authentication, synchronization, persisted browsing, desktop UI, packaging, and release automation
- a configurable observational local-library index with metadata extraction, incremental rescans, lazy hashing, file-state tracking, moved-file recovery, and paginated reads
- `library_tracks` and `local_files` persistence from Milestone 7
- `track_links` and `track_rejections` persistence for matching decisions
- deterministic metadata normalization, bounded candidate generation, compatibility rules, scoring, and automatic/review/unresolved match outcomes
- persisted user confirmations and rejections exposed through Tauri commands and typed frontend wrappers
- `sync_runs` persistence with prepare-through-matching phase tracking
- one-active-sync coordination with cooperative cancellation at synchronization phase boundaries
- source-to-library reconciliation that reuses persisted links, creates stable `LibraryTrack` records, resolves preferred present files, and classifies desired tracks as matched, missing, or needing review
- typed sync-run Tauri commands and frontend wrappers for starting, cancelling, reading, and listing synchronization runs

Milestones 7 through 9 remain non-destructive with respect to library ownership. Milestone 9 does not acquire, move, normalize, or delete user audio files.

## Active Work

Milestone 9, **Reconciliation Core**, is implemented on the current development branch. The implementation includes dedicated reconciliation coverage for idempotency, shared library-track identity, duplicate playlist positions, manual decisions, cross-collection references, and cancellation-preserved committed state.

Static, type, lint, formatting, and build validation should pass before merge. The dedicated test suites remain part of pull-request/CI validation.

## Recent Changes

- added `sync_runs` persistence and paginated run history
- added one-active-sync coordination and cancellation across source refresh and reconciliation boundaries
- added initial synchronization phases for Spotify refresh, local-library scan, persisted-link resolution, matching, and missing-library-track creation
- added stable canonical `LibraryTrack` creation and preferred-present-file resolution
- added matched, missing, and needs-review reconciliation classification
- added `start_sync`, `cancel_sync`, `get_sync_run`, and `list_sync_runs` Tauri commands plus typed frontend wrappers
- added focused reconciliation integration coverage for the Milestone 9 verification cases
- retained the Milestone 7 observational local-library safeguards and Milestone 8 matching behavior
- released Refrain v0.1.0 on 2026-09-25

## Known Issues

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Unsigned or ad-hoc-signed release packages may require platform security confirmation. Production signing/notarization depends on release credentials being available.

## Next

Validate and merge **Milestone 9: Reconciliation Core**, then begin **Milestone 10: Filesystem Normalization and Ownership Safety** according to `docs/IMPLEMENTATION.md`.

## Blockers

No implementation blocker is currently documented.

Public code signing and macOS notarization remain credential-dependent and are not required for local unsigned development builds.

## Open Decisions

The saved-album source-model decision is recorded in ADR 002. The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior and acceptance criteria.
- `docs/TDD.md` defines the technical architecture, local-library scanner, matching engine, reconciliation engine, and desktop data flow.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/reference/codebase-map.md` maps the current source structure.
- `docs/reference/setup.md` documents local setup and library-root configuration.
- `docs/reference/testing.md` documents validation coverage, including local-library, matcher, and reconciliation tests.
- `docs/reference/release.md` documents tag-driven release packaging.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision.
- `docs/decisions/002-saved-albums-as-source-collections.md` records the saved-album persistence and identity model.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and authentication behavior.
