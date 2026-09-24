# Release Process

Refrain publishes desktop packages through `.github/workflows/release.yml` when a semantic-version tag is pushed.

## Version sources

The release tag must match the application version in all three release metadata sources:

- `package.json`
- `src-tauri/tauri.conf.json`
- `src-tauri/Cargo.toml`

For v0.1.0, the expected tag is:

```text
v0.1.0
```

The workflow fails before packaging when the tag and configured versions disagree.

## Automatic packaging

A pushed `v*.*.*` tag triggers builds for:

- Linux x86_64: `.deb` and AppImage
- macOS Apple Silicon: `.app` and `.dmg`
- macOS Intel: `.app` and `.dmg`
- Windows x86_64: NSIS and MSI installers

`tauri-apps/tauri-action@v1` uploads the generated bundles to the GitHub Release associated with the tag and asks GitHub to generate release notes.

The macOS bundle uses an ad-hoc signing identity by default so CI builds remain usable for development and testing without private signing credentials. Public distribution should use proper platform signing and macOS notarization when credentials are available.

## Before tagging

1. Confirm the intended version is set consistently in `package.json`, `src-tauri/tauri.conf.json`, and `src-tauri/Cargo.toml`.
2. Update `CHANGELOG.md` from `Unreleased` to the release date.
3. Confirm CI passes on the release commit.
4. Complete the manual checks in `docs/reference/testing.md`, including Spotify connect/refresh, restart hydration, clean first run, and log-secret inspection.
5. Confirm application metadata and icons are the intended release assets.

## Tagging

After the release commit is merged to `main`, create and push the matching version tag. For v0.1.0:

```bash
git checkout main
git pull --ff-only
git tag v0.1.0
git push origin v0.1.0
```

Do not reuse or move a published release tag. If a release needs correction after publication, bump the application version and create a new tag.

## Signing and notarization

The automated workflow does not require private signing credentials. When public release credentials are available, configure the appropriate GitHub Actions secrets and Tauri signing settings for Windows and macOS. Local unsigned development builds must remain possible.
