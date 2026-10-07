# Refrain Product Spec

## Overview

Refrain is a desktop application for Windows, macOS, and Linux that manages an existing local music library alongside selectively tracked Spotify playlists, Liked Songs, and saved albums.

The local library and Spotify are separate workspaces. Refrain can inspect the whole local collection against imported Spotify state, while only Spotify collections and tracks the user explicitly tracks become desired local state for downloading and reconciliation.

Refrain is designed to work directly with ordinary local files. It does not require Plex, Jellyfin, Navidrome, Lidarr, or another media server.

## Users and Needs

The primary user is someone who uses Spotify to organize music but also wants a durable local collection.

The user needs to be able to:

- see their Spotify playlists, Liked Songs, and saved albums in a desktop application
- browse their actual local library independently from Spotify
- choose exactly which Spotify collections and individual tracks Refrain should keep locally
- understand which Spotify tracks already exist locally
- acquire tracks that are missing
- avoid downloading the same recording multiple times
- keep local files consistently organized
- preserve playlist membership and order locally
- preserve saved-album membership and album track order locally
- review ambiguous matches instead of accepting incorrect substitutions
- export playlists for use outside Refrain
- copy or mirror the library to another mounted filesystem location
- scan local files independently and run one full library synchronization workflow
- keep existing personal files safe from unintended deletion

## Release Scope

### v0.1.0

v0.1.0 establishes the Spotify source layer and desktop experience.

It includes:

- desktop support for Windows, macOS, and Linux
- user-provided Spotify application credentials
- Spotify authentication
- retrieval of Liked Songs
- retrieval of saved albums and their tracks
- retrieval of user playlists
- retrieval of playlist tracks and ordering
- local persistence of imported Spotify state
- browsing imported Liked Songs, saved albums, playlists, and tracks
- manual refresh of Spotify state

v0.1.0 does not include local-library scanning, matching, downloading, normalization, playlist export, or library mirroring.

### v1.0.0

v1.0.0 completes the local mirroring workflow.

It includes:

- scanning a local music library
- matching Spotify source tracks to logical library tracks
- associating logical library tracks with physical local files
- identifying missing, ambiguous, failed, and available tracks
- acquisition of missing tracks through a modular ordered provider chain
- Monochrome as the default highest-priority lossless provider, with authenticated Antra and Sockseek available as optional ordered providers
- staging and verification of acquired audio before it enters the library
- normalization of managed library paths and filenames
- resolved local playlists
- M3U8 playlist export
- portable playlist bundle export containing the playlist and copied audio
- full-library mirroring to another mounted filesystem location
- manual synchronization
- synchronization on application startup
- configurable periodic synchronization
- configurable handling of tracks removed from Spotify collections
- persistent collection-level Spotify tracking defaults with per-track include/exclude overrides
- separate Local and Spotify workspaces and scoped synchronization runs

## Core Concepts

### Source Account

A configured Spotify account and its authorization state.

Refrain must not ship shared Spotify credentials. The user supplies their own Spotify application credentials.

### Source Collection

A Spotify collection whose membership Refrain tracks.

For v1 this includes:

- Liked Songs
- saved Spotify albums
- Spotify playlists

Liked Songs should behave like a collection even though Spotify does not expose it as a normal playlist.

Each saved album is represented as its own source collection. Its entries preserve the album's track order. Tracks shared with Liked Songs or playlists still refer to the same Spotify source-track identity rather than creating album-specific audio identities.

### Spotify Tracking Rule

Each Spotify collection has a persistent default inclusion state. Individual track entries may override that default to include or exclude a track. A Spotify track is desired locally when at least one accessible collection includes it after applying its collection default and per-track override.

Refreshing Spotify must preserve tracking rules for collections and tracks that still exist. Inaccessible playlists are retained as source state but kept out of the normal browsing flow and actionable Issues queue in a secondary collapsed section.

### Playlist Entry

A position in a source collection.

Playlist entries preserve ordering and may legitimately contain the same track more than once.

Repeated playlist entries do not imply repeated audio storage.

Saved-album collections use the same ordered collection-entry storage model for album tracks, but do not imply playlist semantics or duplicate audio storage.

### Source Track

A track as represented by Spotify.

A source track contains the Spotify-side identity and metadata Refrain needs for display, comparison, and matching.

### Library Track

Refrain's logical representation of a distinct local recording or version.

Multiple source tracks may resolve to the same library track when they represent the same recording.

A library track may exist before a local audio file is available.

### Local File

A physical audio file on disk associated with a library track.

A library track may have more than one local file, for example when pre-existing duplicates or multiple encodings are discovered. Refrain should normally maintain one canonical managed copy.

### Track Link

The relationship between a source track and a library track.

A link may be established automatically or confirmed manually by the user.

Manual confirmations and rejections must persist.

### Acquisition Job

An attempt to obtain audio for a missing library track.

Acquisition providers supply candidate audio. Refrain remains responsible for verification, normalization, and final library placement.

### Mirror Target

A mounted filesystem location that receives a synchronized copy of the normalized library and resolved playlists.

## Core User Flows

### Connect Spotify

1. The user provides their Spotify application configuration.
2. Refrain authenticates the user with Spotify.
3. Refrain stores the resulting authorization state securely.
4. Refrain fetches Liked Songs, saved albums and their tracks, playlists, and playlist entries.
5. The imported state becomes visible in the desktop UI.
6. Subsequent refreshes update the persisted Spotify state.

A user should not need to paste a new short-lived access token for every session.

### Configure the Local Library Root

The user selects the local music-library root from Settings. Activating the Library root path field should open the operating system's native folder picker. Selecting a directory populates the setting with that directory so normal setup does not require manually typing an absolute filesystem path.

### Manage Settings

Settings is organized into five working category tabs for General, Sync, Spotify, Library, and Advanced. Appearance controls live in General, while acquisition controls live in Sync. Acquisition Settings expose an ordered enabled-provider list: providers can be added, removed while at least one remains, and moved up or down to define search priority. Acquisition enablement and provider-order changes persist immediately without a separate Save action. Health controls are shown for enabled providers, and Sockseek account controls are shown whenever Sockseek is enabled. The category tabs remain on one horizontal row at supported desktop sizes; changing tabs replaces the settings panel without leaving the Settings workspace, and keyboard users can move between categories with the standard horizontal tab keys.

Appearance provides Light, Dark, and System theme choices. The selected theme is stored locally on the device, applied before the main interface mounts, and System follows operating-system light/dark changes while the app is running. Dark mode uses an approximately `#181818` application canvas with distinct elevated, control, hover, and selected surfaces, bright foreground text, and clear dark-theme borders. Filled blue actions use darker action-specific fills so white labels retain strong contrast. On macOS and Windows, the native window appearance is synchronized with Refrain's selected theme and the native material is refreshed after theme changes. The base window remains transparent so the primary navigation sidebar can expose the native Sidebar/Acrylic material. The complete application content pane, including all Settings content and the Settings summary rail, is painted as an opaque themed layer above that base window so native material cannot bleed into content surfaces.

The latest checked Monochrome provider-health result is stored locally so Settings can restore the previously verified availability message after an app restart. The UI identifies restored health as coming from the previous session until the user runs Check Monochrome again.

On wide desktop layouts, a compact summary rail keeps Spotify account state, synchronization shortcuts, and local-library status visible while the selected category remains the primary work surface. The rail collapses away when the content area becomes too narrow.

### Browse Spotify State

The user can browse:

- Liked Songs
- saved albums and album tracks
- playlists
- track order within a collection
- basic track metadata
- persistent tracking status for each collection and track

Repeated occurrences of the same track in a playlist remain visible as repeated entries.

Saved Albums and Playlists use an artwork-first grid browser rather than a permanent collection sidebar. Selecting a card opens that collection's track view, with a clear action to return to the collection grid. Liked Songs remains track-oriented. Right-clicking an album or playlist card exposes a pin toggle. Pinned collections stay at the front of their respective Albums or Playlists grid, persist across restarts, and show their pin state directly on the cover.

The normal Playlists grid contains only accessible playlists. Inaccessible playlists remain available in a collapsed secondary section so they do not compete with collections the user can act on.

Search and filtering in Liked Songs must cover the complete collection rather than only the currently visible page. While a query or filter is active and more source entries remain, the view continues loading pages until the complete imported collection can be evaluated.

### Browse and Filter the Local Library

The Local workspace is a compact desktop library browser. Each local track row shows its file path directly, not only at unusually wide window sizes. When embedded cover art is available in the audio metadata, Refrain should use it as the local artwork; otherwise it may fall back to matched Spotify artwork or a neutral placeholder.

Search remains visible on dense library screens. State, metadata, and technical filters belong behind a single compact `Filters` control so the toolbar stays focused on search and primary actions.

Local Library search and filtering must cover the complete indexed result set rather than only the currently visible page. While a query or filter is active and more rows remain, the view continues loading pages until the complete current library projection can be evaluated.

The desktop shell keeps the left navigation at a stable 214 px width as the window grows or shrinks. Refrain enforces a 973 × 720 minimum window size; when the remaining workspace becomes narrow, main-content controls reflow, grids reduce columns, and split views stack while dense tables keep their own internal scrolling.

The filter panel may include:

- Spotify/local state
- tracking state
- file path or location
- acquisition state
- match state
- explicit state
- other fields that cannot be handled directly by sortable table headers

Do not duplicate sortable columns as persistent filters without a specific filtering need. The panel should group related controls, use compact dropdowns for state filters, expand controls evenly across the available row, show active-filter state on the trigger, and provide a clear action without clearing the search query.

Examples of technical filters include:

- file path or location
- acquisition state
- match state
- explicit state
- duration

Spotify track views use the same filtering model where the fields are meaningful. In Spotify views, `Spotify Only` means the source item has no matching local file, while `Needs Local Copy` is the tracked subset that still lacks a present local file or requires match review. Liked Songs exposes a collection-level tracking switch. The switch is on only when the whole collection is effectively tracked. Toggling it applies the requested state to the whole collection and clears older per-track overrides for that collection.

Spotify track tables expose the per-song tracking state directly near the track title. Users can toggle one song at a time or select multiple loaded track rows and use one stateful bulk action to include or exclude the selection from tracking. Bulk selection does not alter search or filter state.

`Local Only` is a normal Local Library status: the track exists on disk but has no membership in the imported accessible Spotify source state. It remains visible in Local Library metrics and track status, but it is not an Issue.

The Issues workspace is reserved for conditions that need a decision or repair, including ambiguous local↔Spotify matching, a previously known local file becoming missing, invalid local files, and inaccessible source collections other than playlists. Inaccessible playlists remain source diagnostics and do not enter Issues. Invalid local files expose an explicit Move to Trash action that uses the platform Trash / Recycle Bin for files inside the configured library root. Acquisition-specific failures and provider ambiguity are handled in Staging.

The Staging workspace contains only tracked Spotify material that still needs a present local copy. A tracked track enters Staging immediately as `Needs Local Copy`, even before an acquisition job exists. Untracked Spotify material that is absent locally is normal browseable source state and does not enter Staging. Non-active Staging rows can be excluded from tracking through the checkbox multi-selection actions. Excluding a Staging track applies exclusions to every linked Spotify source reference for that logical track so it leaves the tracked-only Staging projection. When no tracked material needs acquisition, Staging becomes the detailed synchronization view and shows the current/latest sync phase, timeline, counts, and cancellation state.

### Scan Files

Scan Files is the lightweight local operation. It indexes supported audio under the configured local-library root and also discovers `.m3u` / `.m3u8` playlist files. A previously unknown local playlist file is imported into Local > Playlists, preserving its ordered entries and intentional duplicates when those entries resolve to indexed local audio. Once imported, the persisted Refrain playlist is authoritative so later scans do not overwrite in-app playlist edits from an older file on disk. Scan Files does not acquire missing Spotify tracks or run the full reconciliation workflow.

The Local workspace shows each present local track and, when matched, where it appears on Spotify such as Liked Songs, a saved album, or one or more playlists.

### Sync Library

Sync Library is the primary end-to-end synchronization action. It first performs the local reconciliation/normalization phase, then refreshes Spotify source state, applies the user's persistent tracking rules, and reconciles the resulting tracked subset. When acquisition is enabled, missing tracked tracks are queued, downloaded, verified, tagged, and imported automatically as part of the same operation. Untracked Spotify material remains browseable but does not become desired local state.

The internal local and Spotify synchronization scopes remain repeatable and independently persisted, but the normal UI presents them as one Sync Library action. Repeating synchronization without relevant source, tracking, or local changes must not create duplicate downloads or duplicate managed files.

### Review an Ambiguous Match

When Refrain cannot establish identity confidently:

1. The track is marked as needing review.
2. The user can inspect the source track and candidate local track or file.
3. The user can confirm that they are the same recording or reject the match.
4. The decision is persisted.
5. Refrain does not repeatedly ask about the same resolved ambiguity unless relevant data changes.

### Acquire a Missing Track

1. A tracked track without a present local copy appears in Staging as `Needs Local Copy`.
2. **Sync Library** is a fresh acquisition session. Before the Local phase begins, Refrain abandons every prior acquisition job and provider candidate/error state, removes Refrain-owned acquisition staging artifacts from the previous session, and clears temporary provider outage state. The later Spotify phase then creates new jobs only for tracks that are still tracked and still missing. In Staging, checkbox selection exposes contextual actions for targeted start/retry, cancellation, and recovery processing without a permanent top action bar. A selected failed track also exposes a direct **Retry** action in its inspector; it immediately shows a retrying state and runs the same fresh provider-chain search used by **Search Again**.
3. Refrain records `Queued`, then `Searching`, while enabled acquisition providers are searched in configured priority order. Bulk Staging and Sync Library acquisition may process up to three missing tracks concurrently; each track keeps its own ordered provider chain, retries, staging directory, progress, and terminal state. For each provider, Refrain scores and orders candidates using provider-neutral title, artist, album, duration, and ISRC evidence. A compatible exact-ISRC candidate may proceed automatically even when the provider returned multiple equivalent exact-ISRC records. Cosmetic title suffix differences do not disqualify an exact-ISRC candidate when the title still shares the same normalized base and artist/duration evidence remains compatible. Equivalent exact-ISRC candidates are ordered by audio quality before confidence, so a higher-resolution lossless candidate is attempted first. If one exact candidate fails with a retryable transfer error, Refrain tries the next ranked compatible exact candidate from that same provider within the normal bounded attempt budget before advancing to the next provider. Other automatic selections require a high confidence score and a meaningful lead over the runner-up. If the current provider does not produce an automatic-quality match, or its automatic acquisition attempts fail, Refrain continues to the next configured provider. If the ordered provider chain ends on a retryable provider or transfer failure, Refrain automatically reruns the complete provider search/acquisition chain up to three total chain attempts with backoff before surfacing `Failed`. A provider that cannot start or reports unavailable does not prevent a lower-priority configured provider from being tried. Providers may temporarily suppress new work after a confirmed service-wide playback outage so a bulk queue can immediately continue through lower-priority providers rather than repeating the same unavailable endpoint for every track.
4. `Needs Resolution` is for candidate-identity ambiguity, not transport failure. If no configured provider identifies an automatic-quality match, the track pauses as `Needs Resolution` whenever any provider returned candidates. Manual resolution groups candidates by provider in provider-priority order and shows all returned outputs for each provider. Candidate identity includes both provider and provider token. Within the same match confidence, FLAC and other lossless formats are preferred, followed by stronger available audio-quality metadata. For equivalent exact-ISRC Antra candidates, Hi-Res FLAC ranks ahead of CD-quality FLAC, which ranks ahead of FLAC whose quality metadata is unknown even when release metadata gives the lower-quality candidate a slightly higher confidence score. Sockseek rows display provider-reported format, bitrate, sample rate, bit depth, and size when available. Monochrome rows preserve numeric sample-rate and bit-depth metadata when its search response exposes them; when the primary search omits those values, Refrain reads the candidate FLAC STREAMINFO prefix with a bounded range probe and shows the actual kHz/bit depth when that probe succeeds. Legacy `LOSSLESS` candidates are identified as CD-quality 44.1 kHz / 16-bit. After any provider completes a download, Refrain reads the actual staged audio properties and refreshes the selected candidate's format, size, bitrate, sample rate, and bit depth before presenting the downloaded state. If Sockseek has already returned file results but does not report search completion before the bounded wait expires, Refrain preserves those partial findings and includes them with candidates from the other providers instead of discarding them as a timeout. `Add` uses the candidate's provider, `Reject` removes only that provider/token pair, and `Search Again` reruns the ordered chain with fresh provider results. Retrying failed Staging work refreshes the existing row through the current provider chain; starting Sync Library instead discards all previous resolution state and creates a new acquisition session. If Refrain already found an automatic-quality match but its transfer fails after provider fallback is exhausted, acquisition remains `Failed` and retryable rather than asking the user to manually re-select that known match. If every provider returns no candidates, acquisition likewise remains `Failed` for retry.
5. During an active provider download, Staging shows live byte progress when the active provider reports it. Until byte totals are available, download progress is shown as indeterminate rather than as a synthetic percentage. The selected track's artwork and detailed metadata remain visible in the right-side inspector. Durable provider/job state remains authoritative when live progress is unavailable. Because provider runtime jobs are process-local, any persisted `searching` or `downloading` acquisition found during application startup is classified as an interrupted retryable failure rather than left active indefinitely. Retry can requeue that row; starting Sync Library abandons it with the rest of the previous acquisition session.
6. Provider output first lands outside the canonical library in Refrain-controlled staging, then normal acquisition flows immediately continue into verification/import instead of waiting for another user action.
7. Refrain reads the actual file metadata and duration, verifies it against the requested logical track, writes canonical track metadata before import, and embeds linked source artwork when the downloaded file has no embedded cover. Release-level album/ISRC drift is tolerated when decoded title, artist, and duration are exact and there is no warning or incompatibility indicating a different recording version. Automatic acquisitions remain strict when the selected provider candidate itself has a conflicting ISRC. A candidate explicitly chosen from `Needs Resolution` may carry an alternate release ISRC when title, artist, and duration still identify the same recording exactly; live/remix/edit/other incompatible versions remain rejected. Accepted audio is normalized into the canonical library and recorded as managed local audio.
8. If verification/import cannot complete, the downloaded row remains available for selected recovery processing until the user starts a fresh Sync Library session. Recovery rows are processed independently, so one failure does not block other selected tracks. Starting Sync Library intentionally abandons any unrecovered downloaded rows and reacquires still-missing tracked material from scratch. Once a present local file exists, that track disappears automatically from Staging.

Provider success alone must not imply that a track is synced.

### Remove a Track From Spotify

Refrain provides a setting controlling whether audio that is no longer referenced by managed Spotify collections is kept locally.

If the user chooses to keep downloads:

- source collection membership is updated
- the audio remains in the local library

If the user chooses not to keep downloads:

- Refrain may remove a managed audio file only when no managed collection still references its library track
- Refrain must not automatically delete an unmanaged pre-existing user file

### Local Playlists and Managed M3U Files

The Local workspace contains two nested views: `Songs` and `Playlists`. The Local and Spotify workspace rows in the sidebar are independently collapsible so their nested destinations can be hidden without changing the active page.

Transient application-wide operation feedback lives in one bounded carousel in the sidebar footer instead of stacking cards or inserting banners into workspace content. The carousel includes library synchronization progress and cancellation, Spotify refresh progress and cancellation, Local Library scan progress/results, and global operation errors. Only one item is shown at a time with previous/next controls and a position count so feedback cannot push sidebar controls off-screen. Selecting an item opens a modal with the complete message. Page-specific validation and contextual error states remain with the page that owns them.

Local Playlists include both user-authored playlists and Spotify-backed mirrors. A user-authored playlist is edited directly in Refrain. Every Spotify playlist or Liked Songs collection with collection-level tracking enabled has one corresponding Local Playlist mirror linked to that source collection. Saved Albums do not create Local Playlist mirrors.

A Spotify-backed Local Playlist follows the source collection's effective tracked membership and artwork when available. Spotify order and intentional duplicate positions are preserved. Per-track exclusions are omitted from the local mirror. The mirror is rebuilt automatically after Spotify refresh, collection/track tracking changes, and Spotify reconciliation so additions, removals, ordering changes, and updated Spotify playlist covers propagate without manual playlist editing.

Spotify-backed Local Playlists are source-managed: their name and membership cannot be renamed, reordered, deleted, or edited directly from Local. Untracking the Spotify playlist or Liked Songs collection removes the derived Local Playlist.

Existing `.m3u` and `.m3u8` files found under the configured local-library root are also imported as Local Playlists during Scan Files. Relative M3U paths are resolved from the playlist file's directory. Comments and extended-M3U metadata lines are ignored for membership resolution, while playlist order and duplicate track entries are preserved.

For user-authored Local Playlists, the user can:

- create, rename, and delete a Local Playlist
- select one or more tracks in Local > Songs and add them to a Local Playlist
- add an individual Local track from its row actions
- preserve intentional duplicate track entries
- reorder or remove playlist entries without changing the underlying local audio

Playlist membership and ordering persist in SQLite. Refrain automatically maintains a UTF-8 `.m3u8` file for each newly created user playlist and Spotify-backed mirror under `<Library Root>/Playlists/`. The managed filename follows the playlist name and is sanitized for portable filesystem use. Renaming a managed user playlist moves synchronization to the new managed filename and removes the superseded managed file after the replacement succeeds.

Managed playlist files are rewritten automatically after playlist creation, rename, membership changes, reordering, tracked Spotify playlist reconciliation, library-root changes, and application startup. For Spotify-backed mirrors with source artwork, Refrain also downloads a managed image sidecar beside the M3U8 using the same playlist basename when that path is available, such as `Late Nights.m3u8` plus `Late Nights.jpg`. The sidecar follows managed playlist path/name changes, refreshes when Spotify provides a different artwork URL, and avoids overwriting an unrelated pre-existing image by choosing a collision-safe cover filename. There is no manual file picker or Sync action in the Local Playlists UI.

Imported `.m3u` / `.m3u8` files remain linked to their existing user-owned path instead of being silently moved into Refrain's managed `Playlists` directory. In-app edits to an imported playlist still rewrite that linked target automatically.

Before writing, Refrain validates the complete playlist. If any entry no longer resolves to a usable local file, automatic synchronization records the diagnostic on the playlist and leaves any existing destination file unchanged. The playlist edit itself remains persisted. A later automatic synchronization retries after the missing local-copy condition is resolved. Successful synchronization clears the prior error and records the sync time.

M3U output is UTF-8, preserves intentional duplicate entries, includes extended track metadata, and prefers paths relative to the playlist file when they can be represented. Absolute native paths may be used when a relative path cannot be represented, such as across Windows volumes.

### Export a Playlist

Source-derived playlist export remains a separate operation from Local Playlist sync. Refrain supports two source playlist export modes.

#### Playlist File Export

Exports the current resolved playlist as an M3U8 file referring to the appropriate local library files.

Playlist order and intentional duplicate entries must be preserved.

#### Portable Playlist Bundle

Exports a portable bundle containing:

- an M3U8 playlist
- the Spotify playlist cover as `cover.<image extension>` when source artwork exists
- copies of the audio files needed by the playlist

The playlist inside the bundle must use paths that remain valid when the bundle is moved or extracted elsewhere.

### Mirror the Library

The user selects another mounted filesystem location, such as an external drive.

Refrain mirrors:

- the normalized music library
- resolved playlists

The destination should remain portable across mount locations where practical.

Refrain must distinguish files it manages on the mirror from unrelated files already present there. Mirror cleanup must not delete unrelated user data.

## Matching Behavior

Matching must prioritize correctness over aggressively resolving every track.

Refrain should support three identity outcomes:

- same recording
- different recording
- uncertain

### Strong Matches

Different Spotify source tracks may resolve to the same library track when Refrain has strong evidence that they represent the same recording.

Examples may include:

- album and single releases of the same recording
- compilation and original-album appearances
- Spotify catalog replacements
- duplicate Spotify objects with compatible recording metadata

Album identity alone must not force two otherwise identical recordings to remain separate.

### Distinct Versions

Refrain should treat meaningfully different versions as separate library tracks.

Examples include:

- live versions
- acoustic versions
- remixes
- demos
- instrumentals
- radio edits
- extended mixes
- explicit and clean versions

Version information must not be discarded merely to increase match rates.

### Remasters

Remaster relationships should be treated conservatively. Refrain should not silently collapse a remaster and a non-remastered release unless the evidence is sufficiently strong under the matching rules.

Ambiguous remaster cases should be reviewable.

### Uncertain Cases

When Refrain cannot establish sameness confidently, it should keep the tracks separate and surface the uncertainty rather than risk merging distinct recordings.

Exact matching weights and thresholds belong in the technical design, not this product spec.

## Local File Behavior

### Canonical Managed File

Refrain should normally maintain one canonical managed audio file for each library track.

The same track appearing in multiple collections must not result in multiple managed downloads.

### Existing Files

Existing files should be matched and reused when possible.

Matching an existing file must not transfer destructive ownership of that file to Refrain.

### Local Duplicates

If multiple existing files appear to represent the same recording:

- Refrain may associate them with the same library track
- Refrain may choose one as the preferred file for normal use
- Refrain must not automatically delete unmanaged duplicates

Duplicate cleanup may be added later as a separate user-controlled feature.

### Bad or Poorly Tagged Files

Poor metadata should reduce matching confidence rather than force a guess.

Refrain may use available filename, path, duration, tags, known mappings, and other supported evidence, but low-confidence cases should remain reviewable.

## Acquisition Behavior

The acquisition system must be modular.

Refrain persists an ordered list of enabled acquisition providers. Monochrome is the default first provider and prefers Monochrome's current public track service for lossless candidate lookup and direct FLAC streaming. If that direct stream fails after its bounded retries, Monochrome may resolve the same recording through the older compatible HTTP API pool and try its lossless manifest before the coordinator advances to the next configured provider. The fallback recording must remain compatible with the requested title, artist, duration, and available ISRC evidence. Antra can be enabled after completing its browser-approved device login and currently searches authenticated Tidal/Qobuz lossless mirrors. Sockseek can also be added anywhere in the ordered chain. Core sync behavior must not depend on provider-private concepts.

Future providers may include other acquisition mechanisms.

Provider responsibilities are limited to finding or obtaining candidate audio.

Refrain remains responsible for:

- deciding what is missing
- verification
- matching
- normalization
- final filesystem placement
- playlist resolution
- issue reporting

Tracks that cannot be acquired through the configured provider chain remain visible. They are not silently removed from desired state.

## Library Normalization

Preferred present files inside the configured library root should be normalized into a consistent canonical library structure whenever Refrain has usable track metadata. Spotify-linked tracks use the selected Spotify source metadata; local-only or unmatched tracks fall back to metadata derived from the local file.

Normalization includes:

- deterministic organization
- cross-platform-safe path and filename handling
- stable placement suitable for playlist resolution and mirroring

The exact naming template and sanitization rules belong in the technical design.

## Synchronization Modes

Every synchronization run has a `local` or `spotify` scope. Refrain supports:

- manual synchronization
- synchronization on application startup
- configurable periodic synchronization

Automatic synchronization must not prevent the user from triggering a manual sync.

The UI should present one clear Sync Library action while preserving the internal local and Spotify scope history for diagnostics and reporting.

## Track States

The product should expose enough state for the user to understand what Refrain is doing.

At minimum, tracks may be represented as:

- synced
- missing
- queued
- downloading
- needs review
- failed

Internal implementation may use additional states, but user-visible state should remain understandable and should not expose provider-specific internals unnecessarily.

## Business Rules

1. Only Spotify entries included by persistent tracking rules are desired local state for Spotify Sync.
2. Collection entries represent membership and order, not file ownership.
3. One recording should normally occupy storage once, regardless of how many managed collections reference it.
4. Intentional duplicate entries inside a playlist must be preserved.
5. Different Spotify IDs may resolve to one library track only when Refrain has strong evidence they are the same recording.
6. Meaningfully different versions must remain distinct.
7. User-confirmed match and rejection decisions override future automatic ambiguity resolution unless the underlying objects materially change.
8. Acquisition output is not trusted until Refrain verifies it.
9. Managed files are normalized before becoming part of the canonical library.
10. Unmanaged pre-existing local files must not be automatically deleted.
11. A managed file may be removed because of Spotify changes only when the user's removal setting permits it and no managed collection still requires it.
12. Failure to acquire a track must remain visible to the user.
13. Refrain must work without a separate media server.
14. Desktop behavior must be supported on Windows, macOS, and Linux.

## Significant Edge Cases

### Same Track Across Many Collections

One source or library track may appear in Liked Songs, saved albums, and many playlists.

Refrain stores collection memberships separately and reuses one canonical audio file.

### Duplicate Entry in One Playlist

A playlist may intentionally contain the same track more than once.

The exported and resolved playlist preserves each occurrence and its order while reusing one audio file.

### Saved Album Also Represented Elsewhere

A saved album track may also be Liked or appear in one or more playlists.

The memberships remain separate, but the shared Spotify track identity must not imply multiple managed audio copies.

### Different Spotify IDs for the Same Recording

Refrain may merge them to one library track when strong recording-level evidence supports it.

Otherwise they remain separate or require review.

### Same Title and Artist, Different Recording

Refrain must not collapse live, remix, acoustic, clean, explicit, demo, instrumental, edit, or otherwise meaningfully different versions merely because title and artist are similar.

### Spotify Catalog Replacement

A new Spotify source object may represent the same recording as an older object.

Both may resolve to the same library track when identity can be established confidently.

### Local File Moved or Renamed

A previously known local file may move.

Refrain should attempt to recover its relationship using persisted identity and available file evidence rather than immediately treating the track as a new missing download.

Exact recovery mechanics belong in the technical design.

### Multiple Local Files for One Recording

Refrain may discover multiple physical files for the same library track.

It should choose a preferred file for normal operation without automatically deleting unmanaged alternatives.

### Incorrect Acquisition Result

A downloaded candidate may contain the wrong recording despite appearing correct during provider search.

The file must fail verification and remain outside the canonical library.

### No Acquisition Result

The track remains missing or failed and is highlighted for the user.

Future providers may later resolve it.

### Track Removed From One of Several Collections

Removing a track from one playlist or unsaving an album must not remove the local audio if another managed collection still references it.

### Track Removed From All Managed Collections

The configured removal behavior applies.

Unmanaged pre-existing files remain protected.

### Mirror Destination Contains Unrelated Files

Refrain must not treat the destination as disposable storage.

It may update or remove files it manages there, but unrelated user files must remain untouched.

## Non-Goals for v1.0.0

The following are outside the v1 product scope unless explicitly promoted into scope later:

- mobile applications
- mobile-specific synchronization protocols
- cloud backup
- rclone-backed remote mirroring
- music playback as a primary product feature
- music discovery or recommendation features
- Plex, Jellyfin, Navidrome, or Lidarr integration as a requirement
- automatic destructive duplicate cleanup of unmanaged files

## Acceptance Criteria

### v0.1.0

v0.1.0 is complete when:

- the application runs as a desktop application on the supported desktop platforms targeted by the release
- the user can supply their own Spotify application configuration
- the user can authenticate with Spotify without Refrain shipping shared credentials
- Refrain can fetch Liked Songs
- Refrain can fetch the user's saved albums and preserve album track order
- Refrain can fetch the user's playlists
- Refrain can fetch and preserve playlist track order
- imported Spotify state persists across application restarts
- the user can browse the imported Liked Songs, saved albums, playlists, and tracks
- the user can manually refresh Spotify state
- no local-library or download functionality is required for this release

### v1.0.0

v1.0.0 is complete when:

- Refrain can scan and represent an existing local music library
- Local and Spotify are separate primary workspaces
- Scan Files can index the existing local library without starting full synchronization
- Spotify collection tracking and per-track overrides persist across application restarts and source refreshes
- Sync Library performs local reconciliation and then reconciles/acquires only tracked Spotify selections
- the Local workspace shows matched Spotify membership such as Liked Songs, saved albums, and playlists
- Local rows show their file path and use embedded local cover art when available
- Local and Spotify library views keep search visible and group state/metadata filters under one compact Filters control
- Saved Albums and Playlists use artwork-first collection grids that open into collection track views instead of permanently reserving width for a collection sidebar
- inaccessible playlists stay out of the normal playlist flow and are available only in a secondary hidden/collapsed section
- existing local audio can be matched and reused when confidence is sufficient
- ambiguous matches can be reviewed and user decisions persist
- repeated references to the same recording do not create repeated managed downloads
- meaningfully different recording versions are not silently collapsed
- missing tracks can be submitted to the configured acquisition provider
- acquired files are staged and verified before entering the canonical library
- verified files are normalized into the managed library
- failed or unavailable tracks remain visible
- playlist order and intentional duplicate entries are preserved
- the user can create persisted Local Playlists from local-library tracks, preserve duplicate entries and order, and have Refrain automatically maintain their managed M3U8 files without overwriting an existing file when any playlist entry is unavailable
- resolved playlists can be exported as M3U8 files
- portable playlist bundles include the playlist and required copied audio
- the full normalized library and playlists can be mirrored to another mounted filesystem location
- mirror operations do not delete unrelated destination files
- manual, startup, and configurable periodic synchronization are available
- the user can choose whether managed audio removed from all Spotify collections is retained locally
- unmanaged pre-existing audio is never automatically deleted
- Refrain remains usable without a media server

## Open Product Questions

There are no known unresolved product decisions blocking technical design.

Remaining decisions such as the exact database schema, matching weights, filename template, provider interfaces, secure credential implementation, retry policy, and module boundaries belong in the technical design.
