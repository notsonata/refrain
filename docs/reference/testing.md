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

### Rust backend

```bash
npm run rust:fmt
npm run rust:clippy
npm run rust:test
```

Rust tests cover persistence, migrations, Spotify authentication, source synchronization, local-library scanning, and matching behavior. The database suite includes opening a previously absent SQLite database, applying migrations, configuring SQLite pragmas, reopening persisted state, transactional source replacement, local-file pagination, and preferred-file selection.

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

### Reconciliation core

Milestone 9 reconciliation tests cover:

- repeated reconciliation without duplicate library-track creation
- one library track serving multiple source tracks and collection entries
- preservation of intentional duplicate playlist positions
- persistence of manual match decisions across reconciliation passes
- removal from one collection without affecting references from other collections
- cancellation while preserving already committed valid state
- typed frontend sync command wrappers

The Milestone 9 reconciliation core itself does not download, move, normalize, or delete user audio files. Later sync phases may act on its confidently resolved output.

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
- retryable provider failure advancing to a distinct candidate with a bounded attempt count
- cooperative provider cancellation persisting a cancelled job
- successful provider completion stopping at the durable `staged` state without creating or promoting a local audio file
- failed provider jobs appearing automatically in the Issues projection
- migration 6 creating the acquisition job table and indexes

The Milestone 12 native smoke check passed on macOS: the Issues queue/detail panes remain usable after the clipping fix, and activating the Library root field opens the native folder picker and populates the selected path.

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

Provider integration verification should cover startup, exact-version rejection, search/job creation, durable HTTP polling, SignalR progress and reconnect fallback, cancellation, successful staging output, provider failure, and shutdown. HTTP job snapshots are authoritative after event disconnects.

Before release packaging that enables acquisition, perform one real Soulseek download smoke test and inspect the staged output. Do not treat a successful provider download as an imported library file until Milestone 14 verification/import succeeds.

### Issues and manual resolution

Milestone 11 coverage verifies the Issues UI as a projection over durable repository state rather than a separate ticket table. Coverage includes:

- frontend Library and Issues states, counts, selection, and match-decision interactions
- ambiguous-match issues tracking persisted confirmations and rejections
- missing-file, invalid-file, and inaccessible-collection issue projection
- automatic issue removal when the underlying durable condition is repaired
- logical-library projection and preferred/local-file status used by the Library view

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
- Sockseek mock-daemon provider verification when acquisition code changes
- real Soulseek acquisition smoke test before an acquisition-enabled release

See `docs/reference/release.md` for packaging and tag behavior.
