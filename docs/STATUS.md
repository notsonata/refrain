# Project Status

## Current State

Milestones 1 through 4 are merged and verified for the v0.1.0 development line. Milestone 5, the v0.1 Desktop Experience, is implemented on the active feature branch and is awaiting automated validation, a manual desktop browsing smoke test, and merge.

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
- persisted-source read projections for desktop browsing
- Liked Songs, Playlists, and Settings navigation
- playlist detail with ordered and intentionally duplicated entries preserved
- loading, empty, inaccessible, disconnected, refresh, and error states
- restart hydration from SQLite without requiring a source refresh
- lazy artwork loading and a virtualized track list with paginated backend reads

## Active Work

Verify and merge **Milestone 5: v0.1 Desktop Experience**.

The Milestone 3 real Spotify authentication smoke test and Milestone 4 real source-refresh smoke test have both passed on macOS.

## Recent Changes

- added paginated backend projections for the Spotify source overview, playlist list, and collection entries
- added desktop navigation for Liked Songs, Playlists, and Settings
- added dense playlist and Liked Songs track views with lazy artwork
- preserved playlist ordering, duplicate positions, and unavailable entries in the browsing UI
- added explicit inaccessible-playlist and disconnected states
- added restart-state hydration from persisted SQLite source state
- retained manual Spotify refresh and progress/cancellation within the desktop experience
- added virtualized rendering for loaded track rows and incremental page loading
- added Rust projection tests, mocked Tauri IPC tests, and Svelte component coverage for duplicate and empty track-list states
- completed the real Milestone 4 source-refresh smoke test on macOS

## Known Issues

No known implementation defect is currently documented.

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Milestone 5 still needs automated validation and a manual desktop smoke test covering Liked Songs browsing, playlist selection/detail, restart hydration, inaccessible playlists, and refresh behavior.

## Next

After Milestone 5 passes validation and merge, implement **Milestone 6: v0.1 Hardening and Release**.

Milestone 6 covers release metadata and icons, clean first-run and migration checks, user-facing error hardening, log-redaction verification, platform build checks, stable developer/reference documentation, the initial release changelog, and v0.1.0 packaging.

## Blockers

No implementation blocker is currently known.

A connected Spotify account with imported source state is required for the Milestone 5 manual browsing smoke test.

## Open Decisions

No unresolved product or architectural decision blocks Milestone 5 verification or Milestone 6 planning.

The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior and acceptance criteria.
- `docs/TDD.md` defines the technical architecture, paginated frontend data flow, and desktop-view requirements.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and authentication behavior.
