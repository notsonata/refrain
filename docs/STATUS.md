# Project Status

## Current State

Milestones 1 through 11 are complete. Refrain v0.1.0 was released on 2026-09-25 as the first Spotify-only desktop release, and the v1 development line now includes the observational Local Library Index, deterministic Matching Engine, Reconciliation Core, Filesystem Normalization and Ownership Safety, and Issues and Manual Resolution UI.

The application includes:

- the released v0.1 Spotify authentication, synchronization, persisted browsing, desktop UI, packaging, and release automation
- a configurable observational local-library index with metadata extraction, incremental rescans, lazy hashing, file-state tracking, moved-file recovery, and paginated reads
- `library_tracks` and `local_files` persistence from Milestone 7
- `track_links` and `track_rejections` persistence for matching decisions
- deterministic metadata normalization, bounded candidate generation, compatibility rules, scoring, and automatic/review/unresolved match outcomes
- persisted user confirmations and rejections exposed through Tauri commands and typed frontend wrappers
- `sync_runs` persistence with prepare-through-normalization phase tracking
- one-active-sync coordination with cooperative cancellation at synchronization phase boundaries
- source-to-library reconciliation that reuses persisted links, creates stable `LibraryTrack` records, resolves preferred present files, and classifies desired tracks as matched, missing, or needing review
- canonical filesystem normalization for confidently resolved preferred files, using Spotify track metadata for stable artist/album/track paths
- cross-platform filename sanitization, case-insensitive collision handling, stable collision suffixes, case-only rename staging, and verified copy fallback for cross-filesystem moves
- path-boundary validation plus ownership-preserving moves that keep external files external and never automatically delete external duplicates
- Trash / Recycle Bin integration guarded to managed files for future cleanup work
- paginated Library and Issues views backed by durable local-library and reconciliation state
- issue projections for ambiguous matches, missing local files, inaccessible Spotify collections, and invalid local files
- match-review controls for confirming, rejecting, and clearing persisted manual decisions, with resolved issues disappearing from the projection automatically
- typed sync-run Tauri commands and frontend wrappers for starting, cancelling, reading, and listing synchronization runs

Milestones 7 through 9 remain non-destructive with respect to user audio. Milestone 10 may move confidently resolved preferred files into the canonical library structure, but it preserves each file's ownership classification and does not automatically delete external files.

## Active Work

No milestone implementation is currently active. Milestone 11 is complete; Milestone 12 is next.

## Recent Changes

- merged Milestone 9 Reconciliation Core with durable sync runs, one-active-sync coordination, stable source-to-library reconciliation, and cancellation-preserved committed state
- added canonical normalized path construction with single-disc and multi-disc filenames plus unknown album/year fallbacks
- added NFKC filesystem sanitization, Windows reserved-name protection, deterministic component shortening, and case-insensitive collision suffixing
- added atomic rename behavior, case-only rename staging, verified cross-filesystem copy-and-move fallback, and safe path-boundary checks
- preserved preferred-file and managed/external ownership state after moves; external files are never automatically deleted
- added managed-file Trash / Recycle Bin integration for future cleanup workflows
- added Milestone 10 filesystem normalization tests covering the implementation-plan verification cases
- merged Milestone 10 Filesystem Normalization and Ownership Safety
- restored deterministic JavaScript dependency installation for CI with `package-lock.json` and `npm ci`
- added dense Library and Issues desktop views with current issue counts, details, and paginated projections
- added durable issue projections for ambiguous matches, missing/invalid local files, and inaccessible Spotify collections without introducing a separate issue table
- added match-review candidate inspection and persisted confirm/reject/clear actions that refresh the issue projection after resolution
- added frontend and Rust coverage for issue projection behavior and automatic issue removal after durable state repair
- released Refrain v0.1.0 on 2026-09-25

## Known Issues

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Unsigned or ad-hoc-signed release packages may require platform security confirmation. Production signing/notarization depends on release credentials being available.

## Next

Begin **Milestone 12: Acquisition Provider Boundary** according to `docs/IMPLEMENTATION.md`.

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
- `docs/reference/testing.md` documents validation coverage, including local-library, matcher, reconciliation, filesystem-normalization, and issue/manual-resolution tests.
- `docs/reference/release.md` documents tag-driven release packaging.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision.
- `docs/decisions/002-saved-albums-as-source-collections.md` records the saved-album persistence and identity model.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and authentication behavior.
