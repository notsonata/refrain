# Project Status

## Current State

Milestones 1 through 7 are complete. Refrain v0.1.0 was released on 2026-09-25 as the first Spotify-only desktop release, and the v1 development line now includes the observational Local Library Index.

The application includes:

- the released v0.1 Spotify authentication, synchronization, persisted browsing, desktop UI, packaging, and release automation
- a configurable local-library root stored in application settings
- `library_tracks` and `local_files` v1 persistence schema
- recursive local audio scanning for FLAC, MP3, M4A/AAC, OGG, Opus, WAV, and ALAC paths supported by the metadata layer
- `lofty` metadata/property extraction with invalid-file visibility
- incremental rescans using file size and modification time
- lazy BLAKE3 hashing with stale-hash invalidation after file changes
- present, missing, and invalid local-file state
- conservative moved/renamed-file recovery using known hashes or strong metadata
- duplicate-safe physical-file indexing and preferred-file selection primitives
- local-library scan progress events and paginated local-file queries
- Settings controls for saving the library root, starting a scan, and viewing index counts/progress

Milestone 7 scanning is observational. It does not move, rename, normalize, delete, or acquire user audio files.

## Active Work

Milestone 7, **Local Library Index**, is implemented and passes its automated verification set.

No Milestone 8 implementation work is currently recorded.

## Recent Changes

- implemented Milestone 7 local-library schema, domain types, scanner, persistence, IPC commands, and Settings controls
- added incremental scanning, lazy BLAKE3 hashing, moved-file recovery, invalid/missing state, preferred-file primitives, and paginated queries
- added filesystem integration coverage for new, unchanged, changed, removed, moved, malformed, unsupported, symlink, duplicate, and large-directory cases
- verified frontend format/lint/type checks, frontend tests/build, Rust format/Clippy/tests, and a native Tauri no-bundle release build
- released Refrain v0.1.0 on 2026-09-25

## Known Issues

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Unsigned or ad-hoc-signed release packages may require platform security confirmation. Production signing/notarization depends on release credentials being available.

## Next

Begin **Milestone 8: Matching Engine** according to `docs/IMPLEMENTATION.md`.

## Blockers

No implementation blocker is currently documented.

Public code signing and macOS notarization remain credential-dependent and are not required for local unsigned development builds.

## Open Decisions

The saved-album source-model decision is recorded in ADR 002. The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior and acceptance criteria.
- `docs/TDD.md` defines the technical architecture, local-library scanner design, and desktop data flow.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/reference/codebase-map.md` maps the current source structure.
- `docs/reference/setup.md` documents local setup and library-root configuration.
- `docs/reference/testing.md` documents validation coverage, including local-library scanner tests.
- `docs/reference/release.md` documents tag-driven release packaging.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision.
- `docs/decisions/002-saved-albums-as-source-collections.md` records the saved-album persistence and identity model.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and authentication behavior.
