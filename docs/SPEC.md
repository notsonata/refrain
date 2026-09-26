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
- run Local Sync and Spotify Sync independently
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
- acquisition of missing tracks through a modular acquisition provider
- Sockseek/Soulseek as the initial acquisition provider
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

### Browse Spotify State

The user can browse:

- Liked Songs
- saved albums and album tracks
- playlists
- track order within a collection
- basic track metadata
- persistent tracking status for each collection and track

Repeated occurrences of the same track in a playlist remain visible as repeated entries.

Saved Albums and Playlists use an artwork-first grid browser rather than a permanent collection sidebar. Selecting a card opens that collection's track view, with a clear action to return to the collection grid. Liked Songs remains track-oriented.

The normal Playlists grid contains only accessible playlists. Inaccessible playlists remain available in a collapsed secondary section so they do not compete with collections the user can act on.

### Browse and Filter the Local Library

The Local workspace is a compact desktop library browser. Each local track row shows its file path directly, not only at unusually wide window sizes. When embedded cover art is available in the audio metadata, Refrain should use it as the local artwork; otherwise it may fall back to matched Spotify artwork or a neutral placeholder.

The default filter surface should expose commonly useful filters without requiring an advanced panel:

- search
- Spotify/local state
- artist
- album
- year
- file format
- Spotify membership

More technical filters belong behind an Advanced control:

- file path or location
- acquisition state
- match state
- explicit state
- duration

Spotify track views use the same filtering model where the fields are meaningful. In Spotify views, local state distinguishes tracks already present locally from tracks that are missing.

### Local Sync

Local Sync starts from the files the user already has. It scans the configured local library, ensures present files are represented as logical library tracks, compares those tracks against all accessible imported Spotify source data, persists confident links, surfaces ambiguous matches, and normalizes present local files when usable metadata exists.

Local Sync does not create or acquire missing tracks merely because they exist on Spotify. The Local workspace shows each present local track and, when matched, where it appears on Spotify such as Liked Songs, a saved album, or one or more playlists.

### Spotify Sync

Spotify Sync refreshes Spotify source state, applies the user's persistent tracking rules, scans the local library, and reconciles only the resulting tracked subset. Missing tracked tracks may be queued for acquisition when acquisition is enabled. Untracked Spotify material remains browseable but does not become desired local state.

Both synchronization scopes are repeatable. Running either scope again without relevant source, tracking, or local changes must not create duplicate downloads or duplicate managed files.

### Review an Ambiguous Match

When Refrain cannot establish identity confidently:

1. The track is marked as needing review.
2. The user can inspect the source track and candidate local track or file.
3. The user can confirm that they are the same recording or reject the match.
4. The decision is persisted.
5. Refrain does not repeatedly ask about the same resolved ambiguity unless relevant data changes.

### Acquire a Missing Track

1. A missing library track is submitted to the configured acquisition provider.
2. The provider searches for or obtains a candidate audio file.
3. The candidate remains outside the canonical library while being evaluated.
4. Refrain inspects the resulting file and compares it against the requested track.
5. A verified candidate is normalized and imported.
6. A clearly incorrect candidate is rejected.
7. If no suitable candidate can be obtained, the track remains visible as unresolved or failed.

Provider success alone must not imply that a track is synced.

### Remove a Track From Spotify

Refrain provides a setting controlling whether audio that is no longer referenced by managed Spotify collections is kept locally.

If the user chooses to keep downloads:

- source collection membership is updated
- the audio remains in the local library

If the user chooses not to keep downloads:

- Refrain may remove a managed audio file only when no managed collection still references its library track
- Refrain must not automatically delete an unmanaged pre-existing user file

### Export a Playlist

Refrain supports two playlist export modes.

#### Playlist File Export

Exports the current resolved playlist as an M3U8 file referring to the appropriate local library files.

Playlist order and intentional duplicate entries must be preserved.

#### Portable Playlist Bundle

Exports a portable bundle containing:

- an M3U8 playlist
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

Sockseek/Soulseek is the initial provider for v1, but core sync behavior must not depend on Sockseek-specific concepts.

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

Tracks that cannot be acquired through the active provider remain visible. They are not silently removed from desired state.

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

The UI should keep Local Sync and Spotify Sync state distinct rather than combining their histories into one undifferentiated list.

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
- multiple acquisition providers shipping in the initial release

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
- Local Sync compares and normalizes the existing local library without materializing every Spotify track
- Spotify collection tracking and per-track overrides persist across application restarts and source refreshes
- Spotify Sync reconciles and acquires only tracked Spotify selections
- the Local workspace shows matched Spotify membership such as Liked Songs, saved albums, and playlists
- Local rows show their file path and use embedded local cover art when available
- Local and Spotify track views provide search plus common metadata/state filters, with technical filters grouped under an Advanced control
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
