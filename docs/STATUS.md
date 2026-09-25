# Project Status

## Current State

Milestones 1 through 8 are complete. Refrain v0.1.0 was released on 2026-09-25 as the first Spotify-only desktop release, and the v1 development line now includes the observational Local Library Index and deterministic Matching Engine.

The application includes:

- the released v0.1 Spotify authentication, synchronization, persisted browsing, desktop UI, packaging, and release automation
- a configurable observational local-library index with metadata extraction, incremental rescans, lazy hashing, file-state tracking, moved-file recovery, and paginated reads
- `library_tracks` and `local_files` persistence from Milestone 7
- `track_links` and `track_rejections` persistence for matching decisions
- deterministic Unicode NFKC comparison normalization, punctuation/whitespace normalization, `&`/`and` equivalence, featured-artist extraction, and version qualifier parsing
- bounded candidate generation using ISRC, normalized title, primary artist, duration buckets, and a trigram-bounded fuzzy title fallback
- hard version/duration compatibility rules, exact-ISRC handling, weighted metadata scoring, and runner-up margin checks
- automatic, review, and unresolved match outcomes with reusable per-candidate evidence
- persisted user confirmations and rejections exposed through Tauri commands and typed frontend wrappers

Milestones 7 and 8 are observational with respect to the filesystem. They do not move, rename, normalize, delete, or acquire user audio files.

## Active Work

Milestone 8, **Matching Engine**, is implemented and passes its automated verification set.

No Milestone 9 implementation work is currently recorded.

## Recent Changes

- implemented Milestone 8 matching schema, matcher domain evidence, persistence, Tauri commands, and frontend command wrappers
- added deterministic metadata normalization and recognized version parsing
- added bounded candidate indexes, hard compatibility checks, duration scoring, weighted metadata scoring, strong ISRC matching, and runner-up ambiguity handling
- added persisted manual confirmation/rejection/clear behavior
- added a representative matcher fixture corpus covering the Milestone 8 verification cases
- retained the Milestone 7 observational local-library index and its filesystem safeguards
- released Refrain v0.1.0 on 2026-09-25

## Known Issues

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Unsigned or ad-hoc-signed release packages may require platform security confirmation. Production signing/notarization depends on release credentials being available.

## Next

Begin **Milestone 9: Reconciliation Core** according to `docs/IMPLEMENTATION.md`.

## Blockers

No implementation blocker is currently documented.

Public code signing and macOS notarization remain credential-dependent and are not required for local unsigned development builds.

## Open Decisions

The saved-album source-model decision is recorded in ADR 002. The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior and acceptance criteria.
- `docs/TDD.md` defines the technical architecture, local-library scanner, matching engine, and desktop data flow.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/reference/codebase-map.md` maps the current source structure.
- `docs/reference/setup.md` documents local setup and library-root configuration.
- `docs/reference/testing.md` documents validation coverage, including local-library and matcher tests.
- `docs/reference/release.md` documents tag-driven release packaging.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision.
- `docs/decisions/002-saved-albums-as-source-collections.md` records the saved-album persistence and identity model.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and authentication behavior.
