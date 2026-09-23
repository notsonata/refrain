# Project Status

## Current State

Milestones 1 and 2 are merged for the v0.1.0 development line. Milestone 3 is implemented on the active Spotify-authentication branch and is awaiting verification and merge.

The application foundation now includes SQLite source-state persistence. The Milestone 3 branch adds Spotify Client ID configuration, Authorization Code with PKCE, a dynamic loopback callback on `127.0.0.1`, OS credential-store persistence for refresh credentials, in-memory access tokens, token refresh behavior, and connect/disconnect UI.

Spotify source synchronization remains out of scope until Milestone 4.

## Active Work

Verify and merge **Milestone 3: Spotify Authentication** from `docs/IMPLEMENTATION.md`.

## Recent Changes

- added persisted Spotify Client ID configuration
- added the initial Rust `SpotifyClient` authentication adapter
- added PKCE and OAuth-state validation with a dynamic loopback callback
- added native secure refresh-credential storage and in-memory access-token caching
- added refresh, reauthorization, disconnect, and reconnect behavior
- added a focused Spotify connection screen and typed frontend command wrappers
- added mocked authentication coverage and a manual smoke-test reference

## Known Issues

No known implementation defect is currently documented.

The required real Spotify developer-application authentication smoke test cannot run in CI and remains pending until performed manually.

## Next

After Milestone 3 passes automated validation and the manual authentication smoke test, implement **Milestone 4: Spotify Source Synchronization**.

Milestone 4 adds current-user profile retrieval, Liked Songs and playlist pagination, transactional source refresh, retry/rate-limit behavior, progress, and cancellation where practical.

## Blockers

None for automated Milestone 3 validation.

A user-owned Spotify developer application is required for the Milestone 3 manual smoke test.

## Open Decisions

No unresolved product or architectural decision blocks the current milestone.

The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior.
- `docs/TDD.md` defines the technical architecture and security requirements.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and the manual authentication smoke test.
