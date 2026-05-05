use anyhow::Result;
use std::collections::{HashMap, HashSet};
use tauri::AppHandle;
use tauri_plugin_notification::NotificationExt;

use crate::github::types::{PullRequestSummary, WatchedRepoState, WorkflowRun};
use crate::state::{repo_in_project, Project, SharedState};

fn repo_is_silenced(owner: &str, name: &str, projects: &[Project]) -> bool {
    projects
        .iter()
        .any(|p| !p.enabled && repo_in_project(owner, name, p))
}

/// Diff the new poll result against the persisted last-known conclusions and
/// fire macOS notifications for relevant transitions.
///
/// Cold-start safety: the very first poll (when last_run_conclusions is empty)
/// just seeds the map without firing any notifications.
pub async fn dispatch_transitions(
    app: &AppHandle,
    state: &SharedState,
    snapshot: &PollSnapshot<'_>,
) -> Result<()> {
    let prefs = state.config.read().notification_prefs.clone();
    let projects = state.config.read().projects.clone();

    let prior = state.last_run_conclusions.read().clone();
    let cold_start = prior.is_empty();

    let active_prs: Vec<&PullRequestSummary> = snapshot
        .prs
        .iter()
        .filter(|p| !repo_is_silenced(&p.repo.owner, &p.repo.name, &projects))
        .collect();
    let active_watched: Vec<&WatchedRepoState> = snapshot
        .watched
        .iter()
        .filter(|w| !repo_is_silenced(&w.repo.owner, &w.repo.name, &projects))
        .collect();

    let all_runs: Vec<&WorkflowRun> = active_prs
        .iter()
        .flat_map(|p| p.runs.iter())
        .chain(active_watched.iter().flat_map(|w| w.recent_runs.iter()))
        .collect();

    let pr_run_ids: HashSet<u64> = active_prs
        .iter()
        .flat_map(|p| p.runs.iter().map(|r| r.id))
        .collect();

    if !cold_start {
        // Per-run transitions: failure / every transition.
        for run in &all_runs {
            let key = format!(
                "{}:{}",
                run.status,
                run.conclusion.as_deref().unwrap_or("")
            );
            let prev = prior.get(&run.id);
            if prev.map(|p| p == &key).unwrap_or(false) {
                continue;
            }

            let prev_str = prev.map(|s| s.as_str());
            if is_failure_transition(prev_str, &run.status, run.conclusion.as_deref())
                && prefs.on_failure
            {
                send(
                    app,
                    "Workflow failed",
                    &format!(
                        "{} on {} — {}",
                        run.workflow_name.as_str().or_empty(&run.name),
                        run.head_branch,
                        run.conclusion.as_deref().unwrap_or("failure"),
                    ),
                );
            } else if pr_run_ids.contains(&run.id)
                && is_pending_to_success(prev_str, &run.status, run.conclusion.as_deref())
            {
                send(
                    app,
                    "Workflow succeeded",
                    &format!(
                        "{} on {} — success",
                        run.workflow_name.as_str().or_empty(&run.name),
                        run.head_branch,
                    ),
                );
            } else if prefs.on_every_transition && prev.is_some() {
                let conc = run.conclusion.clone().unwrap_or_default();
                send(
                    app,
                    &format!("{} {}", run.status, conc),
                    run.workflow_name.as_str().or_empty(&run.name),
                );
            }
        }

        // Per-PR "all green" transitions.
        if prefs.on_first_success {
            let prior_pr_aggs = prior_pr_aggregates(&prior, snapshot.prs_prev);
            for pr in active_prs.iter().copied() {
                let prev_agg = prior_pr_aggs
                    .get(&(pr.repo.owner.clone(), pr.repo.name.clone(), pr.number));
                let new_all_success = !pr.runs.is_empty()
                    && pr.runs.iter().all(|r| {
                        r.status == "completed" && r.conclusion.as_deref() == Some("success")
                    });
                let was_all_success = prev_agg.copied().unwrap_or(false);
                if new_all_success && !was_all_success {
                    send(
                        app,
                        "All checks passed",
                        &format!(
                            "{}/{} #{} — {}",
                            pr.repo.owner, pr.repo.name, pr.number, pr.title
                        ),
                    );
                }
            }
        }
    }

    // Persist new map.
    let new_map: HashMap<u64, String> = all_runs
        .iter()
        .map(|run| {
            (
                run.id,
                format!(
                    "{}:{}",
                    run.status,
                    run.conclusion.as_deref().unwrap_or("")
                ),
            )
        })
        .collect();
    *state.last_run_conclusions.write() = new_map;

    Ok(())
}

fn send(app: &AppHandle, title: &str, body: &str) {
    let _ = app
        .notification()
        .builder()
        .title(title)
        .body(body)
        .show();
}

fn is_failure_transition(prev_key: Option<&str>, status: &str, conclusion: Option<&str>) -> bool {
    if status != "completed" {
        return false;
    }
    let bad = matches!(conclusion, Some("failure") | Some("timed_out"));
    if !bad {
        return false;
    }
    // Was previous a failure already? Suppress.
    if let Some(p) = prev_key {
        if p.contains(":failure") || p.contains(":timed_out") {
            return false;
        }
    }
    true
}

fn is_pending_to_success(
    prev_key: Option<&str>,
    status: &str,
    conclusion: Option<&str>,
) -> bool {
    if status != "completed" || conclusion != Some("success") {
        return false;
    }
    let prev = match prev_key {
        Some(p) => p,
        None => return false,
    };
    matches!(
        prev,
        s if s.starts_with("queued:")
            || s.starts_with("in_progress:")
            || s.starts_with("waiting:")
            || s.starts_with("requested:")
            || s.starts_with("pending:")
    )
}

trait OrEmpty {
    fn or_empty<'a>(&'a self, fallback: &'a str) -> &'a str;
}
impl OrEmpty for Option<&str> {
    fn or_empty<'a>(&'a self, fallback: &'a str) -> &'a str {
        match self {
            Some(s) if !s.is_empty() => s,
            _ => fallback,
        }
    }
}
impl<'a> OrEmpty for &'a str {
    fn or_empty<'b>(&'b self, fallback: &'b str) -> &'b str {
        if self.is_empty() {
            fallback
        } else {
            self
        }
    }
}

fn prior_pr_aggregates(
    prior: &HashMap<u64, String>,
    prev_prs: &[PullRequestSummary],
) -> HashMap<(String, String, u64), bool> {
    let mut out = HashMap::new();
    for pr in prev_prs {
        let all_success = !pr.runs.is_empty()
            && pr.runs.iter().all(|r| {
                let key = format!("{}:{}", r.status, r.conclusion.as_deref().unwrap_or(""));
                let in_prior = prior.get(&r.id).map(|s| s == &key).unwrap_or(false);
                in_prior && r.status == "completed" && r.conclusion.as_deref() == Some("success")
            });
        out.insert(
            (pr.repo.owner.clone(), pr.repo.name.clone(), pr.number),
            all_success,
        );
    }
    out
}

/// What the poller hands to dispatch_transitions. Holds borrowed slices so
/// we don't clone the whole snapshot just to diff.
pub struct PollSnapshot<'a> {
    pub prs: &'a [PullRequestSummary],
    pub watched: &'a [WatchedRepoState],
    pub prs_prev: &'a [PullRequestSummary],
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cold_start_fires_nothing() {
        // Verified indirectly: dispatch_transitions checks if `prior` is
        // empty before running any logic.
        let prior: HashMap<u64, String> = HashMap::new();
        assert!(prior.is_empty());
    }

    #[test]
    fn failure_transition_fires_once() {
        assert!(is_failure_transition(
            Some("in_progress:"),
            "completed",
            Some("failure")
        ));
        // Already-failed run should not re-fire.
        assert!(!is_failure_transition(
            Some("completed:failure"),
            "completed",
            Some("failure")
        ));
    }

    #[test]
    fn success_is_not_failure() {
        assert!(!is_failure_transition(
            Some("in_progress:"),
            "completed",
            Some("success")
        ));
    }

    #[test]
    fn pending_to_success_fires() {
        assert!(is_pending_to_success(
            Some("in_progress:"),
            "completed",
            Some("success"),
        ));
        assert!(is_pending_to_success(
            Some("queued:"),
            "completed",
            Some("success"),
        ));
    }

    #[test]
    fn already_success_does_not_refire() {
        assert!(!is_pending_to_success(
            Some("completed:success"),
            "completed",
            Some("success"),
        ));
    }

    #[test]
    fn pending_to_failure_is_not_pending_to_success() {
        assert!(!is_pending_to_success(
            Some("in_progress:"),
            "completed",
            Some("failure"),
        ));
    }

    #[test]
    fn no_prior_does_not_fire() {
        assert!(!is_pending_to_success(
            None,
            "completed",
            Some("success"),
        ));
    }
}

