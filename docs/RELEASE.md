# Building & releasing

## Local development build

```bash
npm install
npm run app:dev                 # hot-reloading desktop app
npm run app:build               # → src-tauri/target/release/bundle/{macos,dmg}
```

Outputs (Apple Silicon): `DigitsNepal.app` (~5 MB) and `DigitsNepal_<version>_aarch64.dmg` (~2.6 MB).

For Intel Macs: `rustup target add x86_64-apple-darwin && npx tauri build --target x86_64-apple-darwin`.
For a universal binary, add both targets and use `--target universal-apple-darwin`.

> **macOS 26+ note:** `Cargo.toml` sets `[profile.release.build-override] strip = false`. Without it, stripped proc-macro dylibs fail to load ("mis-aligned LINKEDIT string pool") and release builds break.

## Unsigned builds (default)

Release builds are **not code-signed or notarized** unless you set it up. On another Mac, Gatekeeper will block the first launch:

1. Open the DMG and drag DigitsNepal to Applications.
2. Right-click DigitsNepal and choose **Open**, then confirm. Or allow it in System Settings › Privacy & Security › "Open Anyway".
3. Accessibility permission is tied to the binary's signature. Each new unsigned build may need to be re-approved in System Settings › Privacy & Security › Accessibility (remove the old entry, then add the new one).

## Signing & notarization (maintainers)

Needs an Apple Developer ID. Never commit certificates or passwords. Set them as CI secrets:

| Secret | Purpose |
|---|---|
| `APPLE_CERTIFICATE` / `APPLE_CERTIFICATE_PASSWORD` | base64 .p12 Developer ID Application cert |
| `APPLE_SIGNING_IDENTITY` | e.g. `Developer ID Application: Name (TEAMID)` |
| `APPLE_ID` / `APPLE_PASSWORD` / `APPLE_TEAM_ID` | notarization (app-specific password) |

Tauri picks these up automatically during `tauri build`. Check the result with `spctl -a -vv DigitsNepal.app` and `xcrun stapler validate`.

Do not describe a build as notarized unless those commands pass.

## Versioning

Semantic versioning. Bump `version` in `package.json`, `src-tauri/Cargo.toml` and `src-tauri/tauri.conf.json` together, update `CHANGELOG.md`, then tag `vX.Y.Z`. The `release` workflow builds the DMG and drafts a GitHub release.
