use anyhow::Result;
use serde::Deserialize;

use super::client::{FetchOutcome, GitHubClient};
use super::types::{JobStep, RepoRef, WorkflowJob, WorkflowRun};

#[derive(Debug, Deserialize)]
struct RunsResponse {
    workflow_runs: Vec<RunRaw>,
}

#[derive(Debug, Deserialize)]
struct RunRaw {
    id: u64,
    name: Option<String>,
    status: String,
    conclusion: Option<String>,
    head_branch: Option<String>,
    head_sha: String,
    event: String,
    html_url: String,
    workflow_id: u64,
    created_at: String,
    updated_at: String,
}

fn raw_to_run(raw: RunRaw, workflow_name: String) -> WorkflowRun {
    WorkflowRun {
        id: raw.id,
        name: raw.name.clone().unwrap_or_default(),
        status: raw.status,
        conclusion: raw.conclusion,
        head_branch: raw.head_branch.unwrap_or_default(),
        head_sha: raw.head_sha,
        event: raw.event,
        html_url: raw.html_url,
        workflow_id: raw.workflow_id,
        workflow_name,
        created_at: raw.created_at,
        updated_at: raw.updated_at,
    }
}

/// List the most recent workflow runs in a repository, optionally filtered to
/// a single head SHA (used to scope to a PR's head commit) or branch.
pub async fn list_repo_runs(
    client: &GitHubClient,
    repo: &RepoRef,
    head_sha: Option<&str>,
    branch: Option<&str>,
    per_page: u32,
) -> Result<FetchOutcome<Vec<WorkflowRun>>> {
    let mut path = format!(
        "/repos/{}/{}/actions/runs?per_page={}",
        repo.owner, repo.name, per_page
    );
    if let Some(sha) = head_sha {
        path.push_str(&format!("&head_sha={sha}"));
    }
    if let Some(b) = branch {
        path.push_str(&format!("&branch={b}"));
    }
    let outcome: FetchOutcome<RunsResponse> = client.get_etag(&path).await?;
    Ok(match outcome {
        FetchOutcome::NotModified => FetchOutcome::NotModified,
        FetchOutcome::Fresh(resp) => {
            // For each run, the run object's `name` is the workflow name in
            // current GH responses, so we use that directly.
            let runs = resp
                .workflow_runs
                .into_iter()
                .map(|r| {
                    let wf_name = r.name.clone().unwrap_or_default();
                    raw_to_run(r, wf_name)
                })
                .collect();
            FetchOutcome::Fresh(runs)
        }
    })
}

#[derive(Debug, Deserialize)]
struct JobsResponse {
    jobs: Vec<JobRaw>,
}

#[derive(Debug, Deserialize)]
struct JobRaw {
    id: u64,
    name: String,
    status: String,
    conclusion: Option<String>,
    html_url: String,
    started_at: Option<String>,
    completed_at: Option<String>,
    #[serde(default)]
    steps: Vec<StepRaw>,
}

#[derive(Debug, Deserialize)]
struct StepRaw {
    name: String,
    status: String,
    conclusion: Option<String>,
    number: u32,
}

pub async fn list_run_jobs(
    client: &GitHubClient,
    repo: &RepoRef,
    run_id: u64,
) -> Result<Vec<WorkflowJob>> {
    let path = format!(
        "/repos/{}/{}/actions/runs/{}/jobs?per_page=50",
        repo.owner, repo.name, run_id
    );
    let resp: JobsResponse = client.get_json(&path).await?;
    Ok(resp
        .jobs
        .into_iter()
        .map(|j| WorkflowJob {
            id: j.id,
            name: j.name,
            status: j.status,
            conclusion: j.conclusion,
            html_url: j.html_url,
            started_at: j.started_at,
            completed_at: j.completed_at,
            steps: j
                .steps
                .into_iter()
                .map(|s| JobStep {
                    name: s.name,
                    status: s.status,
                    conclusion: s.conclusion,
                    number: s.number,
                })
                .collect(),
        })
        .collect())
}

#[derive(Debug, Deserialize)]
struct SearchResponse {
    items: Vec<SearchItem>,
}

#[derive(Debug, Deserialize)]
struct SearchItem {
    number: u64,
    title: String,
    html_url: String,
    state: String,
    pull_request: Option<serde_json::Value>,
    repository_url: String,
    #[serde(default)]
    draft: bool,
}

#[derive(Debug, Clone)]
pub struct RecentClosedPr {
    pub repo: RepoRef,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub state: String,
    pub is_draft: bool,
}

/// Search GitHub for recently-closed PRs authored by the viewer. Used to
/// catch in-flight runs that started after merge.
pub async fn recent_closed_prs(
    client: &GitHubClient,
    since_iso_date: &str,
) -> Result<Vec<RecentClosedPr>> {
    let q = format!(
        "is:pr author:@me is:closed updated:>{since_iso_date} archived:false"
    );
    let path = format!(
        "/search/issues?q={}&per_page=30&sort=updated",
        urlencoding(&q)
    );
    let resp: SearchResponse = client.get_json(&path).await?;
    Ok(resp
        .items
        .into_iter()
        .filter(|i| i.pull_request.is_some())
        .filter_map(|i| {
            // repository_url looks like https://api.github.com/repos/{owner}/{name}
            let parts: Vec<&str> = i.repository_url.rsplit('/').take(2).collect();
            if parts.len() != 2 {
                return None;
            }
            let owner = parts[1].to_string();
            let name = parts[0].to_string();
            Some(RecentClosedPr {
                repo: RepoRef { owner, name },
                number: i.number,
                title: i.title,
                url: i.html_url,
                state: i.state,
                is_draft: i.draft,
            })
        })
        .collect())
}

/// Minimal urlencoding for query strings — avoids pulling another crate.
fn urlencoding(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for b in s.bytes() {
        match b {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(b as char)
            }
            b' ' => out.push('+'),
            _ => out.push_str(&format!("%{:02X}", b)),
        }
    }
    out
}
