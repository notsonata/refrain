# Changelog

All notable released changes to Refrain are documented here.

## [0.6.0] - 2026-09-27

### Added

- observational local-library indexing with metadata extraction, artwork caching, incremental rescans, moved-file recovery, and paginated browsing
- deterministic local/Spotify matching, reconciliation, scoped synchronization, persistent collection tracking, and per-track tracking overrides
- filesystem normalization with cross-platform path sanitization, collision handling, ownership preservation, and safe move behavior
- Issues and manual match-resolution workflows for ambiguous matches, missing local copies, local-only tracks, inaccessible collections, invalid files, and acquisition failures
- provider-neutral acquisition jobs plus the Sockseek 3.0.5 sidecar integration, operating-system credential storage, staged downloads, retry/cancellation handling, and provider health reporting
- persistent Light, Dark, and System appearance modes with platform-aware translucent sidebar treatment

### Changed

- overhauled the desktop shell, Settings, Local, Spotify, and Issues interfaces around a denser cross-platform desktop layout
- separated Local and Spotify workspaces, added artwork-first Saved Albums and Playlists grids, and added persistent detail-inspector state
- standardized Local and Spotify track tables, sorting, column persistence, filtering, multi-selection, bulk actions, overflow menus, and status terminology
- improved Spotify refresh efficiency and rate-limit handling by reusing available collection metadata and enforcing in-process cooldowns
- simplified synchronization controls into Sync Library, Refresh Spotify, and Scan Files user-facing actions

### Fixed

- corrected source-tracking semantics so untracked Spotify material no longer inflates reconciliation and issue state
- fixed local incremental scans so unchanged files without embedded artwork reuse their existing index rows
- repaired cross-platform CI, Tauri feature configuration, Windows packaging configuration, SSR-safe collection-chip measurement, and stale frontend/Rust tests

## [0.1.0] - 2026-09-25

### Added

- Tauri 2 desktop application for Windows, macOS, and Linux
- Spotify Authorization Code with PKCE using a user-provided Client ID
- secure Spotify refresh-token persistence through the operating system credential store
- synchronization for Liked Songs, Saved Albums, playlists, and playlist entries
- transactional SQLite persistence and restart hydration
- desktop browsing for Liked Songs, Saved Albums, and Playlists
- ordered collection detail with intentional playlist duplicates preserved
- lazy Spotify artwork and virtualized track rendering
- manual refresh progress, bounded retry/rate-limit handling, and cancellation
- tag-triggered GitHub Release packaging for supported desktop platforms

### Changed

- native window layout is constrained to the Tauri viewport with panel-local scrolling
- unknown Spotify failures now fall back to a user-facing recovery message instead of rendering malformed values

### Security

- Spotify client secrets are not required or stored
- refresh credentials remain in the operating system credential store
- access tokens remain in memory and are not intentionally emitted to application logs
