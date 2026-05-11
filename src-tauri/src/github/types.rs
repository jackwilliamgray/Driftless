use serde::{Deserialize, Serialize};

use crate::state::AggregateState;

#[derive(Clone, Debug, Serialize, Deserialize, Eq, PartialEq, Hash)]
pub struct RepoRef {
    pub owner: String,
    pub name: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowRun {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub head_branch: String,
    pub head_sha: String,
    pub event: String,
    pub html_url: String,
    pub workflow_id: u64,
    pub workflow_name: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PullRequestSummary {
    pub repo: RepoRef,
    pub number: u64,
    pub title: String,
    pub branch: String,
    pub head_sha: String,
    pub url: String,
    pub is_draft: bool,
    pub state: String,
    #[serde(default)]
    pub review_decision: Option<String>,
    pub runs: Vec<WorkflowRun>,
    pub aggregate: AggregateState,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WatchedRepoState {
    pub repo: RepoRef,
    pub branch_filter: Option<String>,
    pub recent_runs: Vec<WorkflowRun>,
    pub aggregate: AggregateState,
    #[serde(default)]
    pub excluded_workflows: Vec<String>,
    #[serde(default)]
    pub dismissed: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct JobStep {
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub number: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct WorkflowJob {
    pub id: u64,
    pub name: String,
    pub status: String,
    pub conclusion: Option<String>,
    pub html_url: String,
    pub steps: Vec<JobStep>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
}
