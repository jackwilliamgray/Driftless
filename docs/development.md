# Development

## Toolchain

- macOS 12 or later
- Rust toolchain (recent stable, via `rustup`)
- Node 20+ and pnpm 10+
- GitHub CLI (`gh`), authenticated with `gh auth login`
- Xcode Command Line Tools (`xcode-select --install`)

For universal (Apple Silicon + Intel) builds you also need both Rust targets:

```sh
rustup target add aarch64-apple-darwin x86_64-apple-darwin
```

## Dev loop

```sh
pnpm install
pnpm tauri dev
```

You should see a tray icon in the macOS menu bar — there is no Dock icon
because the app runs as an `Accessory` activation policy. Click the tray
icon for the popup, or double-click for the full dashboard.

Crank log verbosity with `DRIFTLESS_LOG=trace pnpm tauri dev`. Logs go to
stderr.

## Tests

```sh
cd src-tauri
cargo test --lib
```

Covers the aggregation and notification-transition logic — the only
places where pure logic is non-trivial. Feature correctness is dominantly
manual: run `pnpm tauri dev`, push a commit to a PR, and watch the icon
and popup update through the run lifecycle.

## Type checking the frontend

```sh
pnpm tsc --noEmit
```

`pnpm build` runs `tsc && vite build`, so production builds catch type
errors as well.

## Release builds

| Target | Command | Output |
| --- | --- | --- |
| Apple Silicon only | `pnpm tauri:build:arm64` | `src-tauri/target/aarch64-apple-darwin/release/bundle/` |
| Intel only | `pnpm tauri:build:x86_64` | `src-tauri/target/x86_64-apple-darwin/release/bundle/` |
| Universal (recommended for distribution) | `pnpm tauri:build:universal` | `src-tauri/target/universal-apple-darwin/release/bundle/` |
| Native arch only | `pnpm tauri:build` | `src-tauri/target/release/bundle/` |

Each produces both an `.app` (`bundle/macos/Driftless.app`) and a `.dmg`
(`bundle/dmg/Driftless_<version>_<arch>.dmg`). The bundle config in
`src-tauri/tauri.conf.json` controls icons, DMG layout, and metadata.

## Code signing & notarisation

The build is currently unsigned, so users see a Gatekeeper warning on
first launch. To ship a signed/notarised build, set the standard Tauri
env vars before running `pnpm tauri:build:universal`:

```sh
export APPLE_SIGNING_IDENTITY="Developer ID Application: <Name> (<TEAMID>)"
export APPLE_ID="<apple-id-email>"
export APPLE_PASSWORD="<app-specific-password>"
export APPLE_TEAM_ID="<TEAMID>"
```

See the Tauri docs for full options:
<https://tauri.app/distribute/sign/macos/>.

## Configuration storage

Persisted config lives in `tauri-plugin-store`'s JSON file under the
platform app-data directory:

```
~/Library/Application Support/com.jackgray.driftless/
```

The `.store` JSON files there are safe to delete to reset state.

## Known gaps

- **Tray icons.** Currently placeholder copies of the app icon. Replace
  `src-tauri/icons/tray-*.png` with proper template images (B&W with
  alpha, ~22×22 logical, 2× retina) for clean rendering in the menu bar.
- **Loading icon.** The "refreshing" tray state reuses
  `tray-pending.png`. Add a dedicated `tray-refreshing.png` and point
  `ICON_REFRESHING` in `src-tauri/src/tray.rs` at it for visual
  separation from genuine in-flight workflows.
- **Dock flicker on launch.** `LSUIElement` is set at runtime via
  `set_activation_policy(Accessory)` rather than in the bundle plist,
  causing a brief Dock-icon flash. Adding an `Info.plist` override in
  `tauri.conf.json` removes this.
- **Unsigned builds.** See the signing section above.
