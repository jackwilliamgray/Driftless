use parking_lot::RwLock;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::Arc;
use tokio::sync::Notify;

use crate::github::types::{PullRequestSummary, WatchedRepoState, WorkflowRun};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum AggregateState {
    Idle,
    Pending,
    Success,
    Failure,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct AuthStatus {
    pub logged_in: bool,
    pub login: Option<String>,
    pub error: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AppSnapshot {
    pub auth: AuthStatus,
    pub prs: Vec<PullRequestSummary>,
    pub watched: Vec<WatchedRepoState>,
    pub aggregate: AggregateState,
    pub last_polled_at: Option<String>,
    pub next_poll_at: Option<String>,
    pub rate_limit_remaining: Option<u32>,
    #[serde(default)]
    pub global_excluded_workflows: Vec<String>,
    #[serde(default)]
    pub projects: Vec<Project>,
}

impl Default for AppSnapshot {
    fn default() -> Self {
        Self {
            auth: AuthStatus::default(),
            prs: Vec::new(),
            watched: Vec::new(),
            aggregate: AggregateState::Idle,
            last_polled_at: None,
            next_poll_at: None,
            rate_limit_remaining: None,
            global_excluded_workflows: Vec::new(),
            projects: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ProjectRepoRef {
    pub owner: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub prefix: Option<String>,
    #[serde(default)]
    pub member_repos: Vec<ProjectRepoRef>,
    #[serde(default = "default_true")]
    pub enabled: bool,
}

fn default_true() -> bool { true }

pub fn repo_in_project(owner: &str, name: &str, project: &Project) -> bool {
    if project
        .member_repos
        .iter()
        .any(|r| r.owner == owner && r.name == name)
    {
        return true;
    }
    if let Some(prefix) = project.prefix.as_deref() {
        if !prefix.is_empty() && name.starts_with(prefix) {
            return true;
        }
    }
    false
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchedRepoConfig {
    pub owner: String,
    pub name: String,
    pub branch_filter: Option<String>,
    #[serde(default)]
    pub excluded_workflows: Vec<String>,
    /// If set, runs with id <= this value are considered acknowledged. As
    /// soon as a poll surfaces a run with a higher id, the dismissal clears.
    #[serde(default)]
    pub dismissed_until_run_id: Option<u64>,
}

#[derive(Clone, Debug, Serialize, Deserialize, Default)]
pub struct PersistedConfig {
    pub watched: Vec<WatchedRepoConfig>,
    pub notification_prefs: NotificationPrefs,
    #[serde(default)]
    pub global_excluded_workflows: Vec<String>,
    #[serde(default)]
    pub projects: Vec<Project>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct NotificationPrefs {
    pub on_failure: bool,
    pub on_first_success: bool,
    pub on_every_transition: bool,
}

impl Default for NotificationPrefs {
    fn default() -> Self {
        Self {
            on_failure: true,
            on_first_success: true,
            on_every_transition: false,
        }
    }
}

/// Shared app state. ETag map and snapshot live behind a RwLock.
/// `wakeup` is signaled when a force-refresh or config change should
/// short-circuit the poller's sleep.
pub struct AppState {
    pub snapshot: RwLock<AppSnapshot>,
    pub etags: RwLock<HashMap<String, String>>,
    pub last_run_conclusions: RwLock<HashMap<u64, String>>,
    pub config: RwLock<PersistedConfig>,
    pub wakeup: Arc<Notify>,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            snapshot: RwLock::new(AppSnapshot::default()),
            etags: RwLock::new(HashMap::new()),
            last_run_conclusions: RwLock::new(HashMap::new()),
            config: RwLock::new(PersistedConfig::default()),
            wakeup: Arc::new(Notify::new()),
        }
    }
}

pub type SharedState = Arc<AppState>;

pub fn aggregate_runs<'a, I: IntoIterator<Item = &'a WorkflowRun>>(runs: I) -> AggregateState {
    // Aggregate reflects the current state of each workflow, not its history:
    // for each workflow we keep only its most recent run, so an older failed
    // run that was re-run (or superseded by a later commit) doesn't poison
    // the result.
    let mut latest: HashMap<u64, &WorkflowRun> = HashMap::new();
    for r in runs {
        match latest.get(&r.workflow_id) {
            Some(existing) if existing.updated_at >= r.updated_at => {}
            _ => {
                latest.insert(r.workflow_id, r);
            }
        }
    }

    let mut has_pending = false;
    let mut has_failure = false;
    let mut has_success = false;
    for r in latest.values() {
        match r.status.as_str() {
            "queued" | "in_progress" | "waiting" | "requested" | "pending" => has_pending = true,
            "completed" => match r.conclusion.as_deref() {
                Some("failure") | Some("timed_out") => has_failure = true,
                Some("success") => has_success = true,
                _ => {}
            },
            _ => {}
        }
    }
    if has_failure {
        AggregateState::Failure
    } else if has_pending {
        AggregateState::Pending
    } else if has_success {
        AggregateState::Success
    } else {
        AggregateState::Idle
    }
}

pub fn merge_aggregates<I: IntoIterator<Item = AggregateState>>(items: I) -> AggregateState {
    let mut has_pending = false;
    let mut has_failure = false;
    let mut has_success = false;
    for a in items {
        match a {
            AggregateState::Failure => has_failure = true,
            AggregateState::Pending => has_pending = true,
            AggregateState::Success => has_success = true,
            AggregateState::Idle => {}
        }
    }
    if has_failure {
        AggregateState::Failure
    } else if has_pending {
        AggregateState::Pending
    } else if has_success {
        AggregateState::Success
    } else {
        AggregateState::Idle
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_with(
        workflow_id: u64,
        head_sha: &str,
        updated_at: &str,
        status: &str,
        conclusion: Option<&str>,
    ) -> WorkflowRun {
        WorkflowRun {
            id: 1,
            name: "x".into(),
            status: status.into(),
            conclusion: conclusion.map(|s| s.into()),
            head_branch: "main".into(),
            head_sha: head_sha.into(),
            event: "push".into(),
            html_url: "https://example".into(),
            workflow_id,
            workflow_name: "wf".into(),
            created_at: "".into(),
            updated_at: updated_at.into(),
        }
    }

    fn run(status: &str, conclusion: Option<&str>) -> WorkflowRun {
        run_with(0, "abc", "", status, conclusion)
    }

    #[test]
    fn empty_is_idle() {
        assert_eq!(aggregate_runs(std::iter::empty()), AggregateState::Idle);
    }
    #[test]
    fn any_failure_dominates() {
        let runs = vec![
            run_with(1, "abc", "", "completed", Some("success")),
            run_with(2, "abc", "", "in_progress", None),
            run_with(3, "abc", "", "completed", Some("failure")),
        ];
        assert_eq!(aggregate_runs(runs.iter()), AggregateState::Failure);
    }
    #[test]
    fn pending_beats_success() {
        let runs = vec![
            run_with(1, "abc", "", "completed", Some("success")),
            run_with(2, "abc", "", "in_progress", None),
        ];
        assert_eq!(aggregate_runs(runs.iter()), AggregateState::Pending);
    }
    #[test]
    fn all_success() {
        let runs = vec![
            run("completed", Some("success")),
            run("completed", Some("success")),
        ];
        assert_eq!(aggregate_runs(runs.iter()), AggregateState::Success);
    }
    #[test]
    fn rerun_success_supersedes_earlier_failure() {
        // Same (workflow, commit) ran twice — the more recent run wins.
        let runs = vec![
            run_with(1, "abc", "2026-05-05T10:00:00Z", "completed", Some("failure")),
            run_with(1, "abc", "2026-05-05T11:00:00Z", "completed", Some("success")),
        ];
        assert_eq!(aggregate_runs(runs.iter()), AggregateState::Success);
    }
    #[test]
    fn newer_commit_success_supersedes_old_failure() {
        // Same workflow ran on an older commit (failed) and again on a newer
        // commit (passed). The aggregate reflects current state, so success.
        let runs = vec![
            run_with(1, "old", "2026-05-04T10:00:00Z", "completed", Some("failure")),
            run_with(1, "new", "2026-05-05T11:00:00Z", "completed", Some("success")),
        ];
        assert_eq!(aggregate_runs(runs.iter()), AggregateState::Success);
    }

    #[test]
    fn one_workflow_failing_still_fails_aggregate() {
        // Two distinct workflows; one currently failing, one passing — overall
        // is failure because a workflow is still broken.
        let runs = vec![
            run_with(1, "abc", "2026-05-05T10:00:00Z", "completed", Some("success")),
            run_with(2, "abc", "2026-05-05T10:00:00Z", "completed", Some("failure")),
        ];
        assert_eq!(aggregate_runs(runs.iter()), AggregateState::Failure);
    }
}
