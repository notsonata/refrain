# Refrain Technical Design

## Status

This document defines the technical design for Refrain through v1.0.0.

It implements the behavior defined in:

- `docs/BRIEF.md`
- `docs/SPEC.md`

The repository now contains the released v0.1 desktop application and the v1 Local Library Index, Matching Engine, and Reconciliation Core implementation. Paths and module names in later v1 sections remain design guidance until their owning milestones are implemented.

## Design Goals

The implementation should:

- keep Spotify source state, local-library state, and acquisition state clearly separated
- make Local Sync and Spotify Sync deterministic, repeatable, and independently observable
- avoid duplicate managed audio for the same recording
- prefer unresolved state over incorrect automatic matching
- keep filesystem ownership and deletion safety explicit
- keep acquisition providers replaceable
- work as a normal desktop application on Windows, macOS, and Linux
- remain usable without a media server
- keep the frontend thin and move privileged work into the native backend
- use the smallest architecture that satisfies the product spec

## Technology Choices

### Desktop Runtime

- Tauri 2
- Rust stable for the native backend
- system webview provided by Tauri for the frontend

Tauri owns application lifecycle, native dialogs, process management, filesystem access, secure backend commands, and packaging.

### Frontend

- Svelte 5
- TypeScript
- Vite
- Tailwind CSS 4
- shadcn-svelte / Bits UI for accessible primitives where useful
- Lucide plus Simple Icons, loaded on demand through `unplugin-icons`, for application and brand icons

Use a plain Vite SPA. Do not introduce SvelteKit unless a concrete requirement appears that a static desktop SPA cannot satisfy.

Do not add a client-side router initially. Refrain has a small set of desktop views and can use app-level navigation state. Add routing only if deep-linking or route-addressable state becomes a real requirement.

Do not introduce a global state-management library initially. Use Svelte state/stores for UI state and treat the Rust backend plus SQLite as authoritative application state.

### JavaScript Tooling

- Node.js LTS
- npm
- TypeScript strict mode

Exact package versions should be pinned when the application is scaffolded.

### Persistence

- SQLite
- Rust-owned database access
- `rusqlite` for SQLite access
- `rusqlite_migration` for ordered schema migrations

The frontend must not access SQLite directly.

SQLite configuration:

- foreign keys enabled
- WAL journal mode
- busy timeout configured
- migrations run before normal application startup
- schema-changing migrations execute transactionally where SQLite permits

### HTTP

- `reqwest` in Rust for Spotify and provider HTTP APIs
- `serde` / `serde_json` for API and IPC data

### Audio Metadata

Use `lofty` for reading common audio metadata and file properties.

Initial supported local audio formats should follow formats Refrain can inspect reliably and that Sockseek commonly returns, including:

- FLAC
- MP3
- M4A/AAC
- OGG
- Opus
- WAV
- ALAC where supported by the metadata layer

Unsupported files remain untouched and are ignored by the scanner.

### File Hashing

Use BLAKE3 for Refrain-owned content identity and duplicate verification.

Full-file hashing is lazy. It should not be required for every file on every scan.

### Logging

Use Rust `tracing` with rolling local log files.

Never log:

- Spotify access tokens
- Spotify refresh tokens
- Soulseek passwords
- Authorization headers
- PKCE verifiers

## High-Level Architecture

```text
┌─────────────────────────────────────────────────────────────┐
│                     Svelte Desktop UI                       │
│                                                             │
│  Local          Spotify          Issues          Settings    │
└────────────────────────────┬────────────────────────────────┘
                             │ typed Tauri commands/events
                             ▼
┌─────────────────────────────────────────────────────────────┐
│                      Refrain Core (Rust)                     │
│                                                             │
│  Spotify Adapter                                             │
│  Source Repository                                           │
│  Library Scanner                                             │
│  Matching Engine                                             │
│  Reconciliation Engine                                      │
│  Acquisition Engine                                         │
│  Normalization Service                                      │
│  Export Service                                             │
│  Mirror Service                                             │
│  Scheduler / Job Coordinator                                │
│  Credential Store                                           │
└───────────────┬──────────────────────┬──────────────────────┘
                │                      │
                ▼                      ▼
          ┌──────────┐          ┌───────────────┐
          │ SQLite   │          │ Local Files   │
          └──────────┘          └───────────────┘
                │
                │ acquisition provider boundary
                ▼
        ┌─────────────────────┐
        │ Sockseek Adapter    │
        └──────────┬──────────┘
                   │ HTTP + SignalR
                   ▼
        ┌─────────────────────┐
        │ Bundled Sockseek    │
        │ daemon sidecar      │
        └─────────────────────┘
```

## Architectural Boundaries

### Frontend

The frontend is responsible for:

- rendering application state
- collecting user input
- invoking backend commands
- subscribing to progress and state-change events
- local presentation state such as selected view, sort order, filters, and open dialogs

The frontend is not responsible for:

- Spotify token management
- direct Spotify API calls
- SQLite access
- arbitrary filesystem access
- library scanning
- matching
- download verification
- launching Sockseek
- destructive file operations

### Refrain Core

The Rust backend owns:

- application configuration
- secure credentials
- source synchronization
- persistence
- filesystem indexing
- recording identity and matching
- reconciliation
- acquisition provider orchestration
- staging and verification
- canonical file placement
- exports
- mirrors
- scheduling
- error classification
- native lifecycle

### Provider Boundary

Acquisition providers may search for and acquire candidate audio.

They do not decide:

- whether a track is needed
- whether a candidate is the correct recording
- where the final file belongs
- whether a playlist is resolved
- whether a managed file should be deleted

Those remain Refrain responsibilities.

## Intended Repository Structure

The first implementation should converge on a structure similar to:

```text
src/
├── app/
├── components/
├── views/
├── lib/
└── types/

src-tauri/
├── src/
│   ├── app.rs
│   ├── commands/
│   ├── db/
│   ├── domain/
│   ├── spotify/
│   ├── library/
│   ├── matching/
│   ├── sync/
│   ├── acquisition/
│   ├── export/
│   ├── mirror/
│   ├── scheduler/
│   └── security/
├── migrations/
└── tauri.conf.json
```

This is a module boundary, not a requirement to create empty directories before they are needed.

## Application Data Locations

Use platform application-data directories for internal state.

Store:

```text
<AppData>/refrain/refrain.sqlite3
<AppData>/refrain/logs/
<AppData>/refrain/runtime/
<CacheDir>/refrain/
```

Do not store the database inside the user's music library.

The user selects a canonical library root separately.

The library root contains managed music files but should not require an internal Refrain database.

Mirror destinations may contain:

```text
.refrain/manifest.json
Music/
Playlists/
```

The mirror manifest records only paths managed by Refrain on that target.

## Domain Model

The central relationship is:

```text
SourceCollection
      │
      ▼
PlaylistEntry
      │
      ▼
SourceTrack
      │
      ▼
TrackLink
      │
      ▼
LibraryTrack
      │
      ▼
LocalFile
```

A playlist entry is membership and ordering.

A source track is provider identity.

A library track is Refrain's logical recording identity.

A local file is a physical representation of a library track.

## SQLite Schema

Use integer internal primary keys and provider IDs as unique external identifiers.

Store timestamps as UTC Unix milliseconds.

### `app_settings`

Single-row application configuration.

```text
id                          INTEGER PRIMARY KEY CHECK (id = 1)
library_root                TEXT NULL
keep_removed_managed_files  INTEGER NOT NULL DEFAULT 1
sync_on_startup             INTEGER NOT NULL DEFAULT 0
sync_interval_minutes       INTEGER NULL
acquisition_enabled         INTEGER NOT NULL DEFAULT 0
created_at                  INTEGER NOT NULL
updated_at                  INTEGER NOT NULL
```

`sync_interval_minutes = NULL` means periodic synchronization is disabled.

### `source_accounts`

```text
id                   INTEGER PRIMARY KEY
provider             TEXT NOT NULL
provider_account_id  TEXT NOT NULL
display_name         TEXT NULL
image_url            TEXT NULL
client_id            TEXT NOT NULL
created_at           INTEGER NOT NULL
updated_at           INTEGER NOT NULL
last_source_sync_at  INTEGER NULL
UNIQUE(provider, provider_account_id)
```

For Spotify, `provider_account_id` should use the stable account identifier returned for the authenticated user when available. `image_url` stores the current Spotify profile image URL when the profile exposes one.

Secrets are not stored here.

### `source_collections`

```text
id                      INTEGER PRIMARY KEY
source_account_id       INTEGER NOT NULL REFERENCES source_accounts(id)
provider_collection_id  TEXT NOT NULL
kind                    TEXT NOT NULL
name                    TEXT NOT NULL
snapshot_id             TEXT NULL
owner_provider_id       TEXT NULL
is_accessible           INTEGER NOT NULL DEFAULT 1
access_issue             TEXT NULL
image_url                TEXT NULL
external_url             TEXT NULL
album_artists_json       TEXT NULL
album_release_date       TEXT NULL
album_type               TEXT NULL
album_label              TEXT NULL
album_copyrights_json    TEXT NULL
album_external_url       TEXT NULL
created_at              INTEGER NOT NULL
updated_at              INTEGER NOT NULL
UNIQUE(source_account_id, provider_collection_id)
```

`kind` initially supports:

- `liked_songs`
- `playlist`
- `saved_album`

Use a stable synthetic provider ID for Liked Songs, such as `spotify:liked-songs`.

Saved-album collections persist album-level Spotify metadata separately from track metadata so album detail views can show the album artists, full release date, release type, copyrights, Spotify URL, and label when available without reconstructing them from an arbitrary track. Spotify removed the `label` field from Album responses in February 2026, so the album detail UI uses the phonographic copyright holder as a clearly labeled rights-holder fallback when `album_label` is absent.

Spotify collection artwork and external URLs are also persisted at the collection level. During Spotify refresh, playlist covers are refreshed from Spotify's dedicated playlist-cover endpoint so Refrain follows custom artwork assigned to the playlist. The playlist-list response remains a fallback, and browse projections keep the first-track artwork fallback for rows created before collection artwork was persisted.

### `source_tracks`

```text
id                    INTEGER PRIMARY KEY
provider              TEXT NOT NULL
provider_track_id     TEXT NOT NULL
uri                   TEXT NULL
isrc                  TEXT NULL
title                 TEXT NOT NULL
normalized_title      TEXT NOT NULL
artists_json          TEXT NOT NULL
normalized_artists    TEXT NOT NULL
album                 TEXT NULL
normalized_album      TEXT NULL
duration_ms           INTEGER NULL
disc_number           INTEGER NULL
track_number          INTEGER NULL
release_year          INTEGER NULL
explicit              INTEGER NULL
version_kind          TEXT NULL
version_detail        TEXT NULL
image_url             TEXT NULL
external_url          TEXT NULL
created_at            INTEGER NOT NULL
updated_at            INTEGER NOT NULL
UNIQUE(provider, provider_track_id)
```

`artists_json` stores the ordered display artist list.

`normalized_artists` is a stable normalized comparison representation used by the matcher.

### `collection_entries`

```text
id                 INTEGER PRIMARY KEY
collection_id      INTEGER NOT NULL REFERENCES source_collections(id) ON DELETE CASCADE
position           INTEGER NOT NULL
source_track_id    INTEGER NULL REFERENCES source_tracks(id)
provider_item_uri  TEXT NULL
item_type          TEXT NOT NULL
added_at           INTEGER NULL
unavailable_reason TEXT NULL
UNIQUE(collection_id, position)
```

`source_track_id` may be null when Spotify returns an unavailable, removed, or unsupported playlist item.

This preserves playlist positions without manufacturing a track object.

### `library_tracks`

```text
id                         INTEGER PRIMARY KEY
canonical_source_track_id  INTEGER NULL REFERENCES source_tracks(id)
title                      TEXT NOT NULL
normalized_title           TEXT NOT NULL
artists_json               TEXT NOT NULL
normalized_artists         TEXT NOT NULL
album                      TEXT NULL
normalized_album           TEXT NULL
isrc                       TEXT NULL
duration_ms                INTEGER NULL
disc_number                INTEGER NULL
track_number               INTEGER NULL
release_year               INTEGER NULL
explicit                   INTEGER NULL
version_kind               TEXT NULL
version_detail             TEXT NULL
created_at                 INTEGER NOT NULL
updated_at                 INTEGER NOT NULL
```

A library track may exist without a local file.

Canonical metadata is seeded when the library track is created and remains stable unless deliberately changed. Linking another source release does not automatically move the canonical file to a different album path.

### `track_links`

```text
id                INTEGER PRIMARY KEY
source_track_id   INTEGER NOT NULL REFERENCES source_tracks(id) ON DELETE CASCADE
library_track_id  INTEGER NOT NULL REFERENCES library_tracks(id) ON DELETE CASCADE
method            TEXT NOT NULL
confidence        INTEGER NOT NULL
confirmed_by_user INTEGER NOT NULL DEFAULT 0
created_at        INTEGER NOT NULL
updated_at        INTEGER NOT NULL
UNIQUE(source_track_id)
```

`confidence` is an integer from `0` to `10000`, avoiding floating-point persistence differences.

`method` initially supports:

- `existing`
- `isrc`
- `metadata`
- `acquisition`
- `user`

Many source tracks may link to one library track.

One source track has at most one active library-track link.

### `track_rejections`

```text
source_track_id   INTEGER NOT NULL REFERENCES source_tracks(id) ON DELETE CASCADE
library_track_id  INTEGER NOT NULL REFERENCES library_tracks(id) ON DELETE CASCADE
reason            TEXT NULL
created_at        INTEGER NOT NULL
PRIMARY KEY(source_track_id, library_track_id)
```

A rejected pair is excluded from future automatic matching until relevant metadata is materially changed or the user clears the decision.

### `local_files`

```text
id                 INTEGER PRIMARY KEY
library_track_id   INTEGER NULL REFERENCES library_tracks(id)
path               TEXT NOT NULL
ownership          TEXT NOT NULL
is_preferred       INTEGER NOT NULL DEFAULT 0
state              TEXT NOT NULL
format             TEXT NULL
file_size          INTEGER NOT NULL
modified_at        INTEGER NOT NULL
duration_ms        INTEGER NULL
bitrate            INTEGER NULL
sample_rate        INTEGER NULL
channels           INTEGER NULL
content_hash       TEXT NULL
tag_title          TEXT NULL
tag_artists_json   TEXT NULL
tag_album          TEXT NULL
tag_isrc           TEXT NULL
scan_error         TEXT NULL
created_at         INTEGER NOT NULL
updated_at         INTEGER NOT NULL
UNIQUE(path)
```

`ownership`:

- `managed`: acquired by Refrain
- `external`: existed before Refrain took ownership of the library state

Moving or renaming an external file during normalization does not change it to `managed`.

`state`:

- `present`
- `missing`
- `invalid`

At most one present file per library track should normally be marked `is_preferred = 1`.

### `sync_runs`

```text
id                 INTEGER PRIMARY KEY
trigger             TEXT NOT NULL
status              TEXT NOT NULL
phase               TEXT NULL
started_at          INTEGER NOT NULL
finished_at         INTEGER NULL
source_added        INTEGER NOT NULL DEFAULT 0
source_removed      INTEGER NOT NULL DEFAULT 0
matched             INTEGER NOT NULL DEFAULT 0
missing             INTEGER NOT NULL DEFAULT 0
needs_review        INTEGER NOT NULL DEFAULT 0
acquisition_failed  INTEGER NOT NULL DEFAULT 0
error_message       TEXT NULL
```

`trigger`:

- `manual`
- `startup`
- `scheduled`

`status`:

- `running`
- `succeeded`
- `partial`
- `failed`
- `cancelled`

### `acquisition_jobs`

```text
id                    INTEGER PRIMARY KEY
library_track_id      INTEGER NOT NULL REFERENCES library_tracks(id)
provider              TEXT NOT NULL
provider_job_id       TEXT NULL
status                TEXT NOT NULL
attempt               INTEGER NOT NULL DEFAULT 1
candidate_json        TEXT NULL
staging_path          TEXT NULL
error_code            TEXT NULL
error_message         TEXT NULL
created_at            INTEGER NOT NULL
started_at            INTEGER NULL
finished_at           INTEGER NULL
updated_at            INTEGER NOT NULL
```

Provider-specific payloads remain opaque JSON at the persistence boundary.

Milestone 12 uses the following durable acquisition job lifecycle:

- `queued` — durable missing-track work exists but has not started
- `running` — a provider candidate/job is currently active
- `staged` — the provider completed successfully and candidate output remains in Refrain-controlled staging pending verification/import
- `failed` — bounded acquisition attempts ended without acceptable provider completion
- `cancelled` — acquisition was cooperatively cancelled

`staged` is deliberately not equivalent to synced or imported audio. Verification and canonical-library import remain Refrain responsibilities in the later acquisition verification milestone.

### `playlist_exports`

```text
id              INTEGER PRIMARY KEY
collection_id   INTEGER NOT NULL REFERENCES source_collections(id)
mode            TEXT NOT NULL
destination     TEXT NOT NULL
status          TEXT NOT NULL
created_at      INTEGER NOT NULL
finished_at     INTEGER NULL
error_message   TEXT NULL
```

`mode`:

- `m3u8`
- `bundle`

### `playlist_export_entries`

Immutable snapshot of what was exported.

```text
export_id               INTEGER NOT NULL REFERENCES playlist_exports(id) ON DELETE CASCADE
position                INTEGER NOT NULL
source_track_id         INTEGER NOT NULL REFERENCES source_tracks(id)
library_track_id        INTEGER NOT NULL REFERENCES library_tracks(id)
local_file_id           INTEGER NOT NULL REFERENCES local_files(id)
exported_relative_path  TEXT NOT NULL
PRIMARY KEY(export_id, position)
```

### `mirror_targets`

```text
id            INTEGER PRIMARY KEY
name          TEXT NOT NULL
root_path     TEXT NOT NULL
enabled       INTEGER NOT NULL DEFAULT 1
created_at    INTEGER NOT NULL
updated_at    INTEGER NOT NULL
last_sync_at  INTEGER NULL
UNIQUE(root_path)
```

### `mirror_runs`

```text
id             INTEGER PRIMARY KEY
mirror_target_id INTEGER NOT NULL REFERENCES mirror_targets(id)
status         TEXT NOT NULL
started_at     INTEGER NOT NULL
finished_at    INTEGER NULL
copied_count   INTEGER NOT NULL DEFAULT 0
removed_count  INTEGER NOT NULL DEFAULT 0
error_message  TEXT NULL
```

### `mirror_entries`

```text
mirror_target_id  INTEGER NOT NULL REFERENCES mirror_targets(id) ON DELETE CASCADE
kind              TEXT NOT NULL
source_key        TEXT NOT NULL
relative_path     TEXT NOT NULL
content_hash      TEXT NULL
file_size         INTEGER NULL
updated_at        INTEGER NOT NULL
PRIMARY KEY(mirror_target_id, kind, source_key)
```

`kind` initially supports:

- `audio`
- `playlist`

Only paths represented by a previous Refrain mirror manifest or `mirror_entries` row are eligible for automatic mirror cleanup.

## Database Indexes

Create indexes for:

- `source_tracks(isrc)`
- `source_tracks(normalized_title)`
- `source_tracks(normalized_artists)`
- `library_tracks(isrc)`
- `library_tracks(normalized_title)`
- `library_tracks(normalized_artists)`
- `local_files(library_track_id)`
- `local_files(content_hash)`
- `collection_entries(source_track_id)`
- `track_links(library_track_id)`
- `acquisition_jobs(library_track_id, status)`
- `sync_runs(started_at)`
- `playlist_exports(collection_id, created_at)`

Do not add FTS initially.

Matching builds bounded in-memory indexes from lightweight track descriptors during reconciliation.

## Spotify Integration

### Authorization

Use Spotify Authorization Code with PKCE.

Do not use a client secret.

Flow:

1. user enters their Spotify Client ID
2. Refrain generates a cryptographically random PKCE verifier and state value
3. Refrain opens the system browser to Spotify authorization
4. Refrain starts a loopback callback listener bound only to `127.0.0.1`
5. Spotify redirects to the loopback callback
6. Refrain validates `state`
7. Refrain exchanges the code using the PKCE verifier
8. Refrain stores the refresh credential in the OS credential store
9. access tokens are kept in memory and refreshed as needed

Use a dynamically selected loopback port.

The user should register a loopback redirect URI based on `127.0.0.1`; Spotify currently permits dynamic ports for loopback IP literals.

PKCE verifier and OAuth state remain memory-only and are discarded after the flow.

### Scopes

Request only the scopes needed by Refrain:

```text
user-library-read
playlist-read-private
playlist-read-collaborative
user-read-private
```

Do not request playlist write or playback scopes.

### Credential Storage

Store:

- Client ID in SQLite
- refresh token in the OS credential store
- access token in memory only

Use the Rust `keyring` crate behind a `CredentialStore` interface.

Service key format:

```text
service: refrain
account: spotify:<provider_account_id>:refresh-token
```

If the OS credential store is unavailable, fail setup with an actionable error rather than silently storing the refresh token in SQLite.

### Spotify API Boundary

Create a `SpotifyClient` adapter.

It maps Spotify API DTOs into provider-neutral domain objects before persistence.

The rest of Refrain must not depend on Spotify response JSON.

Relevant v1 endpoints currently include:

```text
GET /me
GET /me/tracks
GET /me/playlists
GET /playlists/{id}/items
```

The adapter must handle:

- pagination
- 401 token refresh
- 403 access restrictions
- 429 `Retry-After`
- nullable/removed playlist items
- fields removed or renamed by Spotify
- transient 5xx errors
- cancelled syncs

Do not assume old Spotify batch endpoints exist.

### Playlist Access Limitation

As of Spotify's February 2026 API changes, playlist item access is limited to playlists owned by the authenticated user or playlists where the user is a collaborator.

`GET /me/playlists` may return followed playlists whose items cannot be read.

Refrain should:

- persist the playlist metadata
- mark the collection `is_accessible = 0`
- store a concise `access_issue`
- show it in the UI
- exclude it from reconciliation/export until its items are accessible

Do not silently treat an inaccessible playlist as empty.

### Source Refresh

A Spotify refresh is atomic at the collection level.

For playlists:

1. fetch current playlist metadata
2. compare `snapshot_id`
3. if unchanged and prior entries are complete, retain existing entries
4. if changed, fetch all accessible items
5. replace the collection entries in one SQLite transaction

For Liked Songs:

- fetch all saved tracks with pagination for correctness
- replace the Liked Songs entry set transactionally after a successful fetch

A network failure must not erase the last known good collection state.

## Matching Engine

### Principle

A false automatic match is worse than an unresolved track.

The matcher returns:

```text
Same
Different
Uncertain
```

It also returns evidence suitable for debugging and user review.

### Matching Stages

Run in this order:

1. persisted user decision
2. existing valid track link
3. strong identifiers
4. candidate generation
5. hard compatibility checks
6. weighted metadata scoring
7. runner-up margin check
8. automatic link, review, or unresolved result

### Normalization

Comparison normalization must be deterministic.

For titles, artists, and albums:

- Unicode NFKC normalization
- Unicode-aware lowercase
- trim and collapse whitespace
- normalize common punctuation separators
- treat `&` and `and` equivalently for comparison
- extract common `feat.`, `ft.`, and `featuring` suffixes into artist information
- preserve original display metadata separately

Parse recognized version qualifiers into structured fields.

Initial recognized version classes:

```text
live
acoustic
remix
demo
instrumental
radio_edit
extended
clean
explicit
remaster
mono
stereo
other
```

Do not erase version semantics from comparison input.

### Hard Compatibility Rules

Reject automatic sameness when both sides contain contradictory known values for:

- live vs studio
- acoustic vs non-acoustic version when explicitly identified
- remix vs original
- demo vs release version
- instrumental vs vocal version
- radio edit vs extended/original when explicitly identified
- clean vs explicit
- mutually exclusive version classes

If both sides have non-null, different ISRC values:

- do not auto-match
- the pair may still be presented for manual review if metadata is otherwise strong

Remaster differences are not a hard rejection, but prevent aggressive automatic collapse when other evidence is weak.

### Strong Identifier Match

An exact ISRC match may auto-match when:

- duration is compatible
- there is no hard version conflict

If multiple distinct library tracks share the same ISRC and cannot otherwise be disambiguated, require review.

### Duration Compatibility

Let:

```text
soft_tolerance = max(6000 ms, 2% of source duration)
hard_tolerance = max(12000 ms, 5% of source duration)
```

Rules:

- difference over `hard_tolerance`: incompatible
- difference up to 2000 ms: full duration score
- between 2000 ms and `soft_tolerance`: linearly reduce duration score
- between `soft_tolerance` and `hard_tolerance`: duration contributes zero but the candidate may remain reviewable
- missing duration: no duration contribution and no duration rejection

### Metadata Score

For candidates that pass hard checks:

```text
title similarity        40 points
artist similarity       25 points
duration                 20 points
album similarity        10 points
track/disc metadata      5 points
                       ----------
total                   100 points
```

Similarity values are normalized to `0..1` before weights are applied.

Use deterministic string-similarity functions. Exact normalized equality receives full score.

Artist comparison should compare normalized artist sets rather than one concatenated raw string.

Album mismatch is intentionally low-weight because album, single, compilation, and catalog-replacement releases may represent the same recording.

### Decision Thresholds

```text
score >= 92
and runner-up is at least 8 points lower
and no compatibility warning
    => automatic match

score >= 80
    => needs review

score < 80
    => unresolved / missing
```

An otherwise high-scoring candidate with a runner-up within 8 points requires review.

A differing non-null ISRC caps the result at review.

Persist integer score as `round(score * 100)` in `track_links.confidence`.

### Candidate Generation

Do not compare every source track against every library track.

For one reconciliation pass, build in-memory indexes by:

- ISRC
- normalized title
- normalized primary artist
- duration bucket

Candidate sets are the union of relevant index hits.

Fall back to a bounded fuzzy title search only when the indexed set is empty.

The current matcher bounds that fallback through an in-memory normalized-title trigram index before applying string similarity.

### Manual Decisions

User confirmation creates or updates a `track_links` row with:

```text
method = user
confirmed_by_user = 1
confidence = 10000
```

User rejection creates a `track_rejections` row.

Manual decisions override future automatic matching until explicitly cleared or the referenced object is deleted.

### Acquisition Verification

Downloaded candidates use the same matching engine.

A provider-reported success is not sufficient.

A candidate must reach the automatic-match threshold or be manually confirmed before normalization into the canonical library.

## Local Library Scanner

### Scan Scope

Scan only the configured library root.

Do not follow directory symlinks by default.

Ignore:

- Refrain temporary files
- mirror metadata
- unsupported formats
- obvious hidden/system metadata files

### Incremental Scan

For known paths, compare:

- size
- modification time

If unchanged, reuse stored metadata.

If changed or new:

- read tags and file properties
- inspect the primary embedded picture when present
- update the `local_files` row

Embedded artwork should be cached outside SQLite under Refrain's application-data directory. The scanner should persist only the metadata needed to address that cache entry. Identical embedded images should share a cache entry when practical. Do not duplicate raw cover-art blobs into every local-file row.

Artwork delivery to the webview must stay narrowly scoped. Prefer a Refrain-owned cached-art path exposed through a constrained Tauri asset/protocol boundary or another backend-owned image endpoint. Do not grant arbitrary filesystem reads to the frontend merely to display covers.

Compute a full BLAKE3 hash only when needed for:

- duplicate confirmation
- moved-file recovery
- mirror integrity
- ambiguous file identity

### Moved or Renamed Files

When a known path disappears:

1. mark it `missing`
2. inspect newly discovered files for an existing content hash when available
3. otherwise compare strong metadata and file properties
4. if identity is sufficiently strong, update the existing local-file path
5. otherwise keep the old file missing and treat the new file independently

Do not redownload immediately merely because a known path moved.

## Canonical Metadata

A `LibraryTrack` has stable canonical metadata.

When created from a source track:

- that source becomes `canonical_source_track_id`
- canonical title, artists, album, track/disc numbers, year, duration, and version fields are seeded from it

Additional source tracks linked later do not automatically rewrite canonical metadata.

This prevents a single release, compilation release, or catalog replacement from repeatedly moving the same local file.

A future metadata-editing feature may allow users to change the canonical source explicitly, but this is not required for v1.

## Filesystem Normalization

### Canonical Layout

Normalization considers every preferred present file inside the configured library root. When a library track has an accessible Spotify link, the selected source track supplies canonical path metadata. When no accessible source link exists, Refrain uses the canonical metadata already seeded from the local file so unmatched and local-only tracks can still be organized. Missing artist or album metadata continues to use the existing unknown-value fallbacks.

Managed library files use:

```text
<LibraryRoot>/
  <Album Artist>/
    <Album> (<Year>)/
      <Track> - <Title>.<ext>
```

For multi-disc releases:

```text
<Disc>-<Track> - <Title>.<ext>
```

Examples:

```text
Radiohead/
  In Rainbows (2007)/
    04 - Weird Fishes - Arpeggi.flac

Artist/
  Multi Disc Album (2020)/
    2-03 - Track Title.flac
```

When year is unknown, omit the year suffix.

When album is unknown, use `Unknown Album`.

When album artist is unavailable, use the primary track artist.

### Filename Safety

The sanitizer must produce names valid on Windows, macOS, and Linux.

It must:

- remove control characters
- replace `/` and `\`
- replace Windows-reserved filename characters
- trim trailing spaces and periods
- guard Windows reserved device names
- normalize filesystem-facing Unicode consistently
- avoid `.` and `..`
- shorten overly long components deterministically
- append a stable short identifier when shortening or collision handling requires it

Collision detection must be case-insensitive even on case-sensitive filesystems so a mirrored library remains portable to Windows and default macOS filesystems.

### Collision Handling

If two different library tracks resolve to the same normalized target path, append a stable suffix derived from the library-track ID:

```text
04 - Song Title [lt-1234].flac
```

Do not overwrite another track.

### Moving Existing Files

Files already inside the configured library root are normalized after confident matching.

If an existing file was user-owned before Refrain:

- its `ownership` remains `external`
- Refrain may organize/rename it
- Refrain must not automatically delete it later

File moves should use atomic rename when source and destination are on the same filesystem.

For copy-based moves:

1. copy to a temporary destination file
2. verify size and, when required, hash
3. atomically rename the temporary file to the final path
4. remove the source only after successful verification

## Safe Removal

When the user disables "keep removed managed files", Refrain may clean up an unreferenced file only if:

- `ownership = managed`
- no managed source collection resolves to its library track
- no active operation is using the file

Automatic cleanup should move the file to the platform Trash / Recycle Bin.

If trashing fails:

- do not fall back to permanent deletion automatically
- create a visible cleanup issue

External files are never automatically deleted.

## Acquisition Provider Interface

Define a provider-neutral Rust interface conceptually equivalent to:

```rust
#[async_trait]
pub trait AcquisitionProvider {
    fn id(&self) -> &'static str;

    async fn health(&self) -> Result<ProviderHealth>;
    async fn search(&self, query: TrackQuery) -> Result<Vec<Candidate>>;
    async fn acquire(&self, request: AcquisitionRequest) -> Result<ProviderJob>;
    async fn status(&self, provider_job_id: &str) -> Result<ProviderJobStatus>;
    async fn cancel(&self, provider_job_id: &str) -> Result<()>;
}
```

Core types must not expose Sockseek DTOs.

`TrackQuery` contains normalized provider-neutral metadata such as:

- title
- artists
- album
- duration
- ISRC
- version information

`Candidate` contains only data needed for display, ranking, and a later provider call, plus an opaque provider token.

Providers may implement direct acquisition without a separate user-visible search step.

## Sockseek Integration

### Role

Sockseek is the initial acquisition provider.

Refrain uses Sockseek only for Soulseek search/download behavior.

Refrain does not use Sockseek's Spotify ingestion.

### Process Model

Refrain pins Sockseek `3.0.5` and bundles its unmodified executable as a Tauri sidecar for each supported release target.

The release and CI workflows fetch the official platform archive and verify its pinned SHA-256 digest before Tauri packaging. The binary is not committed to the Refrain repository.

Run:

```text
sockseek daemon
```

bound to loopback only.

Choose an available local port at runtime rather than assuming port `5030`.

The adapter owns sidecar lifecycle:

- start when acquisition is first needed
- perform health/version check
- keep it alive while the application runs
- terminate the owned child process when the provider manager or application shuts down
- capture stdout/stderr into Refrain logs with secret redaction

Sockseek `3.0.5` does not expose a documented daemon-shutdown API, so Refrain owns the spawned child process and terminates that process directly on shutdown.

### API Boundary

Sockseek's daemon currently exposes HTTP plus SignalR and documents its API as experimental.

Therefore:

- all Sockseek calls live behind `SockseekProvider`
- no UI code depends on Sockseek API shapes
- no database schema depends on Sockseek response DTOs
- pin the bundled Sockseek version per Refrain release
- detect incompatible versions at startup
- treat HTTP job state as authoritative
- use SignalR for progress/invalidation only
- recover from SignalR disconnect by polling durable job state

The `3.0.5` adapter uses Sockseek's published OpenAPI contract for:

- `GET /api/server/info` and `GET /api/server/status` for compatibility and readiness
- `POST /api/jobs/search/tracks` followed by `GET /api/jobs/{id}/results/files` for candidates
- `POST /api/jobs/{searchJobId}/downloads/files` for the selected provider candidate
- `GET /api/jobs/{id}` as the durable job-state authority
- `POST /api/jobs/{id}/cancel` for cooperative cancellation
- `/api/events` SignalR subscriptions for progress and invalidation wakeups

Provider-specific search-job IDs and candidate file references remain serialized inside the opaque provider token and do not enter the core data model.

### Staging

Sockseek downloads into a Refrain-controlled staging directory, never directly into the canonical library.

Suggested runtime layout:

```text
<AppData>/refrain/runtime/acquisition/<job-id>/
```

After provider completion:

1. inspect actual downloaded files
2. select the candidate file
3. read tags and properties
4. run acquisition verification
5. if accepted, normalize into the library
6. if rejected, keep enough diagnostic state for the issue UI and clean staging according to retention rules

### Soulseek Credentials

Soulseek credentials are sensitive.

Store them in the OS credential store.

Do not persist the Soulseek password in SQLite.

Sockseek `3.0.5` uses its normal configuration file for Soulseek credentials. Refrain therefore:

1. materialize a per-run configuration file under Refrain's private runtime directory
2. set owner-only permissions where the OS supports them
3. start Sockseek with that explicit config path
4. delete the temporary credential-bearing file after Sockseek has loaded configuration
5. recreate it for a future process start

Do not pass passwords in command-line arguments when avoidable because command lines may be visible to other local processes.

### License Packaging

Sockseek is currently AGPL-3.0.

Before publishing a Refrain build containing the sidecar:

- keep the sidecar as a separate executable
- include required license notices
- identify the exact bundled Sockseek version
- provide compliant access to the corresponding source for the distributed build
- review compliance again if Refrain starts modifying or linking Sockseek code rather than invoking it as a separate process

This is a release requirement, not a runtime feature.

## Reconciliation Engine

### Desired and Actual State

```text
Spotify desired state = accessible collection entries included by tracking rules
actual state  = library tracks + present local files
```

Reconciliation computes actions without allowing the acquisition provider to define desired state.

### Sync Phases

Synchronization runs carry a durable `scope` of `local` or `spotify`. Legacy pre-scope runs remain distinguishable in persistence.

The desktop UI exposes one primary **Sync Library** action. It runs the local scope first and, unless that phase fails or is cancelled, runs the Spotify scope second. **Scan Files** invokes the local scanner directly without starting either synchronization scope. **Refresh Spotify** refreshes persisted source state without running the synchronization workflow.

Local Sync executes:

```text
1. prepare
2. scan local library
3. compare present local tracks with accessible Spotify source tracks
4. persist confident links / surface review candidates
5. normalize present local files
6. finish and report
```

Spotify Sync executes:

```text
1. prepare
2. refresh Spotify source
3. scan local library
4. resolve existing links for tracked Spotify entries
5. match unresolved tracked source tracks
6. create missing library tracks
7. acquire missing audio
8. verify staged audio
9. normalize accepted files
10. apply safe removals
11. resolve playlist projections
12. finish and report
```

Each phase updates `sync_runs.phase`.

`sync_runs.scope` lets the frontend request Local and Spotify histories independently.

### Persistent Spotify Tracking

Tracking state is backend-owned and stored independently from collection entries:

```text
source_collection_sync_rules
  collection_id -> default_included

source_track_sync_overrides
  (collection_id, source_track_id) -> included
```

Collection refresh replaces entry rows but preserves the collection and source-track identities, so these rules survive ordinary Spotify refreshes. A source track is desired for Spotify Sync if any accessible collection includes it after applying the per-track override first and the collection default second. Acquisition queueing uses the same predicate and cancels stale queued jobs that are no longer desired.

### Idempotency

A second sync with unchanged source and filesystem state must not:

- create duplicate library tracks
- create duplicate local files
- repeat successful downloads
- change canonical paths
- repeat user match questions
- remove unrelated files

### Concurrency

Only one synchronization may mutate the canonical library at a time, regardless of scope.

Use a backend `JobCoordinator` with a library mutation lock.

Manual sync requested while another sync is running should return the active run rather than create a second concurrent run.

Exports may take a read lock on library state.

Mirror writes should not run concurrently with canonical library mutation.

### Cancellation

Cancellation is cooperative.

On cancellation:

- stop scheduling new acquisition work
- cancel provider jobs when supported
- allow an in-progress atomic file operation to finish safely
- persist the run as `cancelled`
- keep already committed valid state

Do not roll back completed downloads or file normalization from earlier phases.

## Scheduling

Scheduling exists only while the Refrain desktop process is running.

v1 does not add an OS service or hidden system daemon.

Supported triggers:

- manual
- startup
- periodic interval

Periodic scheduling:

- disabled by default
- persisted as minutes
- recalculated after settings changes
- does not start another sync when one is already active

If a scheduled tick occurs during an active sync, coalesce it rather than queueing multiple runs.

## Playlist Resolution

A source collection is fully resolved when every supported track entry has:

```text
SourceTrack -> TrackLink -> LibraryTrack -> preferred present LocalFile
```

Unavailable Spotify items are not considered resolved audio entries.

The UI should report unresolved counts separately.

## Playlist Export

### Export Preconditions

Playlist export requires all supported track entries to resolve to preferred present local files.

If unresolved entries exist:

- return an `UNRESOLVED_ENTRIES` error
- report the count
- do not silently create a partial playlist

### M3U8 Export

Generate UTF-8 M3U8.

Preserve:

- source order
- intentional duplicate playlist entries

Path strategy:

1. prefer relative paths from the exported M3U8 to local files when representable
2. if source files are on a different Windows volume and cannot be expressed relatively, use absolute native paths
3. never rewrite the canonical library merely to satisfy one export

### Export Snapshot

Create the database export snapshot before writing the final file with status `running`.

Write to a temporary destination.

After successful atomic replacement:

- mark export `succeeded`
- keep immutable `playlist_export_entries`

On failure:

- mark export `failed`
- retain the snapshot for diagnostics

### Portable Bundle

Bundle layout:

```text
<Playlist Name>.zip
  <Playlist Name>.m3u8
  Music/
    <canonical relative library paths>
```

The M3U8 uses relative paths into `Music/`.

Audio is copied, not moved.

If the same local file occurs multiple times in the playlist, include one audio copy but repeat its M3U entry at each source position.

## Library Mirroring

### Destination Layout

```text
<MirrorRoot>/
  .refrain/
    manifest.json
  Music/
    <canonical relative library tree>
  Playlists/
    <collection name>.m3u8
```

Mirror playlists use paths relative to the playlist location, such as:

```text
../Music/Artist/Album (Year)/01 - Song.flac
```

### Mirror Manifest

The manifest records:

- manifest format version
- Refrain version
- target ID
- generated timestamp
- each managed relative path
- source key
- file size
- content hash when available

The database remains authoritative for local application state.

The destination manifest provides portable ownership evidence if the target is temporarily disconnected or the local DB needs to compare against it.

### Mirror Algorithm

1. verify target is mounted and writable
2. load prior Refrain manifest if present
3. build desired audio and playlist manifest
4. copy changed/new files to temporary paths
5. verify copies
6. atomically replace destination files where supported
7. remove stale paths only if they were present in the previous Refrain manifest
8. write new manifest last
9. update `mirror_entries`

Never recursively delete files simply because they are absent from desired state.

### Change Detection

Prefer:

1. known source content hash
2. stored file size plus modification metadata
3. compute hash when identity remains uncertain

## Tauri Command Boundary

Expose coarse application operations, not raw database primitives.

Initial command surface should resemble:

```text
get_app_state
get_settings
update_settings

spotify_begin_auth
spotify_disconnect
spotify_refresh

list_collections
get_collection
list_collection_entries

get_local_library_overview
list_local_files
scan_local_library
hash_local_file
set_preferred_local_file

start_sync
cancel_sync
get_sync_run
list_sync_runs

list_issues
get_match_candidates
confirm_match
reject_match
clear_match_decision

list_acquisition_jobs
cancel_acquisition_job

export_playlist
list_exports

list_mirror_targets
save_mirror_target
remove_mirror_target
run_mirror
```

Exact Rust function names may differ, but the boundary should remain use-case-oriented.

Do not expose arbitrary SQL, arbitrary path reads, or generic shell execution to the frontend.

## Frontend Data Flow

The frontend requests paginated projections from the backend.

Do not send the entire library to the webview when a screen only needs one page.

Suggested projections:

```text
CollectionSummary
CollectionEntryRow
LibraryTrackRow
AcquisitionJobRow
IssueRow
SyncRunSummary
MirrorTargetSummary
```

List commands support:

- limit
- cursor or offset
- sort
- filter where needed

Local-library and Spotify track projections should support server-side filtering once a filter would otherwise require loading the entire collection into the webview. Common filters include search, artist, album, year, format, Spotify membership, and local/presence state. Spotify collection summaries also project aggregate local-entry and attention-entry counts so Albums and Playlists can expose the same local-state overview without loading every collection's entries into the webview. Technical filters such as path/location, acquisition state, match state, explicit state, and duration can use the same query boundary behind the UI's Advanced controls.

Long track lists should be virtualized in the UI.

## Desktop Views

### v0.1.0

Required views:

- Liked Songs
- Playlists
- Settings

The shell may include placeholders only if they do not imply unavailable functionality.

### v1.0.0

Primary navigation is organized around two source workspaces plus secondary utilities:

```text
Local
Spotify
Settings
Issues (secondary attention flow)
```

The Local workspace shows only present local-library tracks and annotates matched Spotify membership. Each row includes its local file path and local artwork when available. The Spotify workspace contains Liked Songs, Saved Albums, and Playlists sections with persistent tracking controls. Inaccessible playlists are excluded from the normal playlist list and exposed in a collapsed secondary section.

Spotify track tables support row multi-selection for tracking changes. Single-track changes continue through `set_source_track_tracking`; bulk `Track`, `Exclude`, and reset-to-default changes use `set_source_tracks_tracking`, which validates that every selected source track belongs to the collection and applies the deduplicated overrides in one SQLite transaction before the frontend reloads collection/source state once.

Use a compact desktop-density system rather than a literal global CSS zoom. Reduce typography, spacing, controls, card dimensions, row heights, and shell chrome consistently so the application feels substantially denser while keeping normal desktop text readable. The primary sidebar should be materially narrower than the current 13rem shell.

Use dense media rows and chips for track/state browsing. Avoid spreadsheet-style tables for the main Local and Spotify browsing flows.

Saved Albums and Playlists use responsive artwork-first grids. Selecting a collection replaces the grid with that collection's track view rather than keeping a permanent master-detail sidebar. The detail view provides an obvious return action. Liked Songs remains a dense track view.

Default-visible filters are search, Spotify/local state, artist, album, year, format, and Spotify membership where applicable. Path/location, acquisition state, match state, explicit state, and duration live behind an Advanced control.

Album art should remain small and lazy-loaded.

## Backend Events

Use Tauri events for transient progress and invalidation, not as the only source of durable state.

Initial event categories:

```text
source-sync-progress
library-scan-progress
sync-progress
acquisition-progress
mirror-progress
state-invalidated
```

Events carry IDs and progress summaries.

After receiving an invalidation event, the frontend may query authoritative state.

## Error Model

Use structured backend errors:

```text
code
message
recoverable
context
```

Examples:

```text
SPOTIFY_AUTH_REQUIRED
SPOTIFY_RATE_LIMITED
SPOTIFY_PLAYLIST_INACCESSIBLE
LIBRARY_ROOT_UNAVAILABLE
LOCAL_FILE_INVALID
MATCH_AMBIGUOUS
ACQUISITION_PROVIDER_UNAVAILABLE
ACQUISITION_FAILED
ACQUISITION_VERIFICATION_FAILED
UNRESOLVED_ENTRIES
EXPORT_FAILED
MIRROR_TARGET_UNAVAILABLE
MIRROR_WRITE_FAILED
CREDENTIAL_STORE_UNAVAILABLE
```

Do not expose secrets or raw credential-bearing upstream responses.

## Issue Model

The Issues UI is a projection over unresolved durable state rather than a separate general-purpose ticket system.

The primary synchronization issue projection is intentionally limited to two user-facing states:

- `Local Only`: a logical library track has a present local file but no membership in any accessible imported Spotify collection.
- `Needs Local Copy`: a Spotify source track is included by persistent tracking rules but has no linked present local file. Match-review candidates are a resolution path for this same state rather than a separate source-wide issue category.

Issue matching must use the same persisted collection defaults and per-track overrides as Spotify Sync. Untracked Spotify entries must not become issues merely because they are absent locally.

Examples:

- local-only track is absent from Spotify source state
- tracked Spotify track has no present local file
- tracked source track needs match review
- acquisition exhausted or failed for tracked desired state
- Spotify playlist items inaccessible as retained diagnostic state
- managed cleanup could not move file to trash
- mirror target write conflict
- acquired file failed verification

Issues should clear automatically when their underlying state is resolved.

## Security

### Tauri Surface

Use a restrictive Tauri capability configuration.

The frontend should not receive broad shell or filesystem permissions.

Native capabilities should be exposed only through explicit Rust commands.

### Network Binding

Sockseek sidecar must bind to loopback only.

The OAuth callback listener must bind to loopback only.

Do not expose either service on `0.0.0.0`.

### Credentials

Sensitive credentials stay in the OS credential store.

Do not serialize secrets into frontend state.

Do not include secrets in crash reports or logs.

### Remote Images

Avoid giving the webview unrestricted remote network access.

Prefer a Rust-backed image/cache service or tightly scoped content-security policy for known image origins.

Do not store cover-art blobs in SQLite.

### Path Validation

For every destructive or write operation:

- canonicalize the configured root
- ensure the destination remains inside the expected root
- reject traversal outside the root
- never trust a playlist name or track title as a raw path

## Performance

Target libraries of at least tens of thousands of tracks without architectural changes.

Key practices:

- incremental local scans
- transactional source updates
- in-memory matching indexes per sync
- paginated UI queries
- virtualized long lists
- lazy file hashing
- lazy cover-art loading
- deduplicated application-data cover-art caching rather than repeated embedded-image decoding during every render
- bounded acquisition concurrency
- bounded Spotify request concurrency

Do not optimize with a separate search service or server database.

SQLite remains sufficient for the intended single-user desktop workload.

## Acquisition Concurrency

Start conservatively.

Default:

```text
1 active Sockseek acquisition job
```

The provider interface may support greater concurrency later.

A single active job reduces sidecar complexity, network contention, and incorrect parallel duplicate acquisition.

Before starting a job, re-check whether the target library track acquired a valid preferred file since it was queued.

## Retry Policy

### Spotify

For transient HTTP failures:

- honor `Retry-After` for 429
- retry idempotent GET requests with bounded exponential backoff and jitter
- maximum 4 retry attempts per request
- refresh once on 401 when a refresh credential exists
- fail the operation if refreshed authentication also fails

### Acquisition

An acquisition job may attempt up to 3 provider acquisitions automatically.

Each retry must either:

- use a different provider candidate
- or follow a provider-declared retryable failure

Do not repeatedly download the exact same rejected candidate.

After automatic attempts are exhausted, mark the track failed and surface an issue.

### Filesystem

Do not retry permission or path-validation errors indefinitely.

Retry short-lived sharing/lock errors with a small bounded backoff.

## Testing Strategy

### Rust Unit Tests

High-value unit coverage:

- normalization functions
- version parsing
- match scoring
- decision thresholds
- runner-up margin behavior
- path sanitization
- collision handling
- mirror manifest diffing
- deletion ownership rules
- sync state transitions

Create a matcher fixture corpus covering:

- album vs single release
- compilation vs original release
- live vs studio
- acoustic
- remix
- explicit vs clean
- remaster
- same title by different artists
- small duration drift
- large duration conflict
- duplicate Spotify IDs/ISRC cases
- poor tags
- ambiguous runner-up

### Database Tests

Use temporary SQLite databases.

Test:

- every migration from empty database
- foreign-key constraints
- uniqueness guarantees
- cascade behavior
- export snapshots
- persisted match decisions
- mirror ownership rows

### Spotify Contract Tests

Mock HTTP at the adapter boundary.

Test:

- PKCE token exchange behavior
- access-token refresh
- pagination
- playlist snapshot optimization
- 429 handling
- null/unavailable items
- inaccessible followed playlist behavior
- February 2026 field names
- removal of stale collection entries only after successful fetch

Do not call live Spotify from normal CI.

### Sockseek Integration Tests

Use the pinned Sockseek daemon's documented mock mode where possible.

Test:

- sidecar startup
- health/version compatibility
- job submission
- durable job polling
- SignalR disconnect recovery
- successful download into staging
- failure mapping
- cancellation

Live Soulseek credentials are not required for normal CI.

The documented mock daemon starts from a generated local fixture with `--mock-files-dir`. HTTP snapshots remain the source of truth in tests even when SignalR delivery is interrupted.

### Filesystem Integration Tests

Use temporary directories.

Test:

- scan
- rename
- moved-file recovery
- normalization
- cross-platform-invalid names
- case-insensitive collision simulation
- copy/verify/rename flow
- trash failure behavior
- external-file deletion protection

### Mirror Tests

Always include tests proving:

- unrelated destination files survive
- only previously managed stale files are removed
- interrupted copy does not replace a good destination with a partial file
- manifest is written last
- repeated mirror with no changes is a no-op

### Frontend Tests

Use:

- Vitest
- Svelte Testing Library

Test:

- primary states
- empty/error/loading states
- issue resolution flows
- settings validation
- progress updates
- long-list rendering behavior

### UI Flow Tests

Use Playwright against the Vite frontend with a mocked Tauri IPC layer for major user flows.

Do not make full native WebDriver E2E a prerequisite for every PR.

### Build Verification

CI should eventually run:

- frontend formatting/lint/type checks
- frontend tests
- Rust format/clippy/tests
- frontend production build
- Tauri build smoke checks on supported OS targets where practical

Release builds require the complete platform packaging matrix.

## Packaging

Build native installers/packages through Tauri.

Each target package contains:

- Refrain executable/application bundle
- frontend assets
- database migrations
- the pinned Sockseek sidecar where supported
- applicable third-party notices

No Docker or server deployment is required.

No mobile build targets are included.

### Signing

Release signing/notarization is platform-specific:

- Windows code signing when release credentials are available
- macOS signing and notarization for public releases
- Linux packages according to selected distribution formats

Do not block local development builds on signing credentials.

## Upgrade Strategy

### Database

Run migrations on startup before opening normal application state.

Before a migration that can destructively rewrite significant data, create a backup of the SQLite database in the application-data directory.

### Sidecar

Pin one tested Sockseek version per Refrain release. The current v1 development pin is Sockseek `3.0.5`.

A Refrain update may update the sidecar only after adapter contract tests pass against that version.

Do not automatically run arbitrary user-installed Sockseek versions as if they were compatible.

### Config

Configuration changes must have defaults that preserve prior behavior.

## Major Tradeoffs

### Tauri Instead of Electron

Chosen for lower idle resource usage and native sidecar support.

Tradeoff: platform webviews vary more than a bundled Chromium runtime, so UI testing must cover supported platforms.

### Rust-Owned Backend Instead of Frontend-Driven Plugins

Chosen to keep database, credentials, filesystem operations, and process control behind a narrow trusted boundary.

Tradeoff: more Rust code and explicit IPC types.

### SQLite Instead of a File-Only Index

Chosen because Refrain needs durable relationships, user decisions, job history, mirror ownership, and source snapshots.

Tradeoff: schema migrations become part of application maintenance.

### LibraryTrack Separate From LocalFile

Chosen because one recording may have zero, one, or multiple physical files and because multiple Spotify objects may refer to one recording.

Tradeoff: additional relational complexity.

### Conservative Matching

Chosen because a wrong audio match corrupts the user's mirrored collection more severely than an unresolved item.

Tradeoff: some cases require manual review.

### Bundled Sockseek Sidecar

Chosen to make the initial acquisition provider usable without requiring a separately managed daemon.

Tradeoffs:

- per-platform sidecar packaging
- experimental API compatibility management
- third-party license compliance
- another process to supervise

The provider boundary limits this cost to one module.

### Polling Plus SignalR

HTTP job state is authoritative; SignalR accelerates progress updates.

Tradeoff: small amount of duplicate communication.

Benefit: losing the live event connection does not lose durable acquisition state.

### No Filesystem Watcher in v1

Library state refreshes during explicit scans/syncs.

Tradeoff: external filesystem changes are not reflected instantly.

Benefit: fewer cross-platform race conditions and less background complexity.

Add a watcher only if real usage demonstrates a need.

### No OS Background Service in v1

Periodic synchronization runs while the desktop process is alive.

Tradeoff: closing Refrain stops scheduled sync.

Benefit: avoids three separate platform service implementations and keeps v1 a normal desktop application.

## External Integration Assumptions

These should be re-verified when implementing or upgrading the relevant adapter.

### Spotify

Current Spotify documentation as of September 2026 indicates:

- desktop applications should use Authorization Code with PKCE when a client secret cannot be stored safely
- loopback redirect IP literals such as `127.0.0.1` may use dynamically assigned ports
- `GET /me/tracks` provides saved tracks
- `GET /me/playlists` provides current-user playlists
- playlist item access uses `GET /playlists/{id}/items`
- playlist item access is limited to playlists owned by the user or where the user is a collaborator
- 2026 API changes removed or renamed several older endpoints and fields

Implementation should follow the current Spotify OpenAPI/reference rather than copied historical response models.

### Sockseek

Current Sockseek documentation as of September 2026 indicates:

- daemon mode exposes HTTP plus SignalR
- the API is explicitly marked experimental
- the daemon defaults to loopback
- an OpenAPI document is published
- a mock daemon mode exists for client integration testing
- the project is licensed AGPL-3.0

The Refrain adapter must remain pinned and isolated accordingly.

For Sockseek `3.0.5`, Refrain uses an explicit loopback port, the HTTP job API, the `/api/events` SignalR hub, a restricted transient config file for credentials, and Refrain-controlled `outputParentDir` staging. Sockseek's Spotify input path is unused.

## Resolved Technical Decisions

The following are considered decided for v1:

- Tauri 2 desktop shell
- Svelte 5 + TypeScript + Vite frontend
- Rust-owned application core
- SQLite persistence
- Spotify direct integration rather than Sockseek Spotify ingestion
- Spotify PKCE with user-provided Client ID
- OS credential storage for refresh credentials
- source collections separated from playlist entries
- SourceTrack separated from LibraryTrack
- LibraryTrack separated from LocalFile
- deterministic conservative matcher
- one canonical preferred local file per library track under normal operation
- mandatory normalization for the canonical library
- provider-neutral acquisition interface
- bundled Sockseek `3.0.5` daemon as initial provider
- HTTP-authoritative Sockseek job state with SignalR progress/invalidation wakeups
- OS credential storage plus restricted transient Sockseek configuration for Soulseek credentials
- staging and verification before canonical import
- M3U8 and portable bundle exports
- manifest-based filesystem mirroring
- no mobile architecture in v1
- no OS background service in v1
- no filesystem watcher in v1

## Remaining Technical Questions

None of these block scaffolding or v0.1.0.

They should be resolved during implementation when the concrete library or platform behavior is known:

- whether the chosen `keyring` backend needs platform-specific fallback guidance on Linux desktop environments
- exact cover-art cache implementation and eviction policy
- whether native full-app E2E coverage is practical on all three CI platforms

Any resolution that changes product behavior or a major architectural boundary should update this document or create an ADR.
