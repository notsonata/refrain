# Refrain Implementation Plan

## Purpose

This document turns the approved product and technical design into an ordered implementation sequence.

Use these documents for detail rather than duplicating them here:

- `docs/BRIEF.md` for project purpose and boundaries
- `docs/SPEC.md` for product behavior and acceptance criteria
- `docs/TDD.md` for architecture, schema, algorithms, interfaces, security, and testing strategy

This plan records the implementation sequence from the initial project foundation through v0.1.0 and the planned path to v1.0.0.

## Current State

Refrain v0.1.0 was released on 2026-09-25. Milestones 1 through 6 are complete.

The repository now contains the Tauri 2 desktop application, Svelte 5 frontend, Rust backend, SQLite persistence, Spotify Authorization Code with PKCE, Spotify source synchronization and browsing, automated validation, native bundle configuration, and tag-triggered release packaging.

Milestone 7, Local Library Index, is the next implementation milestone. Local-library indexing, matching, reconciliation, acquisition, normalization, exports, and scheduling remain future v1 work.

The approved v1 direction remains:

- Rust application core with SQLite persistence
- direct Spotify integration using Authorization Code with PKCE
- provider-neutral acquisition layer with Sockseek as the initial provider
- normalized local library owned and reconciled by Refrain

## Delivery Principles

Implementation should follow these rules throughout the plan:

- complete one usable vertical slice before adding the next major subsystem
- keep v0.1.0 free of local-library and acquisition functionality
- keep provider-specific behavior behind adapters
- preserve user data before optimizing automation
- add migrations alongside schema changes
- add tests with the behavior they protect rather than deferring all testing
- avoid placeholder abstractions for future providers, sources, mobile clients, or cloud targets beyond the interfaces already required by the TDD
- keep the frontend dependent on use-case-oriented Tauri commands, not database or provider details
- keep each milestone mergeable and independently verifiable

## Release Sequence

```text
Foundation
    ↓
Spotify persistence and authentication
    ↓
Spotify source sync
    ↓
v0.1 desktop UI
    ↓
v0.1 hardening and release
    ↓
Local library indexing
    ↓
Matching and reconciliation
    ↓
Normalization and safe file ownership
    ↓
Issues and manual resolution
    ↓
Acquisition provider + Sockseek
    ↓
Full sync orchestration
    ↓
Exports
    ↓
Library mirroring
    ↓
Scheduling and removal policy
    ↓
v1 hardening and release
```

---

# v0.1.0

## Milestone 1: Application Foundation

### Objective

Create the smallest working cross-platform Tauri application with the agreed frontend and Rust backend structure.

### Work

- scaffold Tauri 2 with Svelte 5, TypeScript, and Vite
- configure Tailwind CSS 4
- establish the root application shell
- create the initial Rust module layout only for modules needed by v0.1
- enable TypeScript strict mode
- establish formatting, linting, Rust formatting, and clippy configuration
- add a minimal typed Tauri command round trip
- establish application-data path resolution
- establish structured Rust logging
- add restrictive initial Tauri capabilities
- add test harnesses for Rust and frontend code
- add CI for static checks, tests, and production builds that can run before native release packaging is complete

Do not add Sockseek, local-library scanning, matching, or mirror code in this milestone.

### Dependencies

None.

### Verification

- application launches in development mode
- frontend can invoke one typed Rust command
- production frontend build succeeds
- Rust formatting, clippy, and tests pass
- TypeScript checks and frontend tests pass
- CI runs the same baseline validation

### Exit Criteria

The repository has a working desktop application skeleton and a stable development workflow without implementing product behavior prematurely.

---

## Milestone 2: Persistence Foundation

### Objective

Establish SQLite, migrations, configuration persistence, and the v0.1 source-domain subset.

### Work

Implement only the schema required by v0.1 plus shared migration infrastructure.

Initial tables:

- `app_settings`
- `source_accounts`
- `source_collections`
- `source_tracks`
- `collection_entries`

Also:

- initialize SQLite in the application-data directory
- enable foreign keys, WAL, and busy timeout
- run migrations at application startup
- create repository/data-access functions for source state
- add transaction helpers for collection replacement
- use UTC Unix milliseconds consistently
- expose only use-case-oriented backend commands to the frontend

Do not create the complete v1 schema merely because it is already designed. Add v1 tables when their owning milestone begins.

### Dependencies

Milestone 1.

### Verification

- empty database migrates successfully
- migrations are repeatable on restart
- foreign-key and uniqueness constraints behave as designed
- source collection replacement is atomic
- failed transactions preserve previous state
- application restarts with persisted settings and source data intact

### Exit Criteria

Refrain has durable local state suitable for Spotify synchronization.

---

## Milestone 3: Spotify Authentication

### Objective

Allow the user to connect their own Spotify application and maintain authenticated access securely.

### Work

- implement Spotify configuration UI for Client ID
- implement Authorization Code with PKCE
- create and validate OAuth state
- run a loopback callback listener on `127.0.0.1`
- use a dynamic callback port
- open authorization in the system browser
- exchange the authorization code
- store refresh credentials using the OS credential store
- keep access tokens in memory
- implement refresh-on-expiry behavior
- implement disconnect/reconnect behavior
- implement the initial `SpotifyClient` adapter
- add structured authentication errors

### Dependencies

Milestones 1 and 2.

### Verification

Use mocked Spotify HTTP flows for automated tests covering:

- PKCE exchange
- state mismatch
- token refresh
- cancelled authorization
- invalid Client ID
- credential-store failure
- disconnect and reconnect

Perform one documented manual authentication smoke test with a real Spotify developer application before declaring the milestone complete.

### Exit Criteria

A user can connect Spotify without Refrain shipping a secret and can reopen the application without repeating the full login flow while credentials remain valid.

---

## Milestone 4: Spotify Source Synchronization

### Objective

Fetch and persist the complete Spotify source state required by v0.1.

### Work

Implement:

- current-user profile retrieval
- Liked Songs pagination
- playlist listing
- playlist-item pagination
- playlist `snapshot_id` optimization
- inaccessible playlist handling
- unavailable/removed playlist-item preservation
- collection-level transactional updates
- manual source refresh
- source refresh progress events
- bounded retry behavior for transient failures
- 401 refresh behavior
- 429 `Retry-After` handling
- cancellation support where practical

Source adapters must map Spotify DTOs into Refrain domain objects before persistence.

Do not use Sockseek for Spotify ingestion.

### Dependencies

Milestones 2 and 3.

### Verification

Contract tests should cover:

- pagination
- changed and unchanged playlist snapshots
- transactional replacement
- Liked Songs refresh
- inaccessible followed playlists
- null or unavailable playlist entries
- stale data preservation after failed network refresh
- rate limiting
- token refresh during sync

### Exit Criteria

A successful manual refresh leaves SQLite with a complete, internally consistent representation of the accessible Spotify collections.

---

## Milestone 5: v0.1 Desktop Experience

### Objective

Make the Spotify source state usable from the desktop application.

### Work

Implement the v0.1 navigation and views:

- Liked Songs
- Playlists
- Settings

Support:

- playlist list
- playlist detail
- ordered track rows
- intentional duplicate entries
- loading states
- empty states
- authentication-required state
- inaccessible playlist state
- refresh state and errors
- manual refresh action
- persistence across restart
- lazy album artwork
- table/list virtualization where needed for large collections

Keep the UI dense and desktop-oriented.

Do not expose nonfunctional Downloads, Library, Issues, or Mirror sections as if they were available.

### Dependencies

Milestones 3 and 4.

### Verification

- frontend component tests cover primary states
- mocked Tauri IPC flow tests cover connect, refresh, browse, restart-state hydration, and errors
- large playlist rendering remains responsive
- manual smoke test verifies Liked Songs and playlist browsing

### Exit Criteria

The v0.1 product acceptance criteria in `docs/SPEC.md` are satisfied.

---

## Milestone 6: v0.1 Hardening and Release

### Objective

Turn the v0.1 feature-complete build into a reproducible desktop release.

### Work

- finalize application metadata and icons
- verify clean first-run behavior
- verify database migration from no database
- improve user-facing errors for authentication and source refresh
- verify secret redaction in logs
- add build smoke checks for Windows, macOS, and Linux
- document developer setup only where actual setup steps now exist
- update the codebase map once the source structure is stable
- add `CHANGELOG.md` when preparing the release
- create release packages appropriate to the supported platforms

Signing and notarization should be used for public release where credentials are available, but local unsigned development builds must remain possible.

### Dependencies

Milestones 1 through 5.

### Verification

Run the complete v0.1 validation set:

- frontend format/lint/type checks
- frontend tests
- Rust format/clippy/tests
- production frontend build
- Tauri build smoke checks
- clean-install smoke test
- Spotify connect and refresh smoke test
- restart persistence smoke test

### Exit Criteria

v0.1.0 is releasable and usable as a Spotify-only desktop application.

---

# v1.0.0

## Milestone 7: Local Library Index

### Objective

Represent the user's existing audio collection without yet changing files.

### Work

Add the relevant v1 schema and domain types:

- `library_tracks`
- `local_files`

Implement:

- library-root setting
- recursive scanner
- supported audio-format filtering
- metadata extraction with `lofty`
- incremental scan using size and modification time
- lazy BLAKE3 hashing
- present/missing/invalid local-file state
- preferred-file selection primitives
- moved/renamed-file recovery
- scan progress events
- paginated library queries

At this stage scanning should be observational. Do not normalize or move user files yet.

### Dependencies

v0.1 foundation and persistence infrastructure.

### Verification

Filesystem integration tests cover:

- new files
- unchanged files
- changed files
- removed files
- moved files
- malformed audio
- unsupported extensions
- symlink behavior
- multiple files representing the same apparent recording
- large-directory incremental rescans

### Exit Criteria

Refrain can build and maintain an accurate index of the configured local library without modifying it.

---

## Milestone 8: Matching Engine

### Objective

Resolve source tracks to logical library tracks conservatively and deterministically.

### Work

Add:

- `track_links`
- `track_rejections`

Implement:

- metadata normalization
- version parsing
- candidate indexing
- hard incompatibility rules
- ISRC-based strong matching
- duration compatibility
- weighted metadata scoring
- runner-up margin
- automatic/review/unresolved outcomes
- persisted user confirmations and rejections
- reusable matching evidence for diagnostics and UI

Create a representative matcher fixture corpus before tuning behavior.

### Dependencies

Milestones 4 and 7.

### Verification

Unit tests must cover every matching case listed in the TDD, including:

- album vs single
- compilation vs album
- live vs studio
- remix
- acoustic
- demo
- explicit vs clean
- remaster
- duration drift
- conflicting ISRC
- weak metadata
- runner-up ambiguity
- manual confirmation and rejection

### Exit Criteria

The matcher can safely classify representative real-world cases without requiring filesystem mutation or acquisition.

---

## Milestone 9: Reconciliation Core

### Objective

Create the durable relationship between desired Spotify state and actual local state.

### Work

Implement:

- creation of missing `LibraryTrack` records
- stable canonical metadata
- reuse of valid persisted links
- source-to-library reconciliation
- preferred local-file resolution
- missing-track detection
- sync-run persistence
- initial sync phases through matching
- one-active-sync coordination
- idempotency protections
- cooperative cancellation

A sync at this stage may stop after classifying tracks as matched, missing, or needing review.

### Dependencies

Milestones 7 and 8.

### Verification

Integration tests prove:

- repeated syncs do not create duplicate library tracks
- one library track can serve many source tracks and playlist entries
- intentional duplicate playlist positions remain separate
- manual decisions persist across syncs
- removal from one playlist does not affect references from others
- cancellation preserves already committed valid state

### Exit Criteria

Refrain can produce a stable reconciliation result without downloading or moving files.

---

## Milestone 10: Filesystem Normalization and Ownership Safety

### Objective

Normalize confidently resolved library files while preserving ownership and deletion safety.

### Work

Implement:

- canonical path builder
- cross-platform filename sanitizer
- case-insensitive collision detection
- stable collision suffixing
- single-disc and multi-disc path formats
- atomic same-filesystem moves
- verified copy-and-move fallback
- managed vs external ownership persistence
- normalization of confidently matched existing files
- preferred-file updates after moves
- safe path-boundary validation
- Trash / Recycle Bin integration for future managed cleanup

Do not automatically delete external duplicate files.

### Dependencies

Milestones 7 through 9.

### Verification

Filesystem tests cover:

- Windows-invalid characters
- reserved device names
- Unicode normalization
- long components
- path collisions
- case-only collisions
- multi-disc tracks
- unknown year/album
- atomic move failure
- cross-filesystem copy failure
- external ownership preservation
- attempted path traversal

### Exit Criteria

Refrain can safely place resolved audio into the canonical normalized structure without losing ownership information.

---

## Milestone 11: Issues and Manual Resolution UI

### Objective

Make unresolved state visible and fixable before introducing automatic acquisition.

### Work

Add the v1 views:

- Library
- Issues

Implement issue projections for:

- ambiguous matches
- missing local files
- inaccessible Spotify collections
- invalid local files

Implement match review:

- inspect source metadata
- inspect candidate local tracks/files
- confirm match
- reject match
- clear a manual decision

Issues should disappear automatically when the underlying condition is resolved.

### Dependencies

Milestones 8 through 10.

### Verification

- frontend tests cover issue states and match decisions
- integration tests verify decisions persist and affect the next reconciliation
- resolving a condition removes the issue without a separate issue-delete workflow

### Exit Criteria

The user can understand and resolve matching ambiguity without editing the database or filesystem manually.

---

## Milestone 12: Acquisition Provider Boundary

### Objective

Introduce missing-track acquisition without coupling core sync logic to Sockseek.

### Work

Add:

- `acquisition_jobs`
- provider-neutral acquisition types
- `AcquisitionProvider` interface
- acquisition coordinator
- staging-directory lifecycle
- bounded retries
- provider health state
- acquisition status projections

Create a fake provider for deterministic integration testing before implementing Sockseek.

### Dependencies

Milestone 9.

### Verification

With the fake provider, prove:

- missing tracks create one acquisition job
- duplicate source references do not create duplicate jobs
- retries use distinct candidates or retryable failures
- cancellation works
- a successful provider job does not bypass Refrain verification
- failed jobs become visible issues

### Exit Criteria

Core acquisition orchestration works without any Sockseek-specific logic.

---

## Milestone 13: Sockseek Sidecar Provider

### Objective

Make Sockseek the first production acquisition provider.

### Work

- select and pin a tested Sockseek release
- bundle platform-specific sidecar binaries where available
- implement sidecar startup and shutdown
- bind daemon to loopback on an available port
- implement health/version compatibility checks
- implement HTTP API adapter
- implement SignalR progress subscription
- use durable HTTP state as authoritative after event disconnects
- configure Refrain-controlled staging output
- add secure Soulseek credential storage
- materialize restricted runtime configuration only if required by the pinned Sockseek version
- redact sidecar logs
- add required AGPL license/source notices for distributed builds

Refrain must not use Sockseek's Spotify input support.

### Dependencies

Milestone 12.

### Verification

Use Sockseek's documented mock daemon mode for automated integration tests covering:

- startup
- version mismatch
- job creation
- job polling
- progress events
- SignalR reconnect/fallback
- cancellation
- successful staging output
- provider failure
- shutdown

Perform a manual Soulseek download smoke test before release packaging.

### Exit Criteria

Refrain can reliably acquire a candidate file through the pinned Sockseek sidecar while keeping provider details isolated.

---

## Milestone 14: Acquisition Verification and Import

### Objective

Turn provider output into safe canonical library content.

### Work

Implement the post-download pipeline:

1. inspect staged files
2. read actual metadata and duration
3. match the downloaded candidate against the requested library track
4. reject incompatible files
5. accept automatic-confidence matches
6. allow manual confirmation where appropriate
7. normalize accepted files into the canonical library
8. create/update the managed `LocalFile`
9. clean or retain staging data according to result
10. surface failed verification as an issue

### Dependencies

Milestones 8, 10, 12, and 13.

### Verification

Tests prove:

- wrong live/remix/version files are rejected
- duration conflicts are rejected
- accepted candidates enter canonical storage
- rejected candidates never become preferred library files
- retry does not reacquire the same rejected candidate indefinitely
- imported files are marked `managed`

### Exit Criteria

Provider success can safely produce a verified, normalized local file without weakening the matcher.

---

## Milestone 15: Full Synchronization Pipeline

### Objective

Connect source refresh, local scan, matching, acquisition, verification, normalization, and playlist resolution into one coherent operation.

### Work

Complete the sync phases defined in the TDD.

Add:

- full `sync_runs` phase tracking
- progress events
- error aggregation
- partial-success status
- missing-track acquisition queue
- post-acquisition reconciliation
- cancellation across provider work
- library mutation locking
- final unresolved counts
- recent sync summaries in the UI

### Dependencies

Milestones 7 through 14.

### Verification

End-to-end backend integration tests cover:

- existing fully matched library
- partially missing library
- ambiguous files
- successful acquisition
- acquisition failure
- provider unavailable
- cancellation
- repeated no-change sync
- source changes between runs

### Exit Criteria

A manual sync can execute the complete v1 reconciliation pipeline safely and idempotently.

---

## Milestone 16: Playlist Export

### Objective

Export fully resolved local playlists.

### Work

Add:

- `playlist_exports`
- `playlist_export_entries`

Implement:

- export precondition checks
- immutable export snapshots
- UTF-8 M3U8 generation
- relative-path preference
- Windows cross-volume absolute-path fallback
- source-order preservation
- intentional duplicate-entry preservation
- atomic file replacement
- portable ZIP bundle creation
- deduplicated audio copies inside bundles
- export history/status UI

Do not create partial exports when supported playlist entries remain unresolved.

### Dependencies

Milestone 15.

### Verification

Tests cover:

- normal M3U8
- duplicate playlist entries
- relative paths
- Windows cross-volume path behavior
- unresolved-entry refusal
- interrupted write
- portable ZIP structure
- one copied audio file referenced by repeated M3U entries

### Exit Criteria

Both export modes meet the product acceptance criteria.

---

## Milestone 17: Library Mirroring

### Objective

Mirror the normalized local library and resolved playlists to another filesystem location without treating that destination as disposable storage.

### Work

Add:

- `mirror_targets`
- `mirror_runs`
- `mirror_entries`

Implement:

- target configuration
- target availability checks
- desired mirror manifest generation
- incremental copy
- temporary-file copy/verify/replace
- relative mirrored playlist paths
- managed-path cleanup
- manifest-last commit behavior
- disconnected-target handling
- mirror progress and result UI

### Dependencies

Milestones 10, 15, and 16.

### Verification

Tests must prove:

- initial mirror succeeds
- unchanged second mirror is effectively a no-op
- changed audio is updated
- stale Refrain-managed files are removed
- unrelated destination files survive
- interrupted copies do not replace good files with partial files
- manifest is written only after successful content operations
- mount-path changes do not break relative playlist references

### Exit Criteria

A mounted external filesystem can contain a portable, incrementally maintained copy of the Refrain library and playlists.

---

## Milestone 18: Scheduling and Removed-Track Policy

### Objective

Complete the automatic synchronization modes and configured cleanup behavior.

### Work

Implement:

- sync on application startup
- configurable periodic sync
- coalescing scheduled triggers while a sync is active
- scheduler reconfiguration after settings changes
- keep-removed-managed-files setting
- detection of library tracks no longer referenced by managed source collections
- managed-file cleanup through Trash / Recycle Bin
- cleanup issue when trashing fails
- explicit protection of external files

Do not add an OS background service.

Scheduled sync runs only while the Refrain application is running.

### Dependencies

Milestone 15.

Safe removal also depends on Milestone 10 ownership tracking.

### Verification

Tests cover:

- startup trigger
- periodic trigger
- no overlapping sync
- scheduled tick coalescing
- keep setting enabled
- keep setting disabled
- track still referenced by another playlist
- managed unreferenced file
- external unreferenced file
- trash failure

### Exit Criteria

All synchronization modes and removal behavior defined for v1 are implemented safely.

---

## Milestone 19: v1 UX Completion

### Objective

Present the complete pipeline clearly in the desktop application.

### Work

Complete the primary navigation:

- Library
- Liked Songs
- Playlists
- Downloads
- Issues
- Settings

Add user-facing state for:

- current sync
- recent sync result
- queued/downloading tracks
- missing tracks
- needs review
- failed acquisition
- inaccessible playlists
- exports
- mirror targets and progress
- provider unavailable
- library root unavailable

Ensure actions are disabled or explained when their prerequisites are not met.

### Dependencies

All v1 feature milestones.

### Verification

Playwright with mocked Tauri IPC covers the major user journeys:

- first library scan
- manual sync
- review ambiguous match
- missing-track acquisition
- failed acquisition
- M3U8 export
- bundle export
- mirror run
- settings changes
- scheduled-sync state presentation

### Exit Criteria

A user can operate the complete v1 workflow without requiring logs, database access, or command-line tools.

---

## Milestone 20: v1 Hardening and Release

### Objective

Verify the complete application across supported platforms and prepare a distributable v1.0.0.

### Work

- run the full migration suite
- verify upgrades from the v0.1 database
- verify clean installs
- verify large-library behavior
- verify interruption recovery
- verify credential-store behavior on supported OSes
- finalize platform-specific Sockseek packaging
- verify Sockseek license/source distribution obligations
- finalize Windows packaging
- finalize macOS signing/notarization workflow where credentials exist
- select and verify Linux package formats
- finalize third-party notices
- update setup/testing/reference documentation based on the actual repository
- update codebase map
- update changelog and release notes

### Dependencies

Milestones 7 through 19.

### Verification

Required release validation:

- frontend lint/type/test suite
- Rust format/clippy/test suite
- database migration suite
- Spotify contract suite
- matcher corpus
- filesystem integration suite
- Sockseek mock integration suite
- mirror safety suite
- production frontend build
- native build/package checks for Windows, macOS, and Linux
- manual real Spotify smoke test
- manual real Sockseek/Soulseek smoke test
- manual export smoke test
- manual external-drive mirror smoke test

### Exit Criteria

Every v1.0.0 acceptance criterion in `docs/SPEC.md` is verified or an explicit release blocker remains open.

---

# Ordering Constraints

The following dependencies should not be bypassed:

1. Spotify persistence must exist before source synchronization.
2. Local scanning must exist before matching against existing files.
3. Matching must be stable before filesystem normalization.
4. Ownership tracking must exist before any automatic cleanup.
5. The provider-neutral acquisition layer must exist before Sockseek integration.
6. Download verification must use the same matcher before acquired files enter the canonical library.
7. Full sync orchestration should be assembled only after its individual phases are independently testable.
8. Playlist export depends on resolved preferred local files.
9. Mirroring depends on canonical normalized paths and resolved playlists.
10. Scheduled sync should be added after manual full sync is reliable.

## Parallel Work Opportunities

Some work can proceed in parallel once the relevant contracts are stable:

- frontend view implementation can proceed alongside backend source-query commands
- matcher fixture creation can proceed alongside local scanner work
- path-sanitizer tests can proceed alongside matching work
- fake acquisition-provider tests can proceed before Sockseek packaging
- export UI can proceed while export backend logic is being finalized
- platform packaging investigation can begin before v1 feature completion

Avoid parallel work that changes the same domain contracts before they have settled.

# Verification Gates

## Gate A: Foundation Ready

Required before product implementation accelerates:

- application launches
- CI works
- SQLite migrations work
- typed frontend/backend command path works

## Gate B: v0.1 Feature Complete

Required before release hardening:

- Spotify authentication works
- Liked Songs and playlists persist
- manual refresh works
- UI can browse persisted source state

## Gate C: Local Reconciliation Ready

Required before acquisition:

- scanner is reliable
- matching corpus passes
- reconciliation is idempotent
- user match decisions persist
- file ownership is explicit

## Gate D: Acquisition Safe

Required before enabling automatic downloads by default:

- provider interface tests pass
- Sockseek adapter tests pass
- acquired-file verification is enforced
- rejected downloads cannot enter canonical storage

## Gate E: v1 Feature Complete

Required before release hardening:

- full manual sync works
- exports work
- mirror works
- scheduling works
- removal policy is safe
- all major UI workflows are usable

# Known Risks

## Spotify API Changes

Spotify has changed its API surface and access rules recently.

Mitigation:

- isolate all Spotify behavior behind one adapter
- use contract tests
- re-verify current Spotify documentation when implementing or upgrading the adapter
- do not spread raw Spotify DTOs through the application

## Sockseek API Instability

Sockseek marks its daemon API experimental.

Mitigation:

- pin a tested Sockseek version
- isolate its API inside `SockseekProvider`
- test against the pinned mock daemon
- keep HTTP durable state authoritative
- do not persist Sockseek DTOs as core domain schema

## Cross-Platform Filesystem Differences

Filename legality, case sensitivity, volumes, trash behavior, and locking differ by operating system.

Mitigation:

- sanitize for the strictest supported filesystem
- test case-insensitive collisions explicitly
- use root-bound path validation
- keep copy/move operations transactional where possible
- test Windows, macOS, and Linux before release

## Matching False Positives

Incorrect matching can place the wrong recording into the library.

Mitigation:

- conservative thresholds
- hard version conflicts
- runner-up margin
- persisted manual decisions
- dedicated fixture corpus
- reuse the same matcher for acquisition verification

## Destructive File Operations

Normalization, cleanup, and mirroring can affect user files.

Mitigation:

- explicit ownership state
- no automatic deletion of external files
- Trash / Recycle Bin for managed cleanup
- mirror manifest ownership
- temporary copy + verification + atomic replacement
- tests specifically proving unrelated files survive

## Linux Credential Store Variability

Desktop environments differ in Secret Service/keyring availability.

Mitigation:

- test the selected keyring backend on supported Linux environments
- fail securely and visibly if no secure store is available
- do not silently fall back to plaintext SQLite storage

## Sidecar Packaging

Sockseek binaries and runtime dependencies may differ by target platform.

Mitigation:

- establish the supported release-target matrix before v1 release
- treat missing compatible sidecar builds as a release blocker for that platform
- keep development capable of using test/mock provider implementations

# Issue Creation Strategy

After this plan is accepted, create GitHub issues from milestones in implementation order.

Issues should be smaller than milestones and should:

- reference the relevant milestone in this document
- reference `docs/SPEC.md` or `docs/TDD.md` rather than copying large sections
- have one clear objective
- include concrete acceptance criteria
- avoid mixing unrelated cleanup with feature work

Do not create all v1 issues before v0.1 implementation begins unless they are needed for dependency tracking.

# Completion Definition

The implementation plan is complete when work can proceed milestone by milestone without rediscovering the project sequence or inventing a major dependency.

Product behavior remains authoritative in `docs/SPEC.md`.

Technical behavior remains authoritative in `docs/TDD.md`.

If implementation reveals a consequential conflict with either document, resolve the underlying decision in the owning document before continuing with a contradictory implementation.
