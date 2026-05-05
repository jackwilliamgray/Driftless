# Driftless

A macOS menu-bar app for tracking GitHub Actions: the runs on your own pull
requests plus runs in repos you choose to watch. Tray icon shows aggregate
state at a glance, click to open a popup with per-PR/per-repo status, double
click for a full dashboard with per-job breakdown.

Stack: Tauri 2 + React 19 + TypeScript + Vite.

## Prerequisites

- macOS 12+
- Rust toolchain (`rustup` recent stable)
- Node 20+, pnpm 10+
- GitHub CLI (`gh`) — auth is read fresh from `gh auth token` on every poll.
  Sign in once with `gh auth login`; no app-side token storage.

## Run

```sh
pnpm install
pnpm tauri dev
```

You should see a tray icon in the macOS menu bar (no Dock icon — the app
runs as an `Accessory` activation policy). Click the tray icon to toggle the
popup; double-click or use the popup's "Open dashboard…" button for the full
window.

## How it works

- **Auth.** `src-tauri/src/auth.rs` shells out to `gh auth token` on each poll
  and validates the token via `GET /user`. No token is persisted.
- **Polling.** A single `tokio` task in `src-tauri/src/poller.rs` owns the
  GitHub poll loop. Adaptive interval: 15 s when any tracked run is active,
  60 s when everything is stable, exponential backoff on errors. ETags on
  workflow-runs requests to keep rate-limit usage low.
- **Data.** Open PRs come from a single GraphQL `viewer.pullRequests` query
  (`src-tauri/src/github/graphql.rs`); workflow runs from REST
  (`/repos/{owner}/{repo}/actions/runs`); jobs lazy-loaded only when the
  dashboard expands a run row.
- **Tray icon.** `src-tauri/src/tray.rs` swaps the tray PNG between
  `tray-{idle,pending,success,failure}.png` based on aggregate state. Icons
  are rendered as macOS template images so they adapt to dark/light menu bars.
- **Notifications.** `src-tauri/src/notify.rs` diffs the previous
  poll's run-id → conclusion map against the new one. Fires macOS
  notifications on transition to failure and on a PR turning all-green.
  Suppressed on cold start (no baseline yet).
- **Persistence.** Watched repos, notification prefs, and the last-known
  conclusion map live in `tauri-plugin-store`'s JSON file in the app data
  directory.

## Project layout

```
.
├── index.html                       # single HTML, both windows load it
├── src/                             # React frontend
│   ├── main.tsx                     # window-label routing → Popup or Dashboard
│   ├── popup/                       # tray dropdown UI
│   ├── dashboard/                   # full dashboard + Settings
│   ├── components/StatusIcon.tsx
│   ├── lib/                         # invoke / event wrappers, types
│   └── styles/app.css
└── src-tauri/
    ├── tauri.conf.json              # two windows: tray-popup, dashboard
    ├── capabilities/default.json    # plugin permissions for both windows
    ├── icons/tray-{idle,pending,success,failure}.png
    └── src/
        ├── lib.rs                   # setup(): tray, plugins, poller
        ├── auth.rs                  # gh token shellout
        ├── github/                  # API client, GraphQL + REST
        ├── poller.rs                # adaptive tokio loop
        ├── tray.rs                  # tray icon + popup positioning
        ├── notify.rs                # transition diff + notifications
        ├── store.rs                 # config persistence
        ├── commands.rs              # #[tauri::command] surface
        └── state.rs                 # shared app state, aggregation
```

## Tests

`cargo test --lib` runs the unit suite for the aggregation and
notification-transition logic — the only places where pure logic is non-trivial.

Feature correctness is dominantly a manual job: does the tray icon match
reality? Run `pnpm tauri dev`, push a commit to a PR, watch the icon cycle.

## Configuration

Open the dashboard → Settings to:

- add/remove watched repos (format `owner/repo`, optional branch filter)
- toggle which notification transitions fire

There are no command-line flags. Token comes from `gh`. Logs go to stderr;
crank verbosity with `DRIFTLESS_LOG=trace pnpm tauri dev`.

## Known v1 gaps

- Tray icons are placeholder copies of the app icon. Replace
  `src-tauri/icons/tray-*.png` with proper template images (B&W with alpha,
  ~22×22 logical, 2× retina) for them to render correctly in the menu bar.
- LSUIElement is set at runtime via `set_activation_policy(Accessory)` rather
  than in the bundle plist, so there's a brief Dock-icon flicker on launch.
  For a polished release, add an `Info.plist` override.
- No auto-launch; install via `tauri-plugin-autostart` if wanted.
- App is unsigned — Gatekeeper warning on first launch outside `tauri dev`.
