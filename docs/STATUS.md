# Project Status

## Current State

Milestones 1 through 12 are complete. Milestone 13, Sockseek Sidecar Provider, is implemented locally and is awaiting its provider-level mock-daemon verification plus the required real Soulseek smoke test before release packaging. Refrain v0.1.0 was released on 2026-09-25 as the first Spotify-only desktop release, and the v1 development line now includes the observational Local Library Index, deterministic Matching Engine, Reconciliation Core, Filesystem Normalization and Ownership Safety, Issues and Manual Resolution UI, the provider-neutral Acquisition Provider Boundary, and the initial Sockseek production-provider integration.

The application includes:

- the released v0.1 Spotify authentication, synchronization, persisted browsing, desktop UI, packaging, and release automation
- a configurable observational local-library index with metadata extraction, incremental rescans, lazy hashing, file-state tracking, moved-file recovery, and paginated reads
- `library_tracks` and `local_files` persistence from Milestone 7
- `track_links` and `track_rejections` persistence for matching decisions
- deterministic metadata normalization, bounded candidate generation, compatibility rules, scoring, and automatic/review/unresolved match outcomes
- persisted user confirmations and rejections exposed through Tauri commands and typed frontend wrappers
- `sync_runs` persistence with prepare-through-normalization phase tracking
- one-active-sync coordination with cooperative cancellation at synchronization phase boundaries
- source-to-library reconciliation that reuses persisted links, creates stable `LibraryTrack` records, resolves preferred present files, and classifies desired tracks as matched, missing, or needing review
- canonical filesystem normalization for preferred present files, using Spotify metadata when linked and local canonical metadata when unmatched or local-only
- cross-platform filename sanitization, case-insensitive collision handling, stable collision suffixes, case-only rename staging, and verified copy fallback for cross-filesystem moves
- path-boundary validation plus ownership-preserving moves that keep external files external and never automatically delete external duplicates
- Trash / Recycle Bin integration guarded to managed files for future cleanup work
- paginated Library and Issues views backed by durable local-library and reconciliation state
- issue projections for ambiguous matches, missing local files, inaccessible Spotify collections, and invalid local files
- match-review controls for confirming, rejecting, and clearing persisted manual decisions, with resolved issues disappearing from the projection automatically
- durable `acquisition_jobs` persistence with provider-neutral candidate/job types and a fake-provider-tested acquisition coordinator
- bounded acquisition attempts, cooperative provider cancellation, Refrain-controlled staging directories, and a `staged` terminal state that still requires later verification/import
- acquisition status IPC plus failed acquisition jobs projected into the Issues view
- Sockseek `3.0.5` pinned as the first production acquisition provider, with checksum-verified sidecar fetching for supported release targets
- lazy Sockseek daemon startup on an available loopback port, exact version/readiness checks, owned process shutdown, and credential-redacted sidecar logging
- provider-specific HTTP search/download/status/cancel mapping kept behind the opaque acquisition-provider boundary
- SignalR progress/invalidation wakeups with reconnect behavior while durable HTTP job snapshots remain authoritative
- Soulseek credentials stored in the operating system credential store and materialized only into a restricted transient Sockseek configuration during daemon startup
- Refrain-controlled Sockseek staging output that remains in the durable `staged` acquisition state pending Milestone 14 verification/import
- Settings controls for enabling acquisition, saving/clearing Soulseek credentials, and checking Sockseek provider health
- release packaging support for the Sockseek sidecar plus AGPL license/source notices
- native Library root folder selection through the Tauri dialog plugin
- a fixed-height Issues workspace whose queue and detail panes scroll independently instead of clipping at the supported desktop window sizes
- typed sync-run Tauri commands and frontend wrappers for starting, cancelling, reading, and listing synchronization runs
- separate Local and Spotify primary workspaces with contextual sync actions instead of one mixed source/library navigation model
- scoped `local` / `spotify` sync runs and scope-filtered history queries
- persistent Spotify collection tracking defaults plus per-track include/exclude overrides used as Spotify Sync desired state
- Local library rows restricted to present local music and annotated with matched Spotify memberships
- media-row Local/Spotify browsing UI with unavailable playlists moved into a collapsed secondary section and excluded from the actionable Issues queue
- compact desktop-density shell and workspaces with a materially narrower primary sidebar and reduced typography/control spacing
- artwork-first Saved Albums and Playlists grids that open into collection detail views, while Liked Songs remains track-oriented
- persistent collection tracking controls that can be changed directly from the collection grid without forcing navigation into the collection
- default Local/Spotify filters for common metadata/state fields plus technical filters grouped under Advanced
- Local rows with directly visible file paths and embedded artwork when present
- embedded local artwork extraction with content-hash deduplication into the application-data artwork cache, referenced from `local_files` rather than stored as SQLite blobs

Milestones 7 through 9 remain non-destructive with respect to user audio. Milestone 10 may move confidently resolved preferred files into the canonical library structure, but it preserves each file's ownership classification and does not automatically delete external files.

## Active Work

Milestone 13 implementation is complete locally. Static/build validation is being completed against the pinned sidecar. The remaining milestone verification is Sockseek's documented mock-daemon provider flow plus one real Soulseek acquisition smoke test before an acquisition-enabled release is packaged.

The approved v1 desktop-density and library-browsing refinement is implemented locally. Remaining active work is the Milestone 13 provider verification gate described above.

## Recent Changes

- merged Milestone 9 Reconciliation Core with durable sync runs, one-active-sync coordination, stable source-to-library reconciliation, and cancellation-preserved committed state
- added canonical normalized path construction with single-disc and multi-disc filenames plus unknown album/year fallbacks
- added NFKC filesystem sanitization, Windows reserved-name protection, deterministic component shortening, and case-insensitive collision suffixing
- added atomic rename behavior, case-only rename staging, verified cross-filesystem copy-and-move fallback, and safe path-boundary checks
- preserved preferred-file and managed/external ownership state after moves; external files are never automatically deleted
- added managed-file Trash / Recycle Bin integration for future cleanup workflows
- added Milestone 10 filesystem normalization tests covering the implementation-plan verification cases
- merged Milestone 10 Filesystem Normalization and Ownership Safety
- restored deterministic JavaScript dependency installation for CI with `package-lock.json` and `npm ci`
- added dense Library and Issues desktop views with current issue counts, details, and paginated projections
- added durable issue projections for ambiguous matches, missing/invalid local files, and inaccessible Spotify collections without introducing a separate issue table
- added match-review candidate inspection and persisted confirm/reject/clear actions that refresh the issue projection after resolution
- added frontend and Rust coverage for issue projection behavior and automatic issue removal after durable state repair
- manually verified local-library scanning with 478 indexed files: 477 present, 0 missing, and 1 invalid
- added migration 6 for durable acquisition jobs with one durable job per logical library track
- added provider-neutral acquisition types, health checks, staging, bounded retries, cancellation, status projection, and deterministic fake-provider integration coverage
- kept provider success at `staged`, preserving the later verification/import boundary instead of treating a provider download as synced audio
- projected failed acquisition jobs into Issues and repaired the Issues desktop layout so the queue and detail panes remain independently scrollable
- replaced manual Library root entry with the native operating-system folder picker while retaining explicit Save path / Scan library actions
- manually verified the Milestone 12 Issues layout and Library root folder-picker changes on macOS
- pinned Sockseek `3.0.5` and added deterministic platform-sidecar download/extraction with published SHA-256 verification
- added the Sockseek sidecar manager, loopback runtime port selection, version/readiness checks, HTTP acquisition adapter, SignalR wake/reconnect path, and owned process shutdown
- added OS credential-store persistence for Soulseek credentials with restricted transient Sockseek configuration and secret-redacted sidecar logs
- wired acquisition-enabled synchronization through the production Sockseek provider while preserving `staged` as the pre-verification boundary
- added acquisition Settings controls and third-party AGPL/source notices for distributed Sockseek builds
- fixed the Windows Rust CI Clippy failure by platform-gating the Unix-only `std::io::Write` import used for restricted Sockseek runtime configuration writes
- fixed Sockseek compatibility checks so the pinned `3.0.5` release accepts Sockseek's equivalent `3.0.5.0` server version while still rejecting real version differences
- corrected Sockseek health semantics for v3.0.5: an idle daemon with Soulseek state `None` is ready because the Soulseek client is created and logged in lazily on the first acquisition job; Settings now reflects this and shows configured accounts in green
- clarified acquisition Settings with separate account/configuration and live Sockseek connection chips, plus an explanation that missing-track downloads remain staged for later verification/import
- replaced Sockseek's unhelpful `Soulseek is not ready (None)` health text with a clearer not-ready message when no meaningful client state is reported
- added a visible Run synchronization control in the app header and empty Library state, refreshed Library/Issues/source projections after a run, and decoupled Library/Settings/Issues navigation from Spotify hydration failures
- clarified that local-library scanning indexes physical files while synchronization builds the logical Library shown in the main Library view
- fixed duplicate-key loading skeletons in Library and Issues that could crash Svelte rendering while those views reloaded
- released Refrain v0.1.0 on 2026-09-25

- extended filesystem normalization to organize unmatched and local-only preferred files using scanned local metadata when no accessible Spotify link exists
- split synchronization into Local Sync and Spotify Sync, with Local Sync remaining local-first and Spotify Sync materializing only persisted tracked selections
- added durable source-tracking rules, per-track overrides, scoped sync metadata, and acquisition filtering so stale untracked queued jobs cannot run
- replaced the old Library/Liked Songs/Saved Albums/Playlists primary navigation with Local and Spotify workspaces and removed the spreadsheet-style track tables from those flows
- approved the follow-up desktop-density/library-browser design: compact shell and rows, collection grids for albums/playlists, expanded filters with an Advanced technical group, visible local paths, and embedded local artwork when available
- implemented the desktop-density/library-browser pass across the shell, Local, Spotify, Issues, and Settings views, including a narrower primary sidebar and denser controls
- replaced Saved Albums and Playlists master-detail browsing with artwork-first collection grids and explicit grid-to-detail/back navigation
- added default metadata/state filters and Advanced technical filters to dense Local and Spotify track views
- added visible Local file paths plus embedded-cover extraction, hashed application-data artwork caching, and scoped Tauri asset serving
- added migration `0008_local_artwork.sql` for cached artwork references and MIME metadata on `local_files`

## Known Issues

Port `43817` must be available while starting Spotify authorization. Refrain reports an authentication error rather than choosing a different port when it is occupied.

Unsigned or ad-hoc-signed release packages may require platform security confirmation. Production signing/notarization depends on release credentials being available.

Milestone 13 has not yet completed the documented Sockseek mock-daemon integration verification or the real Soulseek download smoke test. Do not treat the provider as release-verified until those checks pass.

## Next

Complete the Milestone 13 Sockseek provider verification using the documented mock daemon and one real Soulseek download smoke test. After the provider passes that gate, proceed to **Milestone 14: Acquisition Verification and Import**.

## Blockers

No implementation blocker is currently documented.

Public code signing and macOS notarization remain credential-dependent and are not required for local unsigned development builds.

## Open Decisions

The saved-album source-model decision is recorded in ADR 002. The deferred technical questions in `docs/TDD.md` remain deferred until their affected implementation areas begin.

## Relevant Context

- `docs/BRIEF.md` defines project purpose and release boundaries.
- `docs/SPEC.md` defines v0.1.0 and v1.0.0 behavior and acceptance criteria.
- `docs/TDD.md` defines the technical architecture, local-library scanner, matching engine, reconciliation engine, and desktop data flow.
- `docs/IMPLEMENTATION.md` defines milestone order and verification gates.
- `docs/reference/codebase-map.md` maps the current source structure.
- `docs/reference/setup.md` documents local setup and library-root configuration.
- `docs/reference/testing.md` documents validation coverage, including local-library, matcher, reconciliation, filesystem-normalization, and issue/manual-resolution tests.
- `docs/reference/release.md` documents tag-driven release packaging.
- `docs/decisions/001-fixed-spotify-callback-port.md` records the fixed callback-port decision.
- `docs/decisions/002-saved-albums-as-source-collections.md` records the saved-album persistence and identity model.
- `docs/reference/spotify-auth.md` documents Spotify developer setup and authentication behavior.
