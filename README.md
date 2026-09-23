# Refrain

Refrain is a desktop application that treats Spotify as the desired state for a normalized local music library.

It is being built to import Spotify playlists and Liked Songs, reconcile them against local audio, acquire missing tracks through modular providers, normalize the resulting library, export playlists, and mirror the library to another filesystem location.

## Project Status

Refrain is under active development and does not have a released version yet.

Milestones 1 through 3 of the v0.1.0 development line are implemented:

- Tauri 2 + Svelte 5 + TypeScript desktop application foundation
- SQLite persistence for application settings and v0.1 source state
- user-provided Spotify Client ID configuration
- Spotify Authorization Code with PKCE
- loopback OAuth callback on `127.0.0.1`
- secure refresh-credential storage through the operating system credential store
- in-memory Spotify access tokens with refresh and reconnect behavior

Spotify source synchronization is the next milestone. Liked Songs and playlist fetching are not implemented on `main` yet.

For current project state, see [`docs/STATUS.md`](docs/STATUS.md).

## v0.1.0 Scope

The first release is focused on the Spotify source layer:

- desktop support for Windows, macOS, and Linux
- user-provided Spotify developer credentials
- Spotify authentication
- fetch and persist Liked Songs
- fetch and persist playlists and playlist tracks
- browse imported Spotify state
- manual refresh

Downloading, local-library reconciliation, matching, normalization, playlist export, and mirroring are planned for v1.0.0 rather than v0.1.0.

## Tech Stack

- [Tauri 2](https://v2.tauri.app/)
- Rust
- Svelte 5
- TypeScript
- Vite
- Tailwind CSS 4
- SQLite
- npm

## Prerequisites

Install the following before running Refrain locally:

- Git
- Node.js `>=22.12.0`
- npm
- Rust `>=1.95`
- the native system dependencies required by Tauri for your operating system

Tauri maintains the platform-specific prerequisite instructions here:

https://v2.tauri.app/start/prerequisites/

On Linux, the exact packages differ by distribution. The repository CI currently builds on Ubuntu with WebKitGTK, AppIndicator, librsvg, and `patchelf` development dependencies installed.

## Run Locally

Clone the repository:

```bash
git clone https://github.com/notsonata/refrain.git
cd refrain
```

Install JavaScript dependencies:

```bash
npm install
```

Start the desktop application in development mode:

```bash
npm run tauri dev
```

This starts the Vite development server and opens the Tauri desktop window.

To run only the frontend in a browser without the native backend:

```bash
npm run dev
```

Some application behavior depends on Tauri commands and will not work in browser-only mode.

## Spotify Setup

Refrain does not ship shared Spotify credentials. Each user supplies their own Spotify application Client ID.

1. Create a Spotify developer application.
2. Add this redirect URI to the application:

   ```text
   http://127.0.0.1/callback
   ```

3. Start Refrain with `npm run tauri dev`.
4. Enter the Spotify Client ID in the application.
5. Choose **Connect Spotify** and complete authorization in the system browser.

Refrain uses Authorization Code with PKCE and does not require a Spotify client secret.

See [`docs/reference/spotify-auth.md`](docs/reference/spotify-auth.md) for credential handling details and the manual authentication smoke test.

## Development Commands

| Command | Purpose |
| --- | --- |
| `npm run tauri dev` | Run the full desktop app in development mode |
| `npm run dev` | Run only the Vite frontend |
| `npm run build` | Build the frontend |
| `npm run tauri build -- --no-bundle` | Compile the desktop application without creating installer bundles |
| `npm run format` | Format frontend source and configuration |
| `npm run format:check` | Check frontend formatting |
| `npm run lint` | Run ESLint |
| `npm run check` | Run Svelte and TypeScript checks |
| `npm test` | Run frontend tests |
| `npm run test:watch` | Run frontend tests in watch mode |
| `npm run rust:fmt` | Check Rust formatting |
| `npm run rust:clippy` | Run Rust clippy with warnings denied |
| `npm run rust:test` | Run Rust tests |

## Validation

The GitHub Actions workflow currently runs:

- frontend formatting
- ESLint
- Svelte/TypeScript checks
- frontend tests
- frontend production build
- Rust formatting, clippy, and tests on Windows, macOS, and Linux
- a native Tauri build smoke test on Linux

## Documentation

The repository documentation is the source of truth for planned behavior and architecture:

- [`docs/BRIEF.md`](docs/BRIEF.md): project purpose, goals, and release boundaries
- [`docs/SPEC.md`](docs/SPEC.md): product behavior and acceptance criteria
- [`docs/TDD.md`](docs/TDD.md): technical architecture and design decisions
- [`docs/IMPLEMENTATION.md`](docs/IMPLEMENTATION.md): implementation milestones and ordering
- [`docs/STATUS.md`](docs/STATUS.md): current project state and next work

## Development Roadmap

The immediate next milestone is **Milestone 4: Spotify Source Synchronization**, which adds profile retrieval, Liked Songs, playlist and playlist-item pagination, transactional source refresh, retry/rate-limit handling, refresh progress, and cancellation where practical.

See [`docs/IMPLEMENTATION.md`](docs/IMPLEMENTATION.md) for the full path through v0.1.0 and v1.0.0.
