# Codebase Map

This is a navigation index for the current Refrain codebase. Source remains authoritative.

## Repository root

- `src/` — Svelte 5 frontend
- `src-tauri/` — Tauri 2 / Rust desktop backend
- `.github/workflows/ci.yml` — pull-request and main-branch validation
- `.github/workflows/release.yml` — version-tag packaging and GitHub Release publishing
- `docs/` — product, technical, status, decision, and reference documentation
- `package.json` — frontend scripts, dependencies, and application version

## Frontend

- `src/main.ts` — frontend entry point
- `src/App.svelte` — desktop shell, navigation, Spotify connect/refresh flows, collection views, and local-library Settings controls
- `src/app.css` — application shell and viewport/layout rules
- `src/components/TrackList.svelte` — virtualized track rows and lazy artwork
- `src/lib/app-info.ts` — application-info Tauri command wrapper
- `src/lib/settings.ts` — settings command wrapper
- `src/lib/library.ts` — local-library overview, paginated file, scan, hash, and preferred-file command wrappers
- `src/lib/matching.ts` — matching evidence and manual confirm/reject/clear command wrappers
- `src/lib/sync.ts` — sync-run types plus start, cancel, get, and list command wrappers
- `src/lib/spotify.ts` — Spotify auth/refresh command wrappers and user-facing error formatting
- `src/lib/source.ts` — persisted source browse command wrappers and types
- `src/**/*.test.ts` — Vitest coverage for frontend helpers/components

## Rust backend

- `src-tauri/src/main.rs` — native binary entry point
- `src-tauri/src/lib.rs` — Tauri application builder and command registration
- `src-tauri/src/app.rs` — application initialization, data directory, logging, database, and shared state
- `src-tauri/src/commands/mod.rs` — Tauri command boundary exposed to the frontend
- `src-tauri/src/spotify.rs` — Spotify PKCE authentication, token lifecycle, and Spotify client behavior
- `src-tauri/src/source_sync.rs` — Spotify profile, Liked Songs, playlists, retry/rate-limit handling, progress, and cancellation
- `src-tauri/src/saved_albums.rs` — saved-album retrieval and refresh integration
- `src-tauri/src/local_library.rs` — observational local-library scanner, metadata extraction, lazy hashing, moved-file recovery, and scan progress
- `src-tauri/src/matching.rs` — deterministic metadata normalization, candidate indexing, compatibility checks, scoring, and match classification
- `src-tauri/src/reconciliation.rs` — sync coordination, initial sync orchestration, reconciliation commands, cancellation, and matched/missing/review classification
- `src-tauri/src/security.rs` — OS credential-store abstraction for Spotify refresh credentials

## Persistence

- `src-tauri/src/db/mod.rs` — SQLite initialization, migration execution, shared database behavior, and core persistence tests
- `src-tauri/src/db/settings.rs` — application settings persistence
- `src-tauri/src/db/source.rs` — source account/collection/track persistence
- `src-tauri/src/db/source_refresh.rs` — transactional Spotify refresh persistence
- `src-tauri/src/db/saved_albums.rs` — saved-album replacement/removal persistence
- `src-tauri/src/db/source_browse.rs` — paginated Spotify browse projections used by the desktop UI
- `src-tauri/src/db/local_library.rs` — local-file persistence, overview/pages, hash persistence, missing-state updates, and preferred-file selection
- `src-tauri/src/db/matching.rs` — source/library match descriptors plus persisted confirmations and rejections
- `src-tauri/src/db/reconciliation.rs` — sync-run persistence, accessible-source projection, library-track materialization, automatic links, and preferred-file resolution
- `src-tauri/migrations/0003_local_library.sql` — `library_tracks` and `local_files` v1 schema/indexes
- `src-tauri/migrations/0004_matching.sql` — `track_links` and `track_rejections` schema/indexes
- `src-tauri/migrations/0005_reconciliation.sql` — `sync_runs` persistence and chronological index
- `src-tauri/migrations/` — SQLite migrations

## Packaging and configuration

- `src-tauri/tauri.conf.json` — application identity, window/security settings, and desktop bundle metadata
- `src-tauri/Cargo.toml` — Rust package metadata and dependencies
- `src-tauri/icons/` — source application icons used by Tauri bundling
- `src-tauri/capabilities/default.json` — Tauri IPC capability permissions

## Documentation

- `docs/BRIEF.md` — project purpose and boundaries
- `docs/SPEC.md` — product behavior and acceptance criteria
- `docs/TDD.md` — technical design
- `docs/IMPLEMENTATION.md` — milestone sequence
- `docs/STATUS.md` — current working context
- `docs/decisions/` — durable architecture/product decisions
- `docs/reference/spotify-auth.md` — Spotify setup and auth smoke test
- `docs/reference/setup.md` — local development setup
- `docs/reference/testing.md` — validation strategy and release smoke tests
- `docs/reference/release.md` — version-tag packaging and release process

## Common task areas

| Task | Start here |
| --- | --- |
| Spotify sign-in / reconnect | `src-tauri/src/spotify.rs`, `src-tauri/src/commands/mod.rs`, `src/lib/spotify.ts` |
| Spotify source refresh | `src-tauri/src/source_sync.rs`, `src-tauri/src/saved_albums.rs` |
| Browse persisted collections | `src-tauri/src/db/source_browse.rs`, `src/lib/source.ts`, `src/App.svelte` |
| Local library scanning/indexing | `src-tauri/src/local_library.rs`, `src-tauri/src/db/local_library.rs`, `src/lib/library.ts`, `src/App.svelte` |
| Matching and manual decisions | `src-tauri/src/matching.rs`, `src-tauri/src/db/matching.rs`, `src/lib/matching.ts` |
| Full sync through reconciliation | `src-tauri/src/reconciliation.rs`, `src-tauri/src/db/reconciliation.rs`, `src/lib/sync.ts` |
| Database migrations | `src-tauri/migrations/`, `src-tauri/src/db/mod.rs` |
| Window/layout behavior | `src/App.svelte`, `src/app.css`, `src-tauri/tauri.conf.json` |
| CI/build validation | `.github/workflows/ci.yml`, `docs/reference/testing.md` |
| Packaging/release | `.github/workflows/release.yml`, `src-tauri/tauri.conf.json`, `docs/reference/release.md` |
