# Codebase Map

This is a navigation index for the current Refrain codebase. Source remains authoritative.

## Repository root

- `src/` — Svelte 5 frontend
- `src-tauri/` — Tauri 2 / Rust desktop backend
- `.github/workflows/ci.yml` — pull-request and main-branch validation
- `.github/workflows/release.yml` — version-tag packaging and GitHub Release publishing
- `docs/` — product, technical, status, decision, and reference documentation
- `package.json` — frontend scripts, dependencies, and application version
- `scripts/fetch-sockseek-sidecar.mjs` — pinned Sockseek release download, SHA-256 verification, extraction, and Tauri sidecar naming
- `THIRD_PARTY_NOTICES.md` — distributed third-party version, source, and license notices
- `third-party/sockseek/LICENSE` — bundled upstream Sockseek AGPL-3.0 license text

## Frontend

- `src/main.ts` — frontend entry point
- `src/App.svelte` — compact desktop shell, Local/Spotify navigation, collection grid/detail state, scoped synchronization orchestration, Spotify connect/refresh flows, Issues, and Settings controls
- `src/app.css` — application shell and viewport/layout rules
- `src/components/SpotifyWorkspace.svelte` — Liked Songs track view plus artwork-first Saved Albums/Playlists grids, collection detail navigation, persistent tracking controls, and collapsed unavailable playlists
- `src/components/TrackList.svelte` — virtualized dense Spotify track rows, lazy artwork, common/Advanced filters, local-state metadata, and per-track tracking overrides
- `src/components/LibraryView.svelte` — virtualized present-local-library rows with embedded artwork, visible paths, Spotify membership chips, common/Advanced filters, and Local Sync controls
- `src/components/IssuesView.svelte` — unresolved issue queue, issue details, candidate review, and manual match-decision controls
- `src/lib/app-info.ts` — application-info Tauri command wrapper
- `src/lib/acquisition.ts` — acquisition job/status projection types and Tauri command wrapper
- `src/lib/dialog.ts` — native Library root folder-picker wrapper
- `src/lib/settings.ts` — settings command wrapper
- `src/lib/sockseek.ts` — Soulseek credential and Sockseek provider-health command wrappers
- `src/lib/library.ts` — local-library overview, logical-track projection, paginated file, scan, hash, and preferred-file command wrappers
- `src/lib/issues.ts` — issue projection/count types, match-review data, and issue command wrappers
- `src/lib/matching.ts` — matching evidence and manual confirm/reject/clear command wrappers
- `src/lib/sync.ts` — scoped Local/Spotify sync-run types plus start, cancel, get, and filtered-list command wrappers
- `src/lib/spotify.ts` — Spotify auth/refresh command wrappers and user-facing error formatting
- `src/lib/source.ts` — persisted source browse command wrappers and types
- `src/**/*.test.ts` — Vitest coverage for frontend helpers/components

## Rust backend

- `src-tauri/src/main.rs` — native binary entry point
- `src-tauri/src/lib.rs` — Tauri application builder and command registration
- `src-tauri/src/acquisition.rs` — provider-neutral acquisition interface, coordinator, staging, retry/cancellation behavior, and fake-provider integration tests
- `src-tauri/src/app.rs` — application initialization, data directory, logging, database, and shared state
- `src-tauri/src/commands/mod.rs` — Tauri command boundary exposed to the frontend
- `src-tauri/src/spotify.rs` — Spotify PKCE authentication, token lifecycle, and Spotify client behavior
- `src-tauri/src/source_sync.rs` — Spotify profile, Liked Songs, playlists, retry/rate-limit handling, progress, and cancellation
- `src-tauri/src/saved_albums.rs` — saved-album retrieval and refresh integration
- `src-tauri/src/local_library.rs` — observational local-library scanner, metadata and embedded-artwork extraction, hashed artwork-cache writes, lazy audio hashing, moved-file recovery, and scan progress
- `src-tauri/src/matching.rs` — deterministic metadata normalization, candidate indexing, compatibility checks, scoring, and match classification
- `src-tauri/src/issues.rs` — issue and match-review application services exposed through Tauri commands
- `src-tauri/src/reconciliation.rs` — one-active-sync coordination plus distinct Local and Spotify orchestration, cancellation, matching, acquisition, and normalization phases
- `src-tauri/src/normalization.rs` — canonical library paths, portable filename sanitization, collision handling, safe moves, ownership-safe normalization, and guarded platform Trash integration
- `src-tauri/src/security.rs` — OS credential-store abstraction for Spotify refresh credentials
- `src-tauri/src/sockseek.rs` — Sockseek 3.0.5 sidecar lifecycle, HTTP adapter, SignalR wake/reconnect path, credential config materialization, staging, and version/health checks
- `src-tauri/src/domain/library.rs` — local-file and logical-library projection types
- `src-tauri/src/domain/acquisition.rs` — provider-neutral query/candidate/job/health types and persisted acquisition projection types
- `src-tauri/src/domain/issues.rs` — issue, issue-count, and match-review projection types

## Persistence

- `src-tauri/src/db/mod.rs` — SQLite initialization, migration execution, shared database behavior, and core persistence tests
- `src-tauri/src/db/acquisition.rs` — acquisition job queueing, status persistence, logical-track query projection, and paginated job reads
- `src-tauri/src/db/settings.rs` — application settings persistence
- `src-tauri/src/db/source.rs` — source account/collection/track persistence
- `src-tauri/src/db/source_refresh.rs` — transactional Spotify refresh persistence
- `src-tauri/src/db/saved_albums.rs` — saved-album replacement/removal persistence
- `src-tauri/src/db/source_browse.rs` — paginated Spotify browse projections used by the desktop UI
- `src-tauri/src/db/source_tracking.rs` — persistent collection defaults, per-track tracking overrides, and tracked-source desired-state projection
- `src-tauri/src/db/local_library.rs` — local-file persistence, overview/pages, logical-library rows, hash persistence, missing-state updates, and preferred-file selection
- `src-tauri/src/db/issues.rs` — computed non-match unresolved-state projections for missing/invalid files and inaccessible collections
- `src-tauri/src/db/matching.rs` — source/library match descriptors plus persisted confirmations and rejections
- `src-tauri/src/db/reconciliation.rs` — sync-run persistence, accessible-source projection, library-track materialization, automatic links, and preferred-file resolution
- `src-tauri/src/db/normalization.rs` — confidently resolved preferred-file projection and post-normalization path persistence
- `src-tauri/migrations/0003_local_library.sql` — `library_tracks` and `local_files` v1 schema/indexes
- `src-tauri/migrations/0004_matching.sql` — `track_links` and `track_rejections` schema/indexes
- `src-tauri/migrations/0005_reconciliation.sql` — `sync_runs` persistence and chronological index
- `src-tauri/migrations/0006_acquisition.sql` — durable provider-neutral `acquisition_jobs` persistence and indexes
- `src-tauri/migrations/0007_sync_scopes_and_tracking.sql` — scoped sync-run metadata plus persistent Spotify collection/track tracking rules
- `src-tauri/migrations/0008_local_artwork.sql` — cached embedded-artwork path and MIME references for local files
- `src-tauri/migrations/` — SQLite migrations

## Packaging and configuration

- `src-tauri/tauri.conf.json` — application identity, window/security settings, scoped local-artwork asset protocol/CSP configuration, and desktop bundle metadata
- `src-tauri/binaries/` — ignored target-specific Sockseek sidecar staging used by Tauri builds
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
| Browse logical library | `src-tauri/src/db/issues.rs`, `src/lib/library.ts`, `src/components/LibraryView.svelte`, `src/App.svelte` |
| Issues and manual resolution | `src-tauri/src/issues.rs`, `src-tauri/src/db/issues.rs`, `src/lib/issues.ts`, `src/components/IssuesView.svelte`, `src/App.svelte` |
| Acquisition provider boundary/status | `src-tauri/src/acquisition.rs`, `src-tauri/src/db/acquisition.rs`, `src-tauri/src/domain/acquisition.rs`, `src/lib/acquisition.ts` |
| Sockseek acquisition provider | `src-tauri/src/sockseek.rs`, `src-tauri/src/security.rs`, `src/lib/sockseek.ts`, `scripts/fetch-sockseek-sidecar.mjs` |
| Matching and manual decisions | `src-tauri/src/matching.rs`, `src-tauri/src/db/matching.rs`, `src/lib/matching.ts` |
| Local / Spotify scoped sync | `src-tauri/src/reconciliation.rs`, `src-tauri/src/db/reconciliation.rs`, `src-tauri/src/db/source_tracking.rs`, `src-tauri/src/normalization.rs`, `src/lib/sync.ts`, `src/App.svelte` |
| Filesystem normalization / ownership safety | `src-tauri/src/normalization.rs`, `src-tauri/src/db/normalization.rs`, `src-tauri/src/db/local_library.rs` |
| Database migrations | `src-tauri/migrations/`, `src-tauri/src/db/mod.rs` |
| Window/layout behavior | `src/App.svelte`, `src/app.css`, `src-tauri/tauri.conf.json` |
| CI/build validation | `.github/workflows/ci.yml`, `docs/reference/testing.md` |
| Packaging/release | `.github/workflows/release.yml`, `src-tauri/tauri.conf.json`, `docs/reference/release.md` |
