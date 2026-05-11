use anyhow::Result;
use chrono::Utc;
use std::time::Duration;
use tauri::{AppHandle, Emitter};
use tokio::time::{sleep, Instant};
use tracing::{error, info, warn};

use crate::auth;
use crate::github::client::FetchOutcome;
use crate::github::{graphql, rest, types::*, GitHubClient};
use crate::notify::{dispatch_transitions, PollSnapshot};
use crate::state::{aggregate_runs, merge_aggregates, AggregateState, SharedState};
use crate::store;
use crate::tray;

const POLL_ACTIVE: Duration = Duration::from_secs(15);
const POLL_IDLE: Duration = Duration::from_secs(60);
const POLL_BACKOFF_MAX: Duration = Duration::from_secs(300);

pub fn spawn(app: AppHandle, state: SharedState, client: GitHubClient) {
    tauri::async_runtime::spawn(async move {
        let mut backoff = Duration::from_secs(0);
        loop {
            // Refresh auth on each cycle. Cheap, and lets the user `gh auth
            // login` mid-session without restarting the app.
            let auth_status = auth::current_status().await;
            let prev_auth = state.snapshot.read().auth.clone();
            if prev_auth.logged_in != auth_status.logged_in
                || prev_auth.login != auth_status.login
            {
                let _ = app.emit("auth:changed", &auth_status);
            }
            if auth_status.logged_in {
                if let Ok(Some(tok)) = auth::read_gh_token().await {
                    client.set_token(Some(tok));
                }
            } else {
                client.set_token(None);
            }

            // Signal the start of a poll cycle so the UI can show a loading
            // state and the tray can switch to the refreshing icon.
            let _ = app.emit("poll:started", &Utc::now().to_rfc3339());
            tray::set_refreshing(&app);

            // Snapshot intent: write auth, attempt poll, then write everything.
            let mut next_interval = POLL_IDLE;
            if client.has_token() {
                match poll_once(&app, &client, &state).await {
                    Ok(snapshot) => {
                        backoff = Duration::from_secs(0);
                        // Run transition diff for notifications before the new
                        // snapshot replaces last_run_conclusions.
                        let prev_prs = state.snapshot.read().prs.clone();
                        let diff_input = PollSnapshot {
                            prs: &snapshot.prs,
                            watched: &snapshot.watched,
                            prs_prev: &prev_prs,
                        };
                        if let Err(e) = dispatch_transitions(&app, &state, &diff_input).await {
                            warn!(error = %e, "notification dispatch failed");
                        }
                        if any_active(&snapshot) {
                            next_interval = POLL_ACTIVE;
                        }
                        let mut s = state.snapshot.write();
                        s.auth = auth_status.clone();
                        s.prs = snapshot.prs;
                        s.watched = snapshot.watched;
                        s.aggregate = snapshot.aggregate;
                        s.last_polled_at = Some(Utc::now().to_rfc3339());
                        s.next_poll_at = Some(
                            (Utc::now() + chrono::Duration::from_std(next_interval).unwrap())
                                .to_rfc3339(),
                        );
                        s.rate_limit_remaining = snapshot.rate_limit_remaining;
                        s.global_excluded_workflows = snapshot.global_excluded_workflows;
                        s.projects = state.config.read().projects.clone();
                        let snap = s.clone();
                        drop(s);
                        let _ = app.emit("runs:updated", &snap);
                        tray::update_icon(&app, snap.aggregate);
                    }
                    Err(e) => {
                        error!(error = %e, "poll failed");
                        let _ = app.emit(
                            "poll:error",
                            &serde_json::json!({
                                "message": format!("{e}"),
                                "at": Utc::now().to_rfc3339(),
                            }),
                        );
                        // Restore the tray icon to the last-known aggregate so
                        // the refreshing indicator doesn't persist.
                        tray::update_icon(&app, state.snapshot.read().aggregate);
                        backoff = next_backoff(backoff);
                        next_interval = backoff;
                    }
                }
            } else {
                let mut s = state.snapshot.write();
                s.auth = auth_status.clone();
                s.aggregate = AggregateState::Idle;
                let snap = s.clone();
                drop(s);
                let _ = app.emit("runs:updated", &snap);
                tray::update_icon(&app, AggregateState::Idle);
            }

            // Wait for either the interval or a force-refresh wakeup.
            let deadline = Instant::now() + next_interval;
            let wakeup = state.wakeup.clone();
            tokio::select! {
                _ = sleep(deadline.saturating_duration_since(Instant::now())) => {}
                _ = wakeup.notified() => {
                    info!("poller woken early");
                }
            }
        }
    });
}

fn next_backoff(prev: Duration) -> Duration {
    let next = if prev.is_zero() {
        Duration::from_secs(5)
    } else {
        prev * 2
    };
    next.min(POLL_BACKOFF_MAX)
}

fn any_active(snap: &PollResult) -> bool {
    snap.prs.iter().any(|p| matches!(p.aggregate, AggregateState::Pending))
        || snap
            .watched
            .iter()
            .any(|w| matches!(w.aggregate, AggregateState::Pending))
}

#[derive(Debug)]
struct PollResult {
    prs: Vec<PullRequestSummary>,
    watched: Vec<WatchedRepoState>,
    aggregate: AggregateState,
    rate_limit_remaining: Option<u32>,
    global_excluded_workflows: Vec<String>,
}

async fn poll_once(
    app: &AppHandle,
    client: &GitHubClient,
    state: &SharedState,
) -> Result<PollResult> {
    let global_excluded = state.config.read().global_excluded_workflows.clone();

    // 1. Open PRs from viewer.
    let mut prs = Vec::new();
    let viewer_prs = graphql::viewer_open_prs(client).await?;
    for pr in &viewer_prs {
        let runs = match rest::list_repo_runs(client, &pr.repo, Some(&pr.head_sha), None, 10).await {
            Ok(FetchOutcome::Fresh(runs)) => filter_runs(runs, &[&global_excluded]),
            Ok(FetchOutcome::NotModified) => prior_runs_for_pr(state, &pr.repo, pr.number),
            Err(e) => {
                warn!(repo = %format!("{}/{}", pr.repo.owner, pr.repo.name), error = %e, "PR runs fetch failed");
                Vec::new()
            }
        };
        let agg = aggregate_runs(runs.iter());
        prs.push(PullRequestSummary {
            repo: pr.repo.clone(),
            number: pr.number,
            title: pr.title.clone(),
            branch: pr.branch.clone(),
            head_sha: pr.head_sha.clone(),
            url: pr.url.clone(),
            is_draft: pr.is_draft,
            state: pr.state.clone(),
            runs,
            aggregate: agg,
        });
    }

    // 2. Recently-closed PRs with in-flight runs.
    let since = (Utc::now() - chrono::Duration::days(2))
        .format("%Y-%m-%d")
        .to_string();
    if let Ok(closed) = rest::recent_closed_prs(client, &since).await {
        for c in closed {
            // Skip if the PR is already in our open list — search doesn't
            // overlap, but be defensive.
            if prs
                .iter()
                .any(|p| p.repo == c.repo && p.number == c.number)
            {
                continue;
            }
            // We need a head_sha for filtering — fetch the PR to get it.
            // Cheap because closed PRs are rare and bounded.
            let path = format!("/repos/{}/{}/pulls/{}", c.repo.owner, c.repo.name, c.number);
            let head_sha = match client
                .get_json::<serde_json::Value>(&path)
                .await
                .ok()
                .and_then(|v| {
                    v.get("head")
                        .and_then(|h| h.get("sha"))
                        .and_then(|s| s.as_str())
                        .map(|s| s.to_string())
                }) {
                Some(s) => s,
                None => continue,
            };
            let runs = match rest::list_repo_runs(client, &c.repo, Some(&head_sha), None, 10).await {
                Ok(FetchOutcome::Fresh(runs)) => filter_runs(runs, &[&global_excluded]),
                _ => continue,
            };
            // Drop if no run is still in-flight.
            let any_in_flight = runs
                .iter()
                .any(|r| matches!(r.status.as_str(), "queued" | "in_progress" | "waiting"));
            if !any_in_flight {
                continue;
            }
            let agg = aggregate_runs(runs.iter());
            prs.push(PullRequestSummary {
                repo: c.repo,
                number: c.number,
                title: c.title,
                branch: String::new(),
                head_sha,
                url: c.url,
                is_draft: c.is_draft,
                state: c.state,
                runs,
                aggregate: agg,
            });
        }
    }

    // 3. Watched repos.
    let watched_cfg = state.config.read().watched.clone();
    let mut watched = Vec::new();
    let mut cleared_dismissals: Vec<(String, String)> = Vec::new();
    for w in watched_cfg {
        let runs = match rest::list_repo_runs(
            client,
            &RepoRef {
                owner: w.owner.clone(),
                name: w.name.clone(),
            },
            None,
            w.branch_filter.as_deref(),
            10,
        )
        .await
        {
            Ok(FetchOutcome::Fresh(runs)) => {
                filter_runs(runs, &[&global_excluded, &w.excluded_workflows])
            }
            Ok(FetchOutcome::NotModified) => prior_runs_for_watched(state, &w.owner, &w.name),
            Err(e) => {
                warn!(repo = %format!("{}/{}", w.owner, w.name), error = %e, "watched runs fetch failed");
                Vec::new()
            }
        };
        let agg = aggregate_runs(runs.iter());
        // Clear dismissal as soon as a new run appears (id > dismissed_until).
        let dismissed = match w.dismissed_until_run_id {
            Some(until) => {
                let any_newer = runs.iter().any(|r| r.id > until);
                if any_newer {
                    cleared_dismissals.push((w.owner.clone(), w.name.clone()));
                    false
                } else {
                    true
                }
            }
            None => false,
        };
        watched.push(WatchedRepoState {
            repo: RepoRef {
                owner: w.owner,
                name: w.name,
            },
            branch_filter: w.branch_filter,
            recent_runs: runs,
            aggregate: agg,
            excluded_workflows: w.excluded_workflows,
            dismissed,
        });
    }

    if !cleared_dismissals.is_empty() {
        let cfg_snapshot = {
            let mut cfg = state.config.write();
            for (owner, name) in &cleared_dismissals {
                if let Some(slot) = cfg
                    .watched
                    .iter_mut()
                    .find(|w| &w.owner == owner && &w.name == name)
                {
                    slot.dismissed_until_run_id = None;
                }
            }
            cfg.clone()
        };
        if let Err(e) = store::save_config(app, &cfg_snapshot) {
            warn!(error = %e, "failed to persist cleared dismissals");
        }
    }

    let aggregate = merge_aggregates(
        prs.iter()
            .map(|p| p.aggregate)
            .chain(
                watched
                    .iter()
                    .filter(|w| !w.dismissed)
                    .map(|w| w.aggregate),
            ),
    );

    let rate_limit = state.snapshot.read().rate_limit_remaining;

    Ok(PollResult {
        prs,
        watched,
        aggregate,
        rate_limit_remaining: rate_limit,
        global_excluded_workflows: global_excluded,
    })
}

fn prior_runs_for_pr(state: &SharedState, repo: &RepoRef, number: u64) -> Vec<WorkflowRun> {
    state
        .snapshot
        .read()
        .prs
        .iter()
        .find(|p| &p.repo == repo && p.number == number)
        .map(|p| p.runs.clone())
        .unwrap_or_default()
}

fn filter_runs(runs: Vec<WorkflowRun>, exclusion_lists: &[&[String]]) -> Vec<WorkflowRun> {
    if exclusion_lists.iter().all(|l| l.is_empty()) {
        return runs;
    }
    runs.into_iter()
        .filter(|r| !is_excluded(r, exclusion_lists))
        .collect()
}

fn is_excluded(run: &WorkflowRun, exclusion_lists: &[&[String]]) -> bool {
    let candidates: [&str; 2] = [run.workflow_name.as_str(), run.name.as_str()];
    for list in exclusion_lists {
        for excluded in list.iter() {
            let needle = excluded.trim();
            if needle.is_empty() {
                continue;
            }
            for cand in candidates.iter() {
                if cand.eq_ignore_ascii_case(needle) {
                    return true;
                }
            }
        }
    }
    false
}

fn prior_runs_for_watched(state: &SharedState, owner: &str, name: &str) -> Vec<WorkflowRun> {
    state
        .snapshot
        .read()
        .watched
        .iter()
        .find(|w| w.repo.owner == owner && w.repo.name == name)
        .map(|w| w.recent_runs.clone())
        .unwrap_or_default()
}

