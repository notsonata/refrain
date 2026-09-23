# Project Status

## Current State

Milestones 1 through 3 are merged for the v0.1.0 development line.

The application now includes:

- the Tauri/Svelte/Rust application foundation
- SQLite persistence for v0.1 source state and application settings
- Spotify Client ID configuration
- Authorization Code with PKCE
- a fixed loopback OAuth callback at `http://127.0.0.1:43817/callback`
- OS credential-store persistence for Spotify refresh credentials
- in-memory Spotify access tokens with refresh and reconnect behavior
- connect/disconnect UI and typed frontend command wrappers

Spotify source synchronization has not been implemented yet.

## Active Work

No later milestone is currently active on `main`.

The project is ready for **Milestone 4: Spotify Source Synchronization** from `docs/IMPLEMENTATION.md`.

## Recent Changes

- merged Milestone 3 Spotify authentication
- added persisted Spotify Client ID configuration
- added the initial Rust `SpotifyClient` authentication adapter
- added PKCE and OAuth-state validation
- fixed the Spotify loopback callback to `127.0.0.1:43817` so the runtime URI exactly matches the developer-dashboard registration
- added native secure refresh-credential storage and in-memory access-token caching
- added token refresh, reauthorization, disconnect, and reconnect behavior
- added a focused Spotify connection screen and typed frontend command wrappers
- added mocked authentication coverage and Spotify authentication setup documentation

## Known Issues

No known implementation defect is currently documented.

The real Spotify developer-application authentication smoke test remains a manual verification follow-up because it cannot run in CI.

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

## Next

Implement **Milestone 4: Spotify Source Synchronization**.

Milestone 4 adds:

- current-user profile retrieval
- Liked Songs pagination
- playlist listing and playlist-item pagination
- playlist snapshot optimization
- inaccessible and unavailable playlist-item handling
- collection-level transactional persistence
- manual source refresh
- retry and rate-limit handling
- refresh progress and cancellation where practical

The real Spotify authentication smoke test should also be completed before v0.1.0 release and preferably before or during Milestone 4 integration testing.

## Blockers

None block Milestone 4 implementation.

A user-owned Spotify developer application configured with `http://127.0.0.1:43817/callback` is required for the pending real authentication smoke test.

## Open Decisions

No unresolved product or architectural decision blocks Milestone 4.

The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior.
- `docs/TDD.md` defines the technical architecture and security requirements.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision that supersedes the earlier dynamic-port design.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and the manual authentication smoke test.
