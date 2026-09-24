# Testing

## Validation layers

Use the lowest validation level that gives credible confidence for the change.

### Frontend

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

Rust tests cover persistence, migrations, Spotify authentication and source synchronization behavior. The database suite includes opening a previously absent SQLite database, applying migrations, configuring SQLite pragmas, reopening persisted state, and transactional source replacement.

### Desktop build smoke checks

```bash
npm run tauri build -- --no-bundle
```

CI runs this build on Ubuntu, macOS, and Windows. Installer/application bundle generation is additionally exercised by the tag-driven release workflow.

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

See `docs/reference/release.md` for packaging and tag behavior.
