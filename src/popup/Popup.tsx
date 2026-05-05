import { useEffect, useState } from "react";
import type { AppSnapshot } from "../lib/types";
import { getSnapshot, openInBrowser, showDashboard, forceRefresh, hidePopup } from "../lib/invoke";
import { onSnapshot, onAuthChanged } from "../lib/events";
import { StatusIcon } from "../components/StatusIcon";
import { RunRow } from "./RunRow";

export function Popup() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);

  useEffect(() => {
    let mounted = true;
    getSnapshot()
      .then((s) => mounted && setSnapshot(s))
      .catch(() => {});

    const offSnap = onSnapshot((s) => mounted && setSnapshot(s));
    const offAuth = onAuthChanged(() => {
      getSnapshot()
        .then((s) => mounted && setSnapshot(s))
        .catch(() => {});
    });

    return () => {
      mounted = false;
      offSnap.then((fn) => fn());
      offAuth.then((fn) => fn());
    };
  }, []);

  if (!snapshot) {
    return (
      <div className="popup">
        <div className="popup-empty">Loading…</div>
      </div>
    );
  }

  if (!snapshot.auth.logged_in) {
    return (
      <div className="popup">
        <div className="popup-header"><span className="title">Driftless</span></div>
        <div className="popup-empty">
          <p style={{ marginTop: 0 }}>Not signed in to the GitHub CLI.</p>
          <p style={{ color: "var(--fg-muted)", fontSize: 12 }}>
            Run <code>gh auth login</code> in your terminal, then click refresh.
          </p>
          <button className="btn-primary" onClick={() => forceRefresh()}>Retry</button>
        </div>
        <div className="popup-footer">
          <button onClick={() => showDashboard()}>Settings…</button>
        </div>
      </div>
    );
  }

  const { prs, watched, auth } = snapshot;
  const sortedWatched = [...watched].sort((a, b) => {
    const at = a.recent_runs.reduce((m, r) => Math.max(m, Date.parse(r.updated_at) || 0), 0);
    const bt = b.recent_runs.reduce((m, r) => Math.max(m, Date.parse(r.updated_at) || 0), 0);
    return bt - at;
  });

  return (
    <div className="popup">
      <div className="popup-header">
        <StatusIcon aggregate={snapshot.aggregate} size={14} />
        <span className="title">Driftless</span>
        <span className="meta">{auth.login ? `@${auth.login}` : ""}</span>
      </div>

      <div className="popup-section popup-section--prs">
        <div className="popup-section-label">My pull requests</div>
        {prs.length === 0 && <div className="popup-empty" style={{ padding: "8px 12px" }}>No open PRs.</div>}
        {prs.map((pr) => (
          <div key={`${pr.repo.owner}/${pr.repo.name}#${pr.number}`}>
            <button
              className="popup-row"
              onClick={() => openInBrowser(pr.url)}
              title={`#${pr.number} · ${pr.branch}`}
            >
              <StatusIcon aggregate={pr.aggregate} />
              <span className="name">
                {pr.repo.owner}/{pr.repo.name} <span style={{ color: "var(--fg-muted)" }}>#{pr.number}</span>
              </span>
            </button>
            {pr.runs.length > 0 && (
              <div style={{ paddingLeft: 30, paddingBottom: 4, display: "flex", gap: 4, flexWrap: "wrap" }}>
                {pr.runs.slice(0, 6).map((run) => (
                  <RunRow key={run.id} run={run} />
                ))}
              </div>
            )}
          </div>
        ))}
      </div>

      <div className="popup-section popup-section--watched">
        <div className="popup-section-label">Watched repos</div>
        {watched.length === 0 && (
          <div className="popup-empty" style={{ padding: "8px 12px" }}>
            None yet. Add repos in Settings…
          </div>
        )}
        {sortedWatched.map((w) => (
          <button
            key={`${w.repo.owner}/${w.repo.name}`}
            className="popup-row"
            onClick={() =>
              openInBrowser(`https://github.com/${w.repo.owner}/${w.repo.name}/actions`)
            }
          >
            <StatusIcon aggregate={w.aggregate} />
            <span className="name">
              {w.repo.owner}/{w.repo.name}
              {w.branch_filter && (
                <span style={{ color: "var(--fg-muted)" }}> ({w.branch_filter})</span>
              )}
            </span>
          </button>
        ))}
      </div>

      <div className="popup-footer">
        <button onClick={() => { showDashboard(); hidePopup(); }}>Open dashboard…</button>
        <button onClick={() => forceRefresh()}>Refresh</button>
      </div>
    </div>
  );
}
