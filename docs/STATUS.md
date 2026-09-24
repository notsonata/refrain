# Project Status

## Current State

Milestones 1 through 5 are merged and verified for the v0.1.0 development line. The Saved Albums v0.1 scope extension, virtualized track-list regression fix, and native desktop resize correction are also merged and manually verified on macOS.

Milestone 6, **v0.1 Hardening and Release**, is now active.

The application includes:

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

## Active Work

Complete Milestone 6 release hardening:

- enable native installer/application bundles
- add Windows, macOS, and Linux Tauri build smoke checks
- add tag-triggered GitHub Release packaging
- harden fallback Spotify error messaging
- verify clean-database migration, restart persistence, and log-secret handling
- stabilize setup, testing, release, and codebase reference documentation
- prepare the initial `CHANGELOG.md`

The real Spotify authentication smoke test for Milestone 3, source-refresh smoke test for Milestone 4, desktop browsing smoke test for Milestone 5, remaining Saved Albums refresh/browse checks, and native resize checks have passed on macOS.

## Recent Changes

- completed the remaining Saved Albums smoke test, including album ordering, restart hydration, rendering, and unsaved-album removal behavior
- verified native window resizing at the configured minimum size and larger sizes across the browsing and Settings views
- merged first-class Spotify Saved Albums support through PR #16
- merged the stale virtual-scroll fix through PR #17
- merged the native viewport sizing correction through PR #19
- recorded the saved-album source-model decision in `docs/decisions/002-saved-albums-as-source-collections.md`

## Known Issues

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Unsigned or ad-hoc-signed release packages may require platform security confirmation. Production signing/notarization depends on release credentials being available.

## Next

Finish the Milestone 6 validation set, merge the release-hardening work, then tag `v0.1.0` to produce the first GitHub Release packages.

After v0.1.0 is released, begin **Milestone 7: Local Library Index**.

## Blockers

No implementation blocker is currently known.

Public code signing and macOS notarization remain credential-dependent and are not required for local unsigned development builds.

## Open Decisions

No unresolved product or architectural decision blocks Milestone 6.

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
