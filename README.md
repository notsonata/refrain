# Refrain

Refrain is a desktop application that treats Spotify as the desired state for a normalized local music library.

It is being built to import Spotify playlists, Liked Songs, and saved albums, reconcile them against local audio, acquire missing tracks through modular providers, normalize the resulting library, export playlists, and mirror the library to another filesystem location.

## Project Status

Refrain v0.1.0 was released on 2026-09-25, completing Milestones 1 through 6. The project remains under active development toward v1.0.0.

**Milestones 7 through 9** are implemented and merged. **Milestone 10: Filesystem Normalization and Ownership Safety** is implemented on the current development branch and is awaiting pull-request validation and merge.

For current project state, see [`docs/STATUS.md`](docs/STATUS.md).

## v0.1.0 Scope

The first release is focused on the Spotify source layer:

- desktop support for Windows, macOS, and Linux
- user-provided Spotify developer credentials
- Spotify authentication
- fetch and persist Liked Songs
- fetch and persist saved albums and their tracks
- fetch and persist playlists and playlist tracks
- browse imported Spotify state under Liked Songs, Saved Albums, and Playlists
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

On Linux, the exact packages differ by distribution. The repository CI builds on Ubuntu with WebKitGTK, AppIndicator, librsvg, and `patchelf` development dependencies installed.

See [`docs/reference/setup.md`](docs/reference/setup.md) for the maintained setup reference.

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
2. Add this exact redirect URI to the application:

   ```text
   http://127.0.0.1:43817/callback
   ```

3. Start Refrain with `npm run tauri dev`.
4. Open **Settings** and enter the Spotify Client ID.
5. Choose **Connect Spotify** and complete authorization in the system browser.
6. Choose **Refresh Spotify** to fetch and persist Liked Songs, Saved Albums, and playlist source state.
7. Browse imported tracks under **Liked Songs**, **Saved Albums**, and **Playlists**. The persisted source state is loaded again when Refrain restarts.

Refrain uses Authorization Code with PKCE and does not require a Spotify client secret. During sign-in, Refrain listens only on `127.0.0.1:43817`; if another application is already using that port, Spotify connection cannot start until the port is free.

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

GitHub Actions runs:

- frontend formatting
- ESLint
- Svelte/TypeScript checks
- frontend tests
- frontend production build
- Rust formatting, Clippy, and tests on Windows, macOS, and Linux
- native Tauri build smoke checks on Windows, macOS, and Linux

See [`docs/reference/testing.md`](docs/reference/testing.md) for the validation strategy and manual v0.1 smoke tests.

## Releases

Pushing a semantic version tag such as `v0.1.0` triggers `.github/workflows/release.yml`. The workflow verifies that the tag matches the configured application version, builds native Linux, macOS, and Windows packages, and uploads them to the matching GitHub Release.

See [`docs/reference/release.md`](docs/reference/release.md) for the release checklist and packaging details.

## Documentation

The repository documentation is the source of truth for planned behavior and architecture:

- [`docs/BRIEF.md`](docs/BRIEF.md): project purpose, goals, and release boundaries
- [`docs/SPEC.md`](docs/SPEC.md): product behavior and acceptance criteria
- [`docs/TDD.md`](docs/TDD.md): technical architecture and design decisions
- [`docs/IMPLEMENTATION.md`](docs/IMPLEMENTATION.md): implementation milestones and ordering
- [`docs/STATUS.md`](docs/STATUS.md): current project state and active work
- [`docs/reference/codebase-map.md`](docs/reference/codebase-map.md): current source navigation map
- [`docs/reference/setup.md`](docs/reference/setup.md): development setup
- [`docs/reference/testing.md`](docs/reference/testing.md): automated and manual validation
- [`docs/reference/release.md`](docs/reference/release.md): release packaging and tag workflow
- [`docs/decisions/002-saved-albums-as-source-collections.md`](docs/decisions/002-saved-albums-as-source-collections.md): saved-album source-model decision

## Development Roadmap

Milestones 1 through 9 are complete. **Milestone 10: Filesystem Normalization and Ownership Safety** is implemented on the current development branch; **Milestone 11: Issues and Manual Resolution UI** follows after Milestone 10 is validated and merged.

See [`docs/IMPLEMENTATION.md`](docs/IMPLEMENTATION.md) for the full path through v0.1.0 and v1.0.0.
