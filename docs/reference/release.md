# Release Process

Refrain publishes desktop packages through `.github/workflows/release.yml` when a semantic-version tag is pushed.

## Version sources

The release tag must match the application version in all three release metadata sources:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

For example, version `0.9.0` requires the matching tag:

```text
v0.9.0
```

The workflow fails before packaging when the tag and configured versions disagree or the matching `CHANGELOG.md` entry lacks a non-empty `### What's New` section. Write that section as a short user-facing digest grouped under headings such as `#### New features`, `#### Improvements`, and `#### Fixes`. The release body comes from this section; GitHub's commit-generated notes are disabled.

## Automatic packaging

A pushed `v*.*.*` tag triggers builds for:

- Linux x86_64: `.deb` and AppImage
- macOS Apple Silicon: `.app` and `.dmg`
- macOS Intel: `.app` and `.dmg`
- Windows x86_64: NSIS and MSI installers

`tauri-apps/tauri-action@v1` uploads the generated bundles to the GitHub Release associated with the tag. Its release body comes from the matching `What's New` digest in `CHANGELOG.md` and does not include commit history.

Each packaging job downloads the official Sockseek `3.0.5` archive for its target, verifies the pinned SHA-256 digest, and installs the executable using Tauri's target-triple sidecar naming before bundling. The sidecar binary itself is not tracked in Git.

Release bundles also include `THIRD_PARTY_NOTICES.md` and the upstream Sockseek AGPL-3.0 license. The notices identify the exact bundled version and corresponding upstream source.

FFmpeg is not bundled. Antra can stage native FLAC without it, but Antra MP4 containers containing FLAC require a separately installed `ffmpeg` executable. Validate MP4 extraction and missing-FFmpeg fallback on each supported operating system before advertising this path as fully portable.

The macOS bundle uses an ad-hoc signing identity by default so CI builds remain usable for development and testing without private signing credentials. Public distribution should use proper platform signing and macOS notarization when credentials are available.

## Before tagging

1. Confirm the intended version is set consistently in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`.
2. Add the release entry to `CHANGELOG.md`, including a concise, user-facing `### What's New` section.
3. Confirm CI passes on the release commit.
4. Complete the manual checks in `docs/reference/testing.md`, including Spotify connect/refresh, restart hydration, clean first run, log-secret inspection, and the real Soulseek acquisition smoke test when acquisition is included in the release.
5. Confirm application metadata and icons are the intended release assets.

## Tagging

After the release commit is merged to `main`, create and push the matching version tag. Replace `vX.Y.Z` with the version in the release metadata sources:

```bash
git checkout main
git pull --ff-only
git tag vX.Y.Z
git push origin vX.Y.Z
```

Do not reuse or move a published release tag. If a release needs correction after publication, bump the application version and create a new tag.

## Signing and notarization

The automated workflow does not require private signing credentials. When public release credentials are available, configure the appropriate GitHub Actions secrets and Tauri signing settings for Windows and macOS. Local unsigned development builds must remain possible.
