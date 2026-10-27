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

## Specship coding workflow

Specship is installed for this repository through Codex. Its runner checkout and dependencies are at `.specship/runtime`; keep that directory in place while the MCP registration points to it. Project settings and saved runs remain in `.specship/`. The project uses OpenCode profile `workhorse` with `openai/gpt-6-luna`, `high` reasoning effort, a 300-second assignment limit, and a 120-second delivery margin.

After reloading the MCP connection or restarting Codex, review the changed Interrupt command with `/hooks`. Then start a task by typing `$spec` followed by the requested outcome. Spec remains the model selected in the Codex chat; Ship runs through OpenCode. The live `stop` smoke scenario passed with `openai/gpt-6-luna` high. The full scenario reached a test assertion that expected a PID missing from the OpenCode 1.19.3 runtime snapshot, so crash recovery was not verified.

## Spotify developer setup

Refrain uses a user-provided Spotify application Client ID and Authorization Code with PKCE. Configure this exact redirect URI in the Spotify developer application:

```text
http://127.0.0.1:43817/callback
```

Then enter the Client ID in Refrain Settings and connect Spotify. Port `43817` must be available while authorization starts.

See `docs/reference/spotify-auth.md` for authentication behavior and the real-account smoke test.

## Local library setup

Refrain maintains an observational local-library index. In **Settings → Local library**:

1. Select the **Library root** field and choose the music-library folder from the native operating-system folder picker.
2. Choose **Save path** to persist it without scanning, or **Scan library** to save and scan it.
3. Review the present, missing, invalid, and total indexed counts while/after the scan.
4. Open **Local** and choose **Local Sync** to rescan the local library, compare it with imported Spotify state, and normalize present files.

The scanner reads supported audio metadata and filesystem properties into the physical-file index. Local Sync ensures those present files have logical library-track records, links confident Spotify matches, and normalizes files using Spotify metadata when linked or local metadata otherwise. Directory symlinks are not followed. Unsupported file formats and hidden/system metadata paths are ignored.

## Spotify tracking and sync

Open **Spotify** to browse Liked Songs, saved albums, and playlists. Collections are untracked until selected. A collection-level tracking control sets the default for all its tracks, and individual songs can be included, excluded, or reset to that collection default. These choices persist across restarts and normal Spotify refreshes.

Choose **Spotify Sync** from the Spotify workspace to refresh Spotify and reconcile only the currently tracked selections with the local library. Missing untracked Spotify tracks are not queued for acquisition. Inaccessible playlists appear only in the collapsed **Unavailable** section.

## Missing-track acquisition setup

In **Settings → Acquisition**:

1. Enable **Acquire missing tracks** to allow tracked Spotify items without a local copy to be searched during manual synchronization. Acquisition starts disabled in a new installation.
2. Monochrome is the default provider and needs no account credentials. Use **Check Monochrome** to inspect availability.
3. Optionally add **Antra** or **Sockseek / Soulseek** to the priority list. Use **Up**, **Down**, and **Remove** to control fallback order. Provider order and the acquisition switch save immediately.
4. If using Antra, choose **Sign in with Antra**, complete its device-code approval in the browser, and use **Check Antra** after sign-in. Refrain stores the approved device token in the operating system credential store.
5. If using Sockseek, save credentials for an existing Soulseek account and use **Check Sockseek** to validate the sidecar. Sockseek is a Soulseek client and requires no separate Sockseek account. Credentials are stored in the operating system credential store, not SQLite.
6. Use **Sync Library** to run the current manual synchronization workflow. Startup and periodic synchronization are planned, not active.

For local development with Sockseek enabled, fetch its pinned checksum-verified `3.0.5` sidecar for the current Rust host target before attempting to validate or launch it:

```bash
npm run sidecar:sockseek
```

Released desktop bundles package this sidecar. The daemon binds to `127.0.0.1` on an available runtime-selected port and logs in to Soulseek lazily when its first acquisition job starts. Refrain materializes a restricted temporary Sockseek configuration only for process startup and removes it after the daemon loads configuration.

Antra sometimes delivers FLAC audio inside an MP4 container. To extract those streams, install **FFmpeg** on the machine running Refrain. macOS checks `/opt/homebrew/bin/ffmpeg` and `/usr/local/bin/ffmpeg`; otherwise Refrain looks for `ffmpeg` on `PATH` (including Windows and Linux). Native FLAC downloads need no FFmpeg. Without it, or if MP4 carries AAC rather than FLAC, Refrain reports an Antra candidate failure and tries other eligible acquisition sources. Release bundles do not currently package FFmpeg.

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

Refrain creates its application-data directory through Tauri and stores SQLite state there. Spotify refresh credentials, Antra device tokens, and Soulseek credentials are stored through the operating system credential store rather than SQLite. Acquired audio first enters Refrain-controlled staging under `runtime/acquisition/`. Application logs are written under the application's data directory in `logs/`.
