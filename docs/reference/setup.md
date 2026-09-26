# Setup

## Requirements

- Git
- Node.js 22.12 or newer
- npm
- Rust 1.95 or newer
- native Tauri prerequisites for the target operating system

For platform packages required by Tauri, follow the official Tauri prerequisites documentation.

## Install

```bash
git clone https://github.com/notsonata/refrain.git
cd refrain
npm install
```

## Run

Full desktop application:

```bash
npm run tauri dev
```

Frontend only:

```bash
npm run dev
```

The browser-only frontend cannot execute native Tauri commands.

## Spotify developer setup

Refrain uses a user-provided Spotify application Client ID and Authorization Code with PKCE. Configure this exact redirect URI in the Spotify developer application:

```text
http://127.0.0.1:43817/callback
```

Then enter the Client ID in Refrain Settings and connect Spotify. Port `43817` must be available while authorization starts.

See `docs/reference/spotify-auth.md` for authentication behavior and the real-account smoke test.

## Local library setup

Milestone 7 adds observational local-library indexing. In **Settings → Local library**:

1. Select the **Library root** field and choose the music-library folder from the native operating-system folder picker.
2. Choose **Save path** to persist it without scanning, or **Scan library** to save and scan it.
3. Review the present, missing, invalid, and total indexed counts while/after the scan.
4. Open **Local** and choose **Local Sync** to rescan the local library, compare it with imported Spotify state, and normalize present files.

The scanner reads supported audio metadata and filesystem properties into the physical-file index. Local Sync ensures those present files have logical library-track records, links confident Spotify matches, and normalizes files using Spotify metadata when linked or local metadata otherwise. Directory symlinks are not followed. Unsupported file formats and hidden/system metadata paths are ignored.

## Spotify tracking and sync

Open **Spotify** to browse Liked Songs, saved albums, and playlists. Collections are untracked until selected. A collection-level tracking control sets the default for all its tracks, and individual songs can be included, excluded, or reset to that collection default. These choices persist across restarts and normal Spotify refreshes.

Choose **Spotify Sync** from the Spotify workspace to refresh Spotify and reconcile only the currently tracked selections with the local library. Missing untracked Spotify tracks are not queued for acquisition. Inaccessible playlists appear only in the collapsed **Unavailable** section.

## Sockseek acquisition setup

Refrain pins Sockseek `3.0.5` as its first acquisition provider. For local development, fetch the checksum-verified sidecar for the current Rust host target before running a Tauri command that validates or launches the sidecar:

```bash
npm run sidecar:sockseek
```

In **Settings → Acquisition**:

1. Use an existing Soulseek account. Sockseek does not require a separate account; Refrain runs Sockseek as a client of the Soulseek network.
2. Enter the same Soulseek username and password used by that account and choose **Save Soulseek credentials**.
3. Refrain stores the credentials in the operating system credential store. The password is not stored in SQLite.
4. Use **Check Sockseek** to start the pinned sidecar and verify the provider is ready. Sockseek 3.0.5 connects and logs in to Soulseek lazily when the first acquisition job starts, so an idle daemon normally reports no active Soulseek client yet.
5. Enable **Acquire missing tracks during synchronization** and save the acquisition setting.

The daemon binds to `127.0.0.1` on an available runtime-selected port. Refrain materializes a restricted temporary Sockseek configuration only for process startup and removes it after the daemon loads configuration.

## Common commands

| Command | Purpose |
| --- | --- |
| `npm run tauri dev` | Run the desktop app |
| `npm run dev` | Run the Vite frontend |
| `npm run build` | Build the frontend |
| `npm run tauri build -- --no-bundle` | Compile the desktop app without installers |
| `npm run sidecar:sockseek` | Fetch and verify Sockseek 3.0.5 for the current Rust host target |
| `npm run format:check` | Check frontend formatting |
| `npm run lint` | Run ESLint |
| `npm run check` | Run Svelte/TypeScript checks |
| `npm test` | Run frontend tests |
| `npm run rust:fmt` | Check Rust formatting |
| `npm run rust:clippy` | Run Clippy with warnings denied |
| `npm run rust:test` | Run Rust tests |

## Local data

Refrain creates its application-data directory through Tauri and stores SQLite state there. Spotify refresh credentials and Soulseek credentials are stored through the operating system credential store rather than SQLite. Sockseek downloads first enter Refrain-controlled staging under `runtime/acquisition/`. Application logs are written under the application's data directory in `logs/`.
