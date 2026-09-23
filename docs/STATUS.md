# Project Status

## Current State

Milestones 1 through 3 are merged for the v0.1.0 development line. Milestone 4, Spotify Source Synchronization, is implemented on the active feature branch and is awaiting automated verification, a real source-refresh smoke test, and merge.

The application now includes:

- the Tauri/Svelte/Rust application foundation
- SQLite persistence for v0.1 source state and application settings
- Spotify Client ID configuration
- Authorization Code with PKCE
- a fixed loopback OAuth callback at `http://127.0.0.1:43817/callback`
- OS credential-store persistence for Spotify refresh credentials
- in-memory Spotify access tokens with refresh and reconnect behavior
- current-user profile retrieval
- paginated Liked Songs, playlist, and playlist-item retrieval
- collection-level transactional source updates
- playlist snapshot reuse when persisted entries are complete
- inaccessible playlist and unavailable item preservation
- bounded handling for 401, 429, transient network failures, and Spotify 5xx responses
- manual source refresh progress and cooperative cancellation

## Active Work

Verify and merge **Milestone 4: Spotify Source Synchronization**.

The Milestone 3 real Spotify authentication smoke test has passed on macOS. Restart persistence, disconnect, and reconnect were all verified with a real Spotify developer application.

## Recent Changes

- added Spotify profile, Liked Songs, playlist, and playlist-item source adapters
- mapped Spotify API DTOs into Refrain source-domain records before persistence
- added current and legacy playlist-item response compatibility
- added transactional collection replacement so failed refreshes preserve the previous good collection state
- added playlist `snapshot_id` reuse for unchanged complete collections
- added inaccessible followed-playlist handling without treating those playlists as empty
- added preservation for removed, unavailable, episode, and unsupported playlist positions
- added bounded network/5xx retries, 401 token refresh, and 429 `Retry-After` handling
- added manual refresh progress events and cooperative cancellation
- added a minimal refresh/cancel/status surface to the existing setup screen
- completed the real Milestone 3 authentication smoke test on macOS

## Known Issues

No known implementation defect is currently documented.

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Milestone 4 still needs a manual source-refresh smoke test against a real Spotify account after automated validation passes. Playlist and Liked Songs browsing are intentionally deferred to Milestone 5.

## Next

After Milestone 4 passes automated validation, real source-refresh smoke testing, and merge, implement **Milestone 5: v0.1 Desktop Experience**.

Milestone 5 adds:

- Liked Songs browsing
- playlist list and playlist detail views
- ordered track rows and intentional duplicate positions
- loading, empty, inaccessible, authentication-required, and refresh-error states
- restart-state hydration
- large-list rendering behavior

## Blockers

No implementation blocker is currently known.

A connected Spotify account with a user-owned developer application is required for the Milestone 4 real source-refresh smoke test.

## Open Decisions

No unresolved product or architectural decision blocks Milestone 4 verification or Milestone 5 planning.

The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior.
- `docs/TDD.md` defines the technical architecture and source-refresh safety requirements.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision that supersedes the earlier dynamic-port design.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and the authentication smoke test procedure.
