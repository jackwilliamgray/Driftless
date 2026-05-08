# Driftless

A macOS menu-bar app that keeps a steady eye on your GitHub Actions —
the runs on your own pull requests and the runs in repositories you
choose to watch — without making you tab over to GitHub to check.

The tray icon shows the worst current state across everything you
track, the popup gives a per-PR / per-repo breakdown, and the
dashboard drills into individual workflow runs and jobs.

## What it's for

- Knowing at a glance whether your PRs are green, pending, or broken.
- Catching a failed run on `main` of a watched repo without watching
  GitHub.
- Getting a macOS notification when a workflow on a tracked PR fails
  or turns all-green, instead of polling the Actions tab manually.
- Grouping repos into projects so you can keep an eye on a whole team's
  pipelines in one place.

## Requirements

- **macOS 12 (Monterey) or newer**, Apple Silicon or Intel.
- **GitHub CLI** (`gh`), installed and authenticated. Driftless reads
  your token from `gh auth token` on every poll and never stores its
  own copy. If you don't have `gh` yet:

  ```sh
  brew install gh
  gh auth login
  ```

## Install

1. Download the latest `Driftless_<version>_<arch>.dmg` from the
   [Releases](../../releases) page.
2. Open the DMG and drag **Driftless.app** into **Applications**.
3. Eject the DMG and launch Driftless from Applications, Spotlight, or
   Launchpad.

> The build is currently unsigned, so the first launch will trigger a
> Gatekeeper warning. Right-click the app in Applications and choose
> **Open** to allow it the first time, or run
> `xattr -dr com.apple.quarantine /Applications/Driftless.app`.

There's no Dock icon — Driftless lives entirely in the menu bar. Look
for its icon up next to the clock, Wi-Fi, and battery indicators.

## First-run setup

1. Make sure you've run `gh auth login` at least once. Driftless will
   surface a "Not signed in to GitHub CLI" message in the popup if it
   can't find a token.
2. Click the tray icon to open the popup. Your open pull requests
   should populate within a few seconds.
3. From the popup, click **Open dashboard…** to add watched repos and
   tune notification preferences.

To have Driftless start automatically when you log in, enable
**Settings → Start at login** in the dashboard.

## Using the app

### Tray icon

The tray icon reflects the worst aggregate state across everything you
track:

| Icon       | Meaning                                              |
| ---------- | ---------------------------------------------------- |
| Idle       | Nothing pending, nothing failing.                    |
| Pending    | At least one tracked workflow is currently running.  |
| Success    | All tracked workflows finished green.                |
| Failure    | At least one tracked workflow failed or timed out.   |
| Refreshing | Briefly shown while a poll cycle is in flight.       |

- **Click** the tray icon to toggle the popup.
- **Double-click** the tray icon, or use the menu's **Open
  Dashboard**, to surface the full dashboard window.
- **Right-click** (or click the menu arrow) for **Open Dashboard**,
  **Refresh**, and **Quit**.

### Popup

A compact list of your open pull requests and your watched repos, each
with their aggregate status and the most recent runs as small chips.
Clicking a PR row opens it on github.com; clicking a watched repo opens
its Actions tab.

The **Refresh** button forces an immediate poll. While polling, both
the button and the tray icon show a loading state.

### Dashboard

The full window has three sections:

- **My pull requests** — every open PR you authored, with its
  workflow runs grouped underneath. Expand a run to see its jobs and
  per-step status, or click through to the run on github.com.
- **Watched repos** — repos you've explicitly added, grouped by
  project. Within each project, repos are sorted by most-recent
  workflow activity (alphabetical fallback for repos with no recent
  runs). Use the project headers to collapse groups you don't care
  about right now.
- **Settings** — see below.

## Configuration

All configuration lives in **Dashboard → Settings**:

- **Watched repos.** Add a repo as `owner/repo`, optionally with a
  branch filter (e.g. `main`) to narrow the runs that count toward its
  status. You can also point Driftless at a local folder and it will
  scan for git repositories with GitHub remotes to bulk-add.
- **Excluded workflows.** Suppress noisy or irrelevant workflows
  globally or per-repo so they don't poison your aggregate state.
- **Projects.** Group watched repos under a named project — either by
  explicit membership or by name prefix (e.g. `team-platform-`).
  Projects can be toggled on/off to temporarily hide a whole group.
- **Notifications.** Pick which transitions fire macOS notifications:
  failures, first all-green after a failure, or every transition.
- **Start at login.** Toggle Driftless launching automatically when
  you log in to macOS.

Settings persist locally in
`~/Library/Application Support/com.jackgray.driftless/`.

## Troubleshooting

- **"Not signed in to GitHub CLI"** — run `gh auth login` in a
  terminal, then click **Refresh** in the popup. No restart needed.
- **Status hasn't updated.** The poll interval is 60 s when everything
  is stable, 15 s when something is in flight. Click **Refresh** to
  force an immediate poll.
- **Rate-limited.** The dashboard sidebar shows your remaining GitHub
  REST quota. If it's low, reduce the number of watched repos or
  remove broad branch filters; Driftless uses ETags to keep usage low,
  but watching dozens of busy repos is still going to spend requests.
- **Icon never changes.** Confirm the tray icon you see is actually
  Driftless (it lives in the right side of the menu bar) and that
  `gh auth status` succeeds.
- **Reset everything.** Quit the app, delete
  `~/Library/Application Support/com.jackgray.driftless/`, and
  relaunch.

## Development

If you want to build from source, contribute, or understand how it
works internally:

- [`docs/development.md`](docs/development.md) — toolchain,
  dev/build/release commands, signing.
- [`docs/architecture.md`](docs/architecture.md) — auth, polling,
  data flow, layout.

## License

Driftless is released under the
[GNU General Public License v3.0 or later](LICENSE).
