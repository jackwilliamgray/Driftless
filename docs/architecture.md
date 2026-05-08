# Architecture

How Driftless works under the hood.

## Stack

Tauri 2 + React 19 + TypeScript + Vite. A single Rust process owns the GitHub
poll loop, tray icon, and notifications; two webview windows (the tray popup
and the dashboard) consume the same React bundle and route by `window.label`.

## Auth

`src-tauri/src/auth.rs` shells out to `gh auth token` on every poll cycle and
validates the token via `GET /user`. No token is persisted by the app — the
GitHub CLI is the source of truth, so `gh auth logout` / `gh auth login`
mid-session is picked up automatically on the next poll.

## Polling

A single `tokio` task in `src-tauri/src/poller.rs` owns the poll loop:

- **Adaptive interval.** 15 s while any tracked workflow run is `pending`,
  60 s when everything is stable.
- **Backoff.** Exponential up to 5 minutes on consecutive errors.
- **Force-refresh.** A `tokio::sync::Notify` short-circuits the sleep when the
  user clicks Refresh or changes config.
- **Loading signal.** At the top of every cycle the poller emits a
  `poll:started` event and switches the tray icon to a "refreshing"
  variant; on success/error it restores to the aggregate icon.
- **ETags.** Workflow-runs requests send `If-None-Match`; on 304 the prior
  run list is reused. Keeps rate-limit usage low under the 60 s idle
  cadence.

## Data

- **Open PRs** — single GraphQL `viewer.pullRequests` query
  (`src-tauri/src/github/graphql.rs`).
- **Recently-closed PRs with in-flight runs** — bounded REST search over
  the last two days, so a merged PR with a still-running deploy stays
  visible until the runs settle.
- **Workflow runs** — REST `/repos/{owner}/{repo}/actions/runs`, filtered
  by head SHA for PRs and optionally by branch for watched repos.
- **Jobs** — lazy-loaded only when the dashboard expands a run row.

## Aggregation

`state::aggregate_runs` collapses a run list into one of
`{idle, pending, success, failure}`. For each `workflow_id` only the
most recent run is considered, so a re-run that succeeded supersedes
its earlier failure. `merge_aggregates` then folds across PRs and
watched repos for the global tray state.

## Tray icon

`src-tauri/src/tray.rs` swaps the tray PNG between
`tray-{idle,pending,success,failure}.png` based on aggregate state, and
shows the pending icon during in-flight poll cycles. Icons are rendered
as macOS template images (`icon_as_template(true)`) so they adapt to
dark/light menu bars.

Left-click toggles the popup window (positioned by
`tauri-plugin-positioner` to the tray-icon coordinates); double-click or
the menu's "Open Dashboard" surfaces the full dashboard.

## Notifications

`src-tauri/src/notify.rs` diffs the previous poll's run-id → conclusion
map against the new one and fires macOS notifications on transitions:

- a workflow turning to `failure` / `timed_out`
- a PR turning all-green when at least one run had previously failed

Suppressed on cold start (no baseline yet) so the first poll after launch
doesn't spam notifications for state the user already knew about.

## Persistence

`tauri-plugin-store` writes a single JSON blob into the platform app-data
directory. Persisted:

- watched repos (with optional branch filter and per-repo workflow excludes)
- projects (groupings of repos by name prefix or explicit membership)
- global excluded workflows
- notification preferences
- last-known run-id → conclusion map (so the transition diff survives restart)

## Frontend layout

```
src/
├── main.tsx                  # routes by window label → Popup or Dashboard
├── popup/                    # tray dropdown UI
├── dashboard/                # full dashboard + Settings
├── components/StatusIcon.tsx # shared aggregate-state glyph
├── lib/
│   ├── invoke.ts             # typed wrappers around #[tauri::command]
│   ├── events.ts             # listeners for runs:updated, poll:started, etc.
│   ├── projects.ts           # repo-to-project resolution
│   └── types.ts              # shared TS shapes mirroring Rust types
└── styles/app.css
```

Both windows load `index.html`; `main.tsx` reads the Tauri window label
and mounts `<Popup>` or `<Dashboard>` accordingly.

## Backend layout

```
src-tauri/
├── tauri.conf.json           # two windows: tray-popup, dashboard
├── capabilities/default.json # plugin permissions
├── icons/tray-{idle,pending,success,failure}.png
└── src/
    ├── lib.rs                # setup(): tray, plugins, poller spawn
    ├── auth.rs               # gh token shellout
    ├── github/               # GraphQL + REST client
    ├── poller.rs             # adaptive tokio loop
    ├── tray.rs               # icon + popup positioning
    ├── notify.rs             # transition diff → macOS notifications
    ├── store.rs              # config + last-known-conclusion persistence
    ├── commands.rs           # #[tauri::command] surface
    ├── scan.rs               # local-disk repo discovery for "Add" UI
    └── state.rs              # shared app state, aggregation logic
```
