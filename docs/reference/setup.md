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

## Common commands

| Command | Purpose |
| --- | --- |
| `npm run tauri dev` | Run the desktop app |
| `npm run dev` | Run the Vite frontend |
| `npm run build` | Build the frontend |
| `npm run tauri build -- --no-bundle` | Compile the desktop app without installers |
| `npm run format:check` | Check frontend formatting |
| `npm run lint` | Run ESLint |
| `npm run check` | Run Svelte/TypeScript checks |
| `npm test` | Run frontend tests |
| `npm run rust:fmt` | Check Rust formatting |
| `npm run rust:clippy` | Run Clippy with warnings denied |
| `npm run rust:test` | Run Rust tests |

## Local data

Refrain creates its application-data directory through Tauri and stores SQLite state there. Spotify refresh credentials are stored through the operating system credential store rather than SQLite. Application logs are written under the application's data directory in `logs/`.
