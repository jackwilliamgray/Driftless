import { useEffect, useState } from "react";
import type { RepoRef, WorkflowJob, WorkflowRun } from "../lib/types";
import { getRunJobs, openInBrowser } from "../lib/invoke";
import { StatusIcon } from "../components/StatusIcon";

export function RunDetail({ repo, run }: { repo: RepoRef; run: WorkflowRun }) {
  const [jobs, setJobs] = useState<WorkflowJob[] | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let mounted = true;
    setJobs(null);
    setError(null);
    getRunJobs(repo, run.id)
      .then((js) => mounted && setJobs(js))
      .catch((e) => mounted && setError(String(e)));
    return () => { mounted = false; };
  }, [repo.owner, repo.name, run.id]);

  if (error) return <div className="run-detail" style={{ color: "#ef4444" }}>Failed to load jobs: {error}</div>;
  if (!jobs) return <div className="run-detail" style={{ color: "var(--fg-muted)" }}>Loading jobs…</div>;
  if (jobs.length === 0) return <div className="run-detail" style={{ color: "var(--fg-muted)" }}>No jobs yet.</div>;

  return (
    <div className="run-detail">
      {jobs.map((job) => (
        <div className="job" key={job.id}>
          <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
            <StatusIcon status={job.status} conclusion={job.conclusion} />
            <button
              onClick={() => openInBrowser(job.html_url)}
              style={{ flex: 1, textAlign: "left" }}
            >
              {job.name}
            </button>
          </div>
          {job.steps.map((step) => (
            <div key={`${job.id}-${step.number}`} className="step">
              <StatusIcon status={step.status} conclusion={step.conclusion} size={10} />
              <span>{step.name}</span>
            </div>
          ))}
        </div>
      ))}
    </div>
  );
}
