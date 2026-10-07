# Testing

## Validation layers

Use the lowest validation level that gives credible confidence for the change.

### Frontend

CI installs JavaScript dependencies from the committed `package-lock.json` with `npm ci` so validation uses the exact dependency graph recorded by the repository.

```bash
npm run format:check
npm run lint
npm run check
npm test
npm run build
```

Frontend tests use Vitest. User-visible desktop behavior should be verified in the native Tauri application when IPC, windowing, credential storage, or platform behavior matters.

Frontend coverage includes Liked Songs and Local Library search/filter pagination behavior plus Saved Album/Playlist pin identity, persistence helpers, pinned-first ordering, card pin state, and context-menu wiring. Search/filter tests should confirm that active criteria continue pagination until all available rows can participate in the result set rather than stopping at the initially loaded page. Pinning tests should confirm that pinned cards move ahead of unpinned cards while preserving pin order and that the rendered badge follows the current pin keys without requiring a page reload.

Local Playlist frontend coverage verifies the browser/detail workflow, playlist search, ordered track rendering, managed/linked M3U8 status, automatic-sync copy, empty-playlist guidance, editable local-playlist actions, and read-only Spotify-mirror labeling/artwork. Local Songs continues to own track search/selection, with selected or individual local tracks routed only into editable user-authored playlists.

Long-list coverage includes a Local Library regression that renders a 200-track projection and confirms the desktop table emits only its virtual viewport instead of every loaded row. Frontend performance-sensitive changes should also retain the shared overflow-menu/listener behavior, avoid unconditional state replacement from idle Staging polls, and keep ordinary navigation on cached projections unless the underlying state was explicitly invalidated.

### Rust backend

```bash
npm run rust:fmt
npm run rust:clippy
npm run rust:test
```

Rust tests cover persistence, migrations, Spotify authentication, source synchronization, local-library scanning, and matching behavior. The database suite includes opening a previously absent SQLite database, applying migrations, configuring SQLite pragmas, reopening persisted state, transactional source replacement, local-file pagination, and preferred-file selection.

Local Playlist backend tests cover migration creation, CRUD, duplicate membership, stable order, reordering/removal, automatic managed M3U8 creation, managed filename changes on rename, superseded managed-file removal, imported external-target preservation, automatic rewrite after membership changes, ordered extended-M3U generation, relative paths, destination preservation when an entry has no usable local file, and absolute-target validation. Spotify-mirror coverage verifies tracked playlist and Liked Songs mirror creation from collection-level tracking, source artwork projection where available, managed cover-sidecar writing/state, source order/duplicate preservation, automatic membership refresh, per-track exclusion handling, removal after untracking, direct-edit rejection, and reconciliation integration that materializes and automatically synchronizes mirrors alongside newly tracked missing tracks. Saved Albums are excluded from Local Playlist mirroring. Playlist-artwork tests cover supported image-format detection without requiring network access.

Spotify PKCE callback tests require permission to bind a loopback listener. In restricted sandboxes that deny local socket binding, those tests stop at `callbackListenerFailed` with `Operation not permitted`; run them on a normal development host or CI environment before treating the full Rust suite as green.

### Local library scanner

Filesystem integration tests use temporary directories and cover:

- new, unchanged, changed, removed, and moved audio files
- stale BLAKE3 hash invalidation after content changes
- hash-assisted moved-file recovery without confusing copied duplicates for moves
- malformed supported audio and unsupported extensions
- directory symlinks remaining untraversed
- multiple physical files for the same apparent recording
- large-directory incremental rescans and paginated reads

Scanner tests must remain observational: they may create/change files only inside their temporary fixtures and must not normalize, move, or delete user library files.

### Matching engine

Matcher tests use a representative fixture corpus plus focused persistence tests. Coverage includes:

- album vs single and compilation vs album identity
- live, remix, acoustic, demo, instrumental, explicit/clean, and remaster handling
- compatible and incompatible duration evidence
- exact and conflicting ISRCs, including duplicate-ISRC ambiguity
- weak metadata and runner-up ambiguity
- bounded fuzzy-title candidate fallback
- persisted manual confirmations, rejections, and decision clearing

The matcher tests do not mutate user files or perform acquisition. The fixture corpus lives in `src-tauri/tests/fixtures/matcher_cases.json`.

### Reconciliation and full synchronization

Milestone 9 reconciliation coverage remains in place for stable identity, duplicate collection positions, manual decisions, cross-collection references, cancellation, and typed frontend command wrappers. Milestone 15 adds full-pipeline coverage across the adjacent reconciliation, acquisition, matching, and verification modules for:

- fully matched and partially missing libraries
- ambiguous matching outcomes
- post-acquisition reconciliation so newly present audio clears the final missing count
- successful acquisition and verified managed import
- acquisition failure producing a partial sync result
- unavailable higher-priority providers being skipped while the provider chain continues
- cancellation preserving already committed valid state
- repeated no-change reconciliation remaining idempotent
- source membership added/removed deltas persisting into `sync_runs`
- deduplicated warning aggregation and final acquisition-failure counts

The desktop Sync Library action intentionally remains two scoped runs: Local Sync first, followed by Spotify Sync when Local succeeds. Live `library-sync-progress` events report the active phase while persisted `sync_runs` remain authoritative history.

### Playlist export

Milestone 16 export coverage verifies:

- normal UTF-8 M3U8 output
- source order and intentional duplicate preservation
- relative-path preference and Windows cross-volume absolute-path fallback
- refusal to export when any supported playlist entry is unresolved
- preservation of an existing destination when a temporary write fails
- immutable export snapshot/history persistence
- portable ZIP structure with one copied audio file for repeated references to the same `local_file_id`
- source playlist cover inclusion as `cover.<ext>` in portable ZIP bundles
- M3U entries inside the bundle resolving to the copied archive paths

The native macOS smoke check generated a bundle through the real Rust export path, passed `unzip -t`, opened successfully with Python's standard `zipfile` reader, and confirmed every M3U audio reference existed in the archive.

### Filesystem normalization

Milestone 10 filesystem tests use temporary directories and cover:

- Windows-invalid characters and reserved device names
- consistent Unicode normalization and deterministic long-component shortening
- single-disc and multi-disc canonical paths plus unknown album/year fallbacks
- case-insensitive path collisions, stable suffixing, and case-only rename staging
- atomic rename failures and verified cross-filesystem copy failures
- path traversal and library-root boundary rejection
- preservation of external ownership and preferred-file state after normalization

Normalization moves only confidently resolved preferred files. It preserves managed/external ownership, does not automatically delete external duplicates, and checks cancellation between file operations so an in-progress move can finish safely.

### Acquisition provider boundary

Milestone 12 acquisition tests use a deterministic fake provider and temporary application-data directories. Coverage includes:

- multiple Spotify source references producing one durable acquisition job for the same logical library track
- bulk acquisition overlapping work across multiple missing tracks while never exceeding the configured three-track worker-pool bound
- retryable provider failure advancing to a distinct candidate with a bounded attempt count
- a previously failed acquisition job being re-queued on a later sync when the track is still tracked and still missing locally
- a persisted active acquisition being recovered as `acquisitionInterrupted` after restart semantics, with its stale provider job ID cleared and the next sync requeueing it normally
- clearing a previous acquisition session deleting failed job/error/candidate state and allowing the still-missing track to be queued again as a clean job
- acquisition failures being persisted into the owning sync run so the run is reported as partial instead of silently successful
- cooperative provider cancellation persisting a cancelled job
- successful provider completion stopping at the durable `staged` state without creating or promoting a local audio file
- tracked missing material appearing in the Staging projection before an acquisition job exists
- multiple plausible candidates pausing as `Needs Resolution` instead of being selected silently
- low-confidence provider results remaining available for manual resolution after the full provider chain fails to produce an automatic match
- ordered provider fallback when the highest-priority provider has no automatic-quality match
- ordered provider fallback when a higher-priority provider's automatic acquisition attempt fails
- an automatic-quality exact match whose transfer fails retrying the complete provider chain automatically up to the bounded chain-attempt limit, then remaining a retryable failed acquisition instead of being converted into manual resolution
- stopping provider traversal after a higher-priority provider stages a match
- preserving every returned provider output for manual resolution when no provider auto-matches, grouped in configured provider order
- multiple compatible exact-ISRC provider records auto-selecting one equivalent recording instead of forcing review solely because of a tie
- equal-confidence acquisition candidates preferring FLAC, then other lossless and higher-quality audio metadata
- `Needs Resolution` rows being re-queued with fresh candidates by both bulk retry and a later Sync Library run
- manual selection switching acquisition to the candidate's provider and duplicate provider tokens remaining isolated by `(provider, provider_token)` identity
- backward-compatible rejection of legacy candidates that were persisted before provider provenance was embedded in candidate JSON
- failed provider jobs remaining in Staging instead of entering Issues
- migration 6 creating the acquisition job table and indexes, migration 13 adding Staging stage/candidate persistence, and migration 16 moving acquisition settings from one provider ID to an ordered JSON provider chain

The Milestone 12 native smoke check passed on macOS: the Issues queue/detail panes remain usable after the clipping fix, and activating the Library root field opens the native folder picker and populates the selected path.

### Monochrome provider

The active acquisition provider has deterministic coverage for current track-service candidate parsing/ranking, duplicate catalog entries collapsing by recording identity or normalized ISRC, direct-stream token routing, safe track URL construction, backward-compatible legacy candidate tokens, legacy lossless manifest decoding, explicit MPEG-DASH detection, resumable response-body transport using byte ranges, retryable transient HTTP statuses, the capped exponential retry schedule used for request/body/payload failures, direct-stream failures qualifying for legacy download fallback even when the direct HTTP error itself is non-retryable, compatibility checks that prevent the fallback from switching to a conflicting recording, playback-outage classification, and health short-circuiting while the Monochrome playback cooldown is active. Provider switching, Staging progress/cancellation wiring, and provider-neutral coordinator behavior remain covered without calling public Monochrome services from CI.

Fresh-session coverage also verifies that resetting provider session state clears the Monochrome playback cooldown so a new manual Sync Library run gives the primary provider a new first attempt instead of inheriting the previous run's temporary outage state. Frontend command-wrapper coverage verifies the `reset_acquisition_session` IPC used by the Sync Library action.

The live Monochrome one-click lossless acquisition/import smoke gate has passed against the primary `tracks.monochrome.st` path. Deterministic tests continue to cover retry/resume, fallback qualification, verification/import recovery, and provider-outage behavior without relying on the public service in CI. If a compatible legacy fallback instance is available, its unencrypted lossless-manifest path remains useful optional smoke coverage.

`REFRAIN_MONOCHROME_API_URLS` may point the legacy fallback path at a controlled compatible instance or mock endpoint.

### Antra provider

Antra has deterministic unit coverage for endpoint-manifest parsing, exact-ISRC candidate projection, Hi-Res FLAC over CD-quality FLAC ranking, Tidal Hi-Res/CD quality projection, and opaque candidate-token round trips that exclude endpoint URLs and authentication data. Provider settings coverage also verifies that `antra` is accepted in the ordered acquisition-provider list, and frontend wrapper tests cover account status, device-login commands, provider health, and health-cache behavior without calling the live service.

The live Antra desktop smoke gate has passed through browser-approved login, provider search/download, and the normal verification/import path. Deterministic coverage remains authoritative for endpoint parsing, candidate ranking, token opacity, settings integration, and provider-health behavior in CI.

### Sockseek sidecar provider

Milestone 13 pins Sockseek `3.0.5`. Its documented mock-daemon mode should cover the production adapter without requiring a real Soulseek account:

```bash
python scripts/create_mock_music_library.py -o /tmp/sockseek-fixture
sockseek daemon \
  --mock-files-dir /tmp/sockseek-fixture/mock-library \
  --mock-files-no-read-tags \
  --mock-files-slow \
  --server-port 5030 \
  -o /tmp/sockseek-out
```

Provider integration verification should cover startup, exact-version rejection, runtime assignment of a non-default available Soulseek listen port, lossless `pref-format` runtime configuration, search/job creation, quality metadata projection for bitrate/sample rate/bit depth/format, durable HTTP polling, transient `404` handling while submitted jobs/result projections are still materializing, restart-aware search failure when Sockseek's `restartCount` advances and the submitted job disappears, bounded search deadlines that preserve partial findings, ready-provider empty searches that cancel stale jobs without becoming retry storms, terminal Soulseek connection states short-circuiting later searches, idempotent cancellation when the provider job already disappeared, unready-provider deadlines that remain retryable, SignalR progress and reconnect fallback, cancellation, successful staging output, stale-candidate expiration, provider failure, and shutdown. HTTP job snapshots are authoritative after event disconnects. A stale sidecar holding Sockseek's default `49998` must not prevent a newly managed daemon from starting a search. A Sockseek engine restart that loses a search must surface as retryable `soulseekUnavailable`, not remain pending until Refrain's generic search timeout.

Before release packaging that enables acquisition, perform one real Soulseek download smoke test and inspect the staged output. Do not treat a successful provider download as an imported library file until Milestone 14 verification/import succeeds.

### Issues and Staging resolution

Milestone 11 coverage verifies the Issues UI as a projection over durable repository state rather than a separate ticket table. Coverage includes:

- frontend Library, Issues, and Staging states, counts, selection, live-progress rendering, and action wiring
- ambiguous-match issues tracking persisted confirmations and rejections
- missing-file, invalid-file, and inaccessible-collection issue projection
- Local Only and acquisition failure states remaining outside the Issues projection
- automatic issue removal when the underlying durable condition is repaired
- logical-library projection and preferred/local-file status used by the Library view

Staging coverage verifies that only tracked material without a present local file is projected, that unstarted tracked material appears as `Needs Local Copy`, that linked source artwork reaches the acquisition cards/inspector, that live byte totals produce real progress percentages in the bottom transfer area, and that manual provider ambiguity exposes provider-separated candidate groups plus Add, Reject, and Search Again controls. Acquisition coordinator coverage includes provider priority/fallback, automatic whole-chain retries for retryable provider/transfer failures, bounded exhaustion into `Failed`, exact-ISRC automatic matching, and retrying a second compatible exact candidate from the same provider before provider fallback. Multi-provider unresolved rows summarize the candidate providers instead of displaying whichever provider happened to run last, and Sockseek candidate cards render available format, bitrate, sample rate, bit depth, and size metadata. Checkbox selection must reveal only contextual actions that apply to the selected rows, such as Retry/Start, Cancel, or recovery Process. The no-work state must render synchronization phase/details instead of a blank queue. Verification/import tests prove that accepted staged audio becomes a managed preferred local file, that untagged provider audio can still pass when provider title/artist and decoded duration exactly identify the requested recording, that canonical metadata plus linked Spotify/source artwork is prepared for embedding before import, that a mixed selected recovery batch still imports a valid track when another selected track fails verification, and that a downloaded row whose staging artifact disappeared is classified as `stagingMissing` so the next acquisition pass can reacquire it instead of leaving it permanently stuck at `Downloaded`; the rejected file remains outside canonical storage.

Manual match decisions continue to use the existing matching persistence and reconciliation rules. Resolving an issue does not require deleting an issue record.

### Desktop build smoke checks

```bash
npm run tauri build -- --no-bundle
```

CI runs this build on Ubuntu, macOS, and Windows. Installer/application bundle generation is additionally exercised by the tag-driven release workflow.

Tauri build jobs fetch the pinned Sockseek sidecar first because `externalBin` is validated during the Rust/Tauri build. The fetch script verifies the official v3.0.5 SHA-256 digest before installing the target-triple-named binary under `src-tauri/binaries/`.

## Manual v0.1 smoke tests

Before releasing v0.1.0, verify on a real Spotify account:

1. Start from a clean application-data state and launch Refrain.
2. Configure a Spotify Client ID and complete authorization.
3. Refresh Spotify source state.
4. Confirm Liked Songs, Saved Albums, and Playlists render correctly.
5. Confirm saved-album track order and playlist duplicate positions are preserved.
6. Restart Refrain and confirm persisted source state hydrates without another refresh.
7. Unsaving an album and refreshing should remove the stale saved-album collection when practical to test.
8. Resize the native window to the configured minimum and larger sizes across browsing and Settings views.

The authentication, source-refresh, desktop browsing, Saved Albums, and native resize smoke tests have passed on macOS for the current v0.1 development line.

## Log-secret verification

Before release, inspect representative application logs after connect, refresh, reconnect, cancellation, and failure paths. Access tokens and refresh tokens must not appear in log output.

Current logging records application paths and error categories/details. Spotify refresh credentials remain in the OS credential store, and access tokens are held in memory. Any future logging added around authentication or HTTP requests must avoid serializing authorization headers, token responses, or credential values.

## Release validation

A release candidate should pass:

- frontend format, lint, type, tests, and production build
- Rust format, Clippy, and tests
- Tauri build smoke checks on Windows, macOS, and Linux
- clean first-run / no-database migration smoke test
- Spotify connect and refresh smoke test
- restart persistence smoke test
- representative log-secret inspection
- Monochrome deterministic provider/acquisition tests when Monochrome provider code changes
- real Monochrome lossless acquisition smoke test before treating the current provider as release-verified
- Antra deterministic provider/frontend tests when the Antra adapter or device-login flow changes
- live Antra device-login, exact-search, FLAC-download, and verification/import smoke test before treating Antra as release-verified
- Sockseek mock-daemon verification when changing the Sockseek adapter
- playlist-export Rust/frontend coverage plus an external ZIP-reader smoke check when export or bundle-writing behavior changes

See `docs/reference/release.md` for packaging and tag behavior.
