# Project Status

## Current State

Milestones 1 through 6 are complete. Refrain v0.1.0 was released on 2026-09-25 as the first Spotify-only desktop release.

The released application includes:

- the Tauri/Svelte/Rust application foundation
- SQLite persistence for v0.1 source state and application settings
- Spotify Client ID configuration
- Authorization Code with PKCE
- a fixed loopback OAuth callback at `http://127.0.0.1:43817/callback`
- OS credential-store persistence for Spotify refresh credentials
- in-memory Spotify access tokens with refresh and reconnect behavior
- current-user profile retrieval
- paginated Liked Songs, saved-album, playlist, and playlist-item retrieval
- collection-level transactional source updates
- playlist snapshot reuse when persisted entries are complete
- saved albums represented as ordered `saved_album` source collections using shared Spotify track identities
- inaccessible playlist and unavailable item preservation
- bounded handling for 401, 429, transient network failures, and Spotify 5xx responses
- manual source refresh progress and cooperative cancellation, including the saved-album phase
- persisted-source read projections for desktop browsing
- Liked Songs, Saved Albums, Playlists, and Settings navigation
- saved-album and playlist detail with source ordering preserved
- playlist detail with intentional duplicate entries preserved
- loading, empty, inaccessible, disconnected, refresh, and error states
- restart hydration from SQLite without requiring a source refresh
- lazy artwork loading and a virtualized track list with paginated backend reads
- a viewport-bound desktop shell with panel-local scrolling and track virtualization that follows the available window height
- native application bundles and cross-platform Tauri build smoke checks
- tag-triggered GitHub Release packaging

The real Spotify authentication, source-refresh, desktop browsing, Saved Albums refresh/browse, and native resize smoke tests passed on macOS for the v0.1 development line.

## Active Work

Milestone 7, **Local Library Index**, is the next implementation milestone. No Milestone 7 implementation work is currently recorded.

Its existing scope is defined in `docs/IMPLEMENTATION.md` and begins with observational local-library indexing without moving or normalizing user files.

## Recent Changes

- released Refrain v0.1.0 on 2026-09-25
- completed Milestone 6 release hardening and tagged-release packaging
- added Windows, macOS, and Linux Tauri build smoke checks
- added native application bundle metadata and version-tag GitHub Release automation
- hardened fallback Spotify error messaging
- stabilized setup, testing, release, and codebase reference documentation
- completed the Saved Albums smoke test, native resize verification, and virtualized track-list regression fix

## Known Issues

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Unsigned or ad-hoc-signed release packages may require platform security confirmation. Production signing/notarization depends on release credentials being available.

## Next

Begin **Milestone 7: Local Library Index** according to `docs/IMPLEMENTATION.md`.

## Blockers

No implementation blocker is currently documented.

Public code signing and macOS notarization remain credential-dependent and are not required for local unsigned development builds.

## Open Decisions

The saved-album source-model decision is recorded in ADR 002. The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior and acceptance criteria.
- `docs/TDD.md` defines the baseline technical architecture and desktop data flow.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/reference/codebase-map.md` maps the current source structure.
- `docs/reference/setup.md` documents local setup and commands.
- `docs/reference/testing.md` documents validation coverage.
- `docs/reference/release.md` documents tag-driven release packaging.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision.
- `docs/decisions/002-saved-albums-as-source-collections.md` records the saved-album persistence and identity model.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and authentication behavior.
