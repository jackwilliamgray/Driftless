use anyhow::{anyhow, Context, Result};
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, AUTHORIZATION, IF_NONE_MATCH, USER_AGENT};
use reqwest::{Method, Response, StatusCode};
use serde::de::DeserializeOwned;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tracing::{debug, warn};

use crate::state::SharedState;

const API: &str = "https://api.github.com";
const UA: &str = concat!("Driftless/", env!("CARGO_PKG_VERSION"));

/// Wraps reqwest with shared ETag tracking, rate-limit awareness, and
/// transparent 304 handling. Token is held in-memory only.
#[derive(Clone)]
pub struct GitHubClient {
    http: reqwest::Client,
    token: Arc<parking_lot::RwLock<Option<String>>>,
    state: SharedState,
}

#[derive(Debug)]
pub enum FetchOutcome<T> {
    Fresh(T),
    NotModified,
}

impl<T> FetchOutcome<T> {
    pub fn into_option(self) -> Option<T> {
        match self {
            Self::Fresh(v) => Some(v),
            Self::NotModified => None,
        }
    }
}

impl GitHubClient {
    pub fn new(state: SharedState) -> Result<Self> {
        let http = reqwest::Client::builder()
            .user_agent(UA)
            .timeout(Duration::from_secs(30))
            .gzip(true)
            .brotli(true)
            .build()
            .context("building reqwest client")?;
        Ok(Self {
            http,
            token: Arc::new(parking_lot::RwLock::new(None)),
            state,
        })
    }

    pub fn set_token(&self, token: Option<String>) {
        *self.token.write() = token;
    }

    pub fn has_token(&self) -> bool {
        self.token.read().is_some()
    }

    fn auth_headers(&self) -> Result<HeaderMap> {
        let token = self
            .token
            .read()
            .clone()
            .ok_or_else(|| anyhow!("no GitHub token available"))?;
        let mut h = HeaderMap::new();
        h.insert(USER_AGENT, HeaderValue::from_static(UA));
        h.insert(ACCEPT, HeaderValue::from_static("application/vnd.github+json"));
        h.insert("X-GitHub-Api-Version", HeaderValue::from_static("2022-11-28"));
        h.insert(
            AUTHORIZATION,
            HeaderValue::from_str(&format!("Bearer {token}"))
                .map_err(|e| anyhow!("bad token bytes: {e}"))?,
        );
        Ok(h)
    }

    fn note_rate_limit(&self, resp: &Response) {
        if let Some(remaining) = resp
            .headers()
            .get("x-ratelimit-remaining")
            .and_then(|v| v.to_str().ok())
            .and_then(|s| s.parse::<u32>().ok())
        {
            self.state.snapshot.write().rate_limit_remaining = Some(remaining);
        }
    }

    /// GET with shared ETag map. Returns NotModified if the cached ETag is
    /// still valid; the caller is responsible for keeping its prior value.
    pub async fn get_etag<T: DeserializeOwned>(
        &self,
        path: &str,
    ) -> Result<FetchOutcome<T>> {
        let url = format!("{API}{path}");
        let mut headers = self.auth_headers()?;
        if let Some(etag) = self.state.etags.read().get(path).cloned() {
            if let Ok(v) = HeaderValue::from_str(&etag) {
                headers.insert(IF_NONE_MATCH, v);
            }
        }

        let resp = self.http.get(&url).headers(headers).send().await
            .with_context(|| format!("GET {url}"))?;
        self.note_rate_limit(&resp);

        match resp.status() {
            StatusCode::NOT_MODIFIED => Ok(FetchOutcome::NotModified),
            s if s.is_success() => {
                if let Some(etag) = resp
                    .headers()
                    .get("etag")
                    .and_then(|v| v.to_str().ok())
                    .map(|s| s.to_string())
                {
                    self.state.etags.write().insert(path.to_string(), etag);
                }
                let body: T = resp.json().await
                    .with_context(|| format!("decoding response from {url}"))?;
                Ok(FetchOutcome::Fresh(body))
            }
            StatusCode::FORBIDDEN | StatusCode::TOO_MANY_REQUESTS => {
                let txt = resp.text().await.unwrap_or_default();
                Err(anyhow!("rate-limited at {url}: {txt}"))
            }
            s => {
                let txt = resp.text().await.unwrap_or_default();
                Err(anyhow!("GET {url} -> {s}: {txt}"))
            }
        }
    }

    /// One-shot GET without ETag tracking. Used for endpoints we always want
    /// fresh, like job details.
    pub async fn get_json<T: DeserializeOwned>(&self, path: &str) -> Result<T> {
        let url = format!("{API}{path}");
        let resp = self
            .http
            .get(&url)
            .headers(self.auth_headers()?)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        self.note_rate_limit(&resp);
        if !resp.status().is_success() {
            let s = resp.status();
            let txt = resp.text().await.unwrap_or_default();
            return Err(anyhow!("GET {url} -> {s}: {txt}"));
        }
        resp.json::<T>()
            .await
            .with_context(|| format!("decoding {url}"))
    }

    /// POST a GraphQL query against api.github.com/graphql.
    pub async fn graphql<V: Serialize, T: DeserializeOwned>(
        &self,
        query: &str,
        variables: V,
    ) -> Result<T> {
        #[derive(serde::Deserialize)]
        struct GraphResp<T> {
            data: Option<T>,
            errors: Option<serde_json::Value>,
        }
        #[derive(serde::Serialize)]
        struct GraphReq<'a, V> {
            query: &'a str,
            variables: V,
        }

        let resp = self
            .http
            .request(Method::POST, format!("{API}/graphql"))
            .headers(self.auth_headers()?)
            .json(&GraphReq { query, variables })
            .send()
            .await
            .context("posting graphql query")?;
        self.note_rate_limit(&resp);
        let status = resp.status();
        if !status.is_success() {
            let txt = resp.text().await.unwrap_or_default();
            return Err(anyhow!("graphql {status}: {txt}"));
        }
        let body: GraphResp<T> = resp.json().await.context("decoding graphql response")?;
        if let Some(errs) = body.errors {
            warn!(errors = %errs, "graphql returned errors");
            return Err(anyhow!("graphql errors: {errs}"));
        }
        body.data.ok_or_else(|| anyhow!("graphql returned no data"))
    }

    pub async fn ping(&self) -> Result<()> {
        let url = format!("{API}/rate_limit");
        let resp = self
            .http
            .get(&url)
            .headers(self.auth_headers()?)
            .send()
            .await
            .context("ping")?;
        self.note_rate_limit(&resp);
        debug!(status = ?resp.status(), "ping response");
        Ok(())
    }
}
