import { useEffect, useMemo, useState } from "react";
import type {
  AppSnapshot,
  NotificationPrefs,
  PollError,
  Project,
  WatchedRepoState,
} from "../lib/types";
import { getSnapshot, forceRefresh } from "../lib/invoke";
import { onSnapshot, onPollError } from "../lib/events";
import { PRCard, WatchedCard } from "./RepoCard";
import { Settings } from "./Settings";
import { projectForRepo } from "../lib/projects";

const defaultPrefs: NotificationPrefs = {
  on_failure: true,
  on_first_success: true,
  on_every_transition: false,
};

type View = "prs" | "watched" | "settings";

export function Dashboard() {
  const [snapshot, setSnapshot] = useState<AppSnapshot | null>(null);
  const [view, setView] = useState<View>("prs");
  const [pollError, setPollError] = useState<PollError | null>(null);
  const [prefs, setPrefs] = useState<NotificationPrefs>(defaultPrefs);
  const [collapsedProjects, setCollapsedProjects] = useState<Set<string>>(new Set());

  const toggleCollapsed = (key: string) => {
    setCollapsedProjects((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const watchedGroups = useMemo(() => {
    if (!snapshot) return null;
    const projects = snapshot.projects ?? [];
    const buckets = new Map<string, WatchedRepoState[]>();
    for (const w of snapshot.watched) {
      const p = projectForRepo(w.repo, projects);
      if (p && !p.enabled) continue;
      const key = p ? p.id : "__ungrouped__";
      const list = buckets.get(key);
      if (list) list.push(w);
      else buckets.set(key, [w]);
    }
    const ordered: { key: string; project: Project | null; repos: WatchedRepoState[] }[] = [];
    for (const p of projects) {
      if (!p.enabled) continue;
      const repos = buckets.get(p.id);
      if (repos) ordered.push({ key: p.id, project: p, repos });
    }
    const ungrouped = buckets.get("__ungrouped__");
    if (ungrouped) {
      ordered.push({ key: "__ungrouped__", project: null, repos: ungrouped });
    }
    return ordered;
  }, [snapshot]);

  const visiblePRs = useMemo(() => {
    if (!snapshot) return [];
    const projects = snapshot.projects ?? [];
    return snapshot.prs.filter((pr) => {
      const p = projectForRepo(pr.repo, projects);
      return !p || p.enabled;
    });
  }, [snapshot]);

  useEffect(() => {
    document.body.classList.add("dashboard");
    let mounted = true;
    getSnapshot().then((s) => mounted && setSnapshot(s)).catch(() => {});
    const offSnap = onSnapshot((s) => mounted && setSnapshot(s));
    const offErr = onPollError((e) => mounted && setPollError(e));
    return () => {
      mounted = false;
      document.body.classList.remove("dashboard");
      offSnap.then((fn) => fn());
      offErr.then((fn) => fn());
    };
  }, []);

  return (
    <div className="dashboard-root">
      <aside className="dashboard-sidebar">
        <h1>Driftless</h1>
        <nav>
          <button className={view === "prs" ? "active" : ""} onClick={() => setView("prs")}>
            My pull requests
          </button>
          <button className={view === "watched" ? "active" : ""} onClick={() => setView("watched")}>
            Watched repos
          </button>
          <button className={view === "settings" ? "active" : ""} onClick={() => setView("settings")}>
            Settings
          </button>
        </nav>
        <div style={{ marginTop: "auto", paddingTop: 12 }}>
          <button onClick={() => forceRefresh()} style={{ padding: "6px 10px" }}>Refresh now</button>
          {snapshot?.last_polled_at && (
            <div style={{ fontSize: 11, color: "var(--fg-muted)", padding: "8px 10px" }}>
              Last polled {new Date(snapshot.last_polled_at).toLocaleTimeString()}
            </div>
          )}
          {snapshot?.rate_limit_remaining != null && (
            <div style={{ fontSize: 11, color: "var(--fg-muted)", padding: "0 10px" }}>
              Rate limit: {snapshot.rate_limit_remaining} req remaining
            </div>
          )}
        </div>
      </aside>

      <main className="dashboard-main">
        {!snapshot && <div className="empty-state">Loading…</div>}
        {snapshot && !snapshot.auth.logged_in && (
          <div className="empty-state">
            <h2>Not signed in to GitHub CLI</h2>
            <p>
              Driftless reads your GitHub token from the <code>gh</code> CLI.<br />
              Open a terminal and run <code>gh auth login</code>, then click Refresh.
            </p>
            {snapshot.auth.error && <p style={{ color: "#ef4444" }}>{snapshot.auth.error}</p>}
            <button className="btn-primary" onClick={() => forceRefresh()}>Refresh</button>
          </div>
        )}
        {snapshot && snapshot.auth.logged_in && view === "prs" && (
          <>
            <h2>My pull requests</h2>
            {visiblePRs.length === 0 && <div className="empty-state">No open PRs.</div>}
            {visiblePRs.map((pr) => (
              <PRCard key={`${pr.repo.owner}/${pr.repo.name}#${pr.number}`} pr={pr} />
            ))}
          </>
        )}
        {snapshot && snapshot.auth.logged_in && view === "watched" && (
          <>
            <h2>Watched repos</h2>
            {snapshot.watched.length === 0 && (
              <div className="empty-state">
                Add a repo from Settings to start tracking its workflow runs.
              </div>
            )}
            {watchedGroups && watchedGroups.length === 0 && snapshot.watched.length > 0 && (
              <div className="empty-state">
                All watched repos are in disabled projects. Enable a project in Settings.
              </div>
            )}
            {watchedGroups?.map((group) => {
              const collapsed = collapsedProjects.has(group.key);
              const label = group.project ? group.project.name : "Ungrouped";
              return (
                <section key={group.key} className="project-group">
                  <button
                    className="project-group-header"
                    onClick={() => toggleCollapsed(group.key)}
                    aria-expanded={!collapsed}
                  >
                    <span className={`chevron${collapsed ? " collapsed" : ""}`} aria-hidden>
                      ▾
                    </span>
                    <span className="project-group-name">{label}</span>
                    <span className="project-group-count">
                      {group.repos.length} repo{group.repos.length === 1 ? "" : "s"}
                    </span>
                  </button>
                  {!collapsed && (
                    <div className="project-group-body">
                      {group.repos.map((w) => (
                        <WatchedCard
                          key={`${w.repo.owner}/${w.repo.name}`}
                          watched={w}
                        />
                      ))}
                    </div>
                  )}
                </section>
              );
            })}
          </>
        )}
        {snapshot && view === "settings" && (
          <Settings snapshot={snapshot} prefs={prefs} onPrefsChange={setPrefs} />
        )}
      </main>

      {pollError && (
        <div className="toast" onClick={() => setPollError(null)}>
          Poll error: {pollError.message}
        </div>
      )}
    </div>
  );
}
