import { useState } from "react";
import type { PullRequestSummary, WatchedRepoState } from "../lib/types";
import { StatusIcon } from "../components/StatusIcon";
import { ReviewIcon } from "../components/ReviewIcon";
import { DismissButton } from "../components/DismissButton";
import { openInBrowser } from "../lib/invoke";
import { RunDetail } from "./RunDetail";

interface PRCardProps { pr: PullRequestSummary; }
export function PRCard({ pr }: PRCardProps) {
  const [openRun, setOpenRun] = useState<number | null>(null);
  return (
    <div className="repo-card">
      <div className="repo-card-header">
        <StatusIcon aggregate={pr.aggregate} />
        <button className="repo-name" onClick={() => openInBrowser(pr.url)}>
          {pr.repo.owner}/{pr.repo.name} #{pr.number}
        </button>
        <ReviewIcon decision={pr.review_decision} size={14} />
        <span className="repo-meta">{pr.branch}{pr.is_draft ? " · draft" : ""}</span>
      </div>
      <div style={{ marginBottom: 6, color: "var(--fg-muted)" }}>{pr.title}</div>
      {pr.runs.length === 0 && (
        <div style={{ color: "var(--fg-muted)", fontSize: 12 }}>No workflow runs yet.</div>
      )}
      {pr.runs.map((run) => (
        <div key={run.id}>
          <div className="run-row" onClick={() => setOpenRun(openRun === run.id ? null : run.id)}>
            <StatusIcon status={run.status} conclusion={run.conclusion} />
            <span className="run-name">{run.workflow_name || run.name}</span>
            <span className="run-meta">{run.event}</span>
          </div>
          {openRun === run.id && <RunDetail repo={pr.repo} run={run} />}
        </div>
      ))}
    </div>
  );
}

interface WatchedCardProps { watched: WatchedRepoState; }
export function WatchedCard({ watched }: WatchedCardProps) {
  const [openRun, setOpenRun] = useState<number | null>(null);
  const canDismiss = watched.aggregate === "failure";
  const showDismiss = canDismiss || watched.dismissed;
  return (
    <div className={`repo-card${watched.dismissed ? " is-dismissed" : ""}`}>
      <div className="repo-card-header">
        <StatusIcon aggregate={watched.aggregate} dismissed={watched.dismissed} />
        <button
          className="repo-name"
          onClick={() => openInBrowser(`https://github.com/${watched.repo.owner}/${watched.repo.name}/actions`)}
        >
          {watched.repo.owner}/{watched.repo.name}
        </button>
        {showDismiss && (
          <DismissButton repo={watched.repo} dismissed={watched.dismissed} />
        )}
        {watched.branch_filter && (
          <span className="repo-meta">branch: {watched.branch_filter}</span>
        )}
      </div>
      {watched.recent_runs.length === 0 && (
        <div style={{ color: "var(--fg-muted)", fontSize: 12 }}>No recent runs.</div>
      )}
      {watched.recent_runs.slice(0, 8).map((run) => (
        <div key={run.id}>
          <div className="run-row" onClick={() => setOpenRun(openRun === run.id ? null : run.id)}>
            <StatusIcon status={run.status} conclusion={run.conclusion} />
            <span className="run-name">{run.workflow_name || run.name}</span>
            <span className="run-meta">{run.head_branch}</span>
          </div>
          {openRun === run.id && <RunDetail repo={watched.repo} run={run} />}
        </div>
      ))}
    </div>
  );
}
