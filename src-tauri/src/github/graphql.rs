use anyhow::Result;
use serde::Deserialize;

use super::client::GitHubClient;
use super::types::RepoRef;

const VIEWER_PRS_QUERY: &str = r#"
query ViewerPRs($first: Int!) {
  viewer {
    pullRequests(first: $first, states: [OPEN], orderBy: {field: UPDATED_AT, direction: DESC}) {
      nodes {
        number
        title
        url
        isDraft
        state
        reviewDecision
        headRefName
        headRefOid
        repository { owner { login } name }
      }
    }
  }
}
"#;

#[derive(Debug, Deserialize)]
struct GraphResp {
    viewer: Viewer,
}

#[derive(Debug, Deserialize)]
struct Viewer {
    #[serde(rename = "pullRequests")]
    pull_requests: PullRequests,
}

#[derive(Debug, Deserialize)]
struct PullRequests {
    nodes: Vec<PrNode>,
}

#[derive(Debug, Deserialize)]
struct PrNode {
    number: u64,
    title: String,
    url: String,
    #[serde(rename = "isDraft")]
    is_draft: bool,
    state: String,
    #[serde(rename = "reviewDecision")]
    review_decision: Option<String>,
    #[serde(rename = "headRefName")]
    head_ref_name: String,
    #[serde(rename = "headRefOid")]
    head_ref_oid: String,
    repository: Repo,
}

#[derive(Debug, Deserialize)]
struct Repo {
    owner: Owner,
    name: String,
}

#[derive(Debug, Deserialize)]
struct Owner {
    login: String,
}

#[derive(Debug, Clone)]
pub struct ViewerPullRequest {
    pub repo: RepoRef,
    pub number: u64,
    pub title: String,
    pub url: String,
    pub is_draft: bool,
    pub state: String,
    pub review_decision: Option<String>,
    pub branch: String,
    pub head_sha: String,
}

pub async fn viewer_open_prs(client: &GitHubClient) -> Result<Vec<ViewerPullRequest>> {
    #[derive(serde::Serialize)]
    struct Vars { first: u32 }
    let resp: GraphResp = client
        .graphql(VIEWER_PRS_QUERY, Vars { first: 100 })
        .await?;
    Ok(resp
        .viewer
        .pull_requests
        .nodes
        .into_iter()
        .map(|n| ViewerPullRequest {
            repo: RepoRef {
                owner: n.repository.owner.login,
                name: n.repository.name,
            },
            number: n.number,
            title: n.title,
            url: n.url,
            is_draft: n.is_draft,
            state: n.state,
            review_decision: n.review_decision,
            branch: n.head_ref_name,
            head_sha: n.head_ref_oid,
        })
        .collect())
}
