# Project Status

## Current State

Milestones 1 through 5 are merged and verified for the v0.1.0 development line. The Saved Albums v0.1 scope extension and its virtualized track-list regression fix are merged. Before Milestone 6 begins, the desktop shell is being corrected so resizing the native Tauri window resizes the application layout instead of turning the whole webview into a scrolling document.

The application now includes:

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

Verify native window resizing across the main browsing and Settings views, then complete the remaining Saved Albums smoke-test coverage before starting Milestone 6.

The real Spotify authentication smoke test for Milestone 3, source-refresh smoke test for Milestone 4, and desktop browsing smoke test for Milestone 5 have passed on macOS.

## Recent Changes

- completed the real Milestone 5 desktop browsing smoke test
- merged first-class Spotify Saved Albums support through PR #16
- added saved-album retrieval using the existing `user-library-read` authorization scope
- represented each saved album as a `saved_album` source collection while reusing shared `source_tracks`
- preserved album track order and saved timestamps in collection entries
- made saved-album replacement atomic and remove stale album collections when albums are unsaved
- preserved richer persisted track identity such as ISRC when simplified album-track payloads do not provide it
- added Saved Albums browsing with paginated album lists and existing virtualized track detail
- retained cooperative cancellation during the added saved-album refresh phase
- merged the stale virtual-scroll fix through PR #17 so short collections no longer inherit an invalid virtual window after collection changes
- constrained the desktop shell to the native viewport, moved scrolling into navigation/content panels, and made the virtual track viewport follow the actual available height
- recorded the source-model decision in `docs/decisions/002-saved-albums-as-source-collections.md`

## Known Issues

The native desktop resize correction still requires real-app verification at the configured minimum window size and at larger sizes on the supported desktop platforms.

The saved-album rendering fix is merged but still needs real-app confirmation during the remaining saved-album smoke test.

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

The saved-albums extension still needs the remaining real Spotify smoke-test coverage for album track order, restart hydration, and removal of an unsaved album when practical.

## Next

After native resize behavior and the remaining Saved Albums verification pass, implement **Milestone 6: v0.1 Hardening and Release**.

Milestone 6 covers release metadata and icons, clean first-run and migration checks, user-facing error hardening, log-redaction verification, platform build checks, stable developer/reference documentation, the initial release changelog, and v0.1.0 packaging.

## Blockers

No implementation blocker is currently known.

A connected Spotify account with at least one saved album is required for the remaining real saved-album smoke test.

## Open Decisions

No unresolved product or architectural decision blocks the current desktop-layout verification, saved-album verification, or Milestone 6 planning.

The saved-album source-model decision is recorded in ADR 002. The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior and acceptance criteria.
- `docs/TDD.md` defines the baseline technical architecture and desktop data flow.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates; the approved Saved Albums scope extension and current UI follow-ups sit between Milestones 5 and 6.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision.
- `docs/decisions/002-saved-albums-as-source-collections.md` records the saved-album persistence and identity model and supersedes the earlier two-kind source-collection assumption.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and authentication behavior.
