# Changelog

All notable released changes to Refrain are documented here.

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
