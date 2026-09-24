# Refrain Project Brief

Refrain is a desktop application that mirrors Spotify playlists, Liked Songs, and saved albums to a normalized local music library.

## Problem

Spotify can represent the music a user wants to keep, but that state is separate from the user's local collection. Keeping local files aligned with Spotify manually requires repeatedly checking playlists, saved tracks and albums, finding missing tracks, avoiding duplicate downloads, organizing files, and rebuilding playlists.

## Solution

Refrain treats Spotify as the desired state for a local music library.

It imports playlists, Liked Songs, and saved albums, reconciles them against local library state, and resolves each source track to a logical library track. Missing tracks can be acquired through modular download providers, initially Sockseek/Soulseek, then staged, verified, normalized, and added to the canonical library.

Playlist and album membership is stored separately from audio files, so a track is stored once even when it appears in multiple Spotify collections. Refrain can export resolved playlists as M3U8 files, create portable playlist bundles containing copied audio, and mirror the full library and playlists to another filesystem location.

## Goals

- Keep Spotify playlists, Liked Songs, and saved albums accurately mirrored to local storage.
- Maintain one normalized, canonical local music library.
- Avoid duplicate downloads when the same recording appears in multiple collections.
- Match source tracks to local recordings conservatively and surface ambiguous cases for review.
- Keep acquisition providers replaceable and separate from Refrain's sync logic.
- Support manual, startup, and configurable periodic synchronization.
- Make playlist export and full-library mirroring straightforward.
- Provide a lightweight, responsive desktop experience.

## Scope

### v0.1.0

- Desktop application for Windows, macOS, and Linux.
- User-provided Spotify developer credentials.
- Spotify authentication.
- Fetch and persist Liked Songs, saved albums and their tracks, playlists, and playlist tracks.
- Browse imported Spotify library state.
- Manual refresh.
- No downloading or local-library reconciliation yet.

### v1.0.0

- Scan and reconcile a local music library.
- Match Spotify source tracks to logical library tracks and local files.
- Acquire missing tracks through a modular acquisition engine, initially using Sockseek.
- Stage and verify acquired files before importing them.
- Normalize filesystem organization and filenames.
- Preserve one local copy for tracks referenced by multiple Spotify collections.
- Highlight missing, failed, and ambiguous tracks.
- Configurable behavior for tracks removed from Spotify collections.
- Export resolved playlists as M3U8 files.
- Export portable playlist bundles containing M3U8 files and copied audio.
- Mirror the normalized library and playlists to another mounted filesystem location.
- Manual, startup, and configurable periodic synchronization.

Future work may include additional acquisition providers, rclone-backed remote mirrors, and other source integrations.

## Constraints

- Refrain is desktop-only for now: Windows, macOS, and Linux.
- Refrain does not ship shared Spotify credentials; users provide their own Spotify application credentials.
- The local library is always normalized by Refrain.
- Matching must prefer unresolved results over incorrect automatic matches.
- Files not created or managed by Refrain must not be automatically deleted.
- Acquisition must remain provider-agnostic so backends can be replaced or extended.
- Playlist and mirror outputs should use portable, cross-platform-safe paths where applicable.
- Refrain must remain usable without Plex, Jellyfin, Navidrome, Lidarr, or another media server.

## Success Criteria

### v0.1.0

A user can configure Spotify access, authenticate, fetch Liked Songs, saved albums, and playlists, persist that state locally, and browse it reliably across application restarts.

### v1.0.0

A user can sync Spotify library state to a normalized local collection, acquire and verify missing tracks, avoid duplicate storage for shared tracks, resolve playlists to local files, export playlists in both supported formats, and mirror the library to another location without manual reconstruction of the collection.
