import { useEffect, useState } from "react";
import {
  disable as disableAutostart,
  enable as enableAutostart,
  isEnabled as isAutostartEnabled,
} from "@tauri-apps/plugin-autostart";
import type {
  AppSnapshot,
  DiscoveredRepo,
  NotificationPrefs,
  Project,
  ProjectRepoRef,
} from "../lib/types";
import {
  addWatchedRepo,
  addWatchedReposBulk,
  removeWatchedRepo,
  scanPathForRepos,
  setNotificationPrefs,
  setGlobalExcludedWorkflows,
  setRepoExcludedWorkflows,
  quitApp,
  upsertProject,
  removeProject,
  setProjectEnabled,
  setProjectMembers,
  setDebugLogging as setDebugLoggingCmd,
  openLogsFolder,
} from "../lib/invoke";
import { repoInProject } from "../lib/projects";

interface SettingsProps {
  snapshot: AppSnapshot;
  prefs: NotificationPrefs;
  onPrefsChange: (p: NotificationPrefs) => void;
}

type SettingsTab = "repos" | "projects" | "preferences";

function newProjectId(): string {
  if (typeof crypto !== "undefined" && "randomUUID" in crypto) {
    return crypto.randomUUID();
  }
  return `p_${Date.now().toString(36)}_${Math.random().toString(36).slice(2, 8)}`;
}

export function Settings({ snapshot, prefs, onPrefsChange }: SettingsProps) {
  const [tab, setTab] = useState<SettingsTab>("repos");
  const [repoInput, setRepoInput] = useState("");
  const [branchInput, setBranchInput] = useState("");
  const [error, setError] = useState<string | null>(null);
  const [scanPath, setScanPath] = useState("");
  const [scanning, setScanning] = useState(false);
  const [scanResults, setScanResults] = useState<DiscoveredRepo[] | null>(null);
  const [selectedKeys, setSelectedKeys] = useState<Set<string>>(new Set());
  const [scanError, setScanError] = useState<string | null>(null);
  const [scanInfo, setScanInfo] = useState<string | null>(null);
  const [globalExcludeInput, setGlobalExcludeInput] = useState("");
  const [repoExcludeInputs, setRepoExcludeInputs] = useState<Record<string, string>>({});
  const [excludeError, setExcludeError] = useState<string | null>(null);
  const [launchAtLogin, setLaunchAtLogin] = useState<boolean | null>(null);
  const [launchAtLoginError, setLaunchAtLoginError] = useState<string | null>(null);
  const [debugLogging, setDebugLogging] = useState<boolean>(snapshot.debug_logging ?? false);
  const [debugLoggingError, setDebugLoggingError] = useState<string | null>(null);

  useEffect(() => {
    setDebugLogging(snapshot.debug_logging ?? false);
  }, [snapshot.debug_logging]);

  const onToggleDebugLogging = async (next: boolean) => {
    setDebugLoggingError(null);
    setDebugLogging(next);
    try {
      await setDebugLoggingCmd(next);
    } catch (e) {
      setDebugLogging(!next);
      setDebugLoggingError(String(e));
    }
  };

  const onOpenLogsFolder = async () => {
    setDebugLoggingError(null);
    try {
      await openLogsFolder();
    } catch (e) {
      setDebugLoggingError(String(e));
    }
  };

  useEffect(() => {
    isAutostartEnabled()
      .then(setLaunchAtLogin)
      .catch((e) => setLaunchAtLoginError(String(e)));
  }, []);

  const onToggleLaunchAtLogin = async (next: boolean) => {
    setLaunchAtLoginError(null);
    setLaunchAtLogin(next);
    try {
      if (next) await enableAutostart();
      else await disableAutostart();
    } catch (e) {
      setLaunchAtLogin(!next);
      setLaunchAtLoginError(String(e));
    }
  };

  const onRemove = async (owner: string, name: string) => {
    setError(null);
    try {
      await removeWatchedRepo(owner, name);
    } catch (e) {
      setError(String(e));
    }
  };

  const projects = snapshot.projects ?? [];
  const [projectError, setProjectError] = useState<string | null>(null);
  const [newProjectName, setNewProjectName] = useState("");
  const [newProjectPrefix, setNewProjectPrefix] = useState("");

  const onCreateProject = async () => {
    setProjectError(null);
    const name = newProjectName.trim();
    if (!name) {
      setProjectError("Project name is required");
      return;
    }
    const project: Project = {
      id: newProjectId(),
      name,
      prefix: newProjectPrefix.trim() || null,
      member_repos: [],
      enabled: true,
    };
    try {
      await upsertProject(project);
      setNewProjectName("");
      setNewProjectPrefix("");
    } catch (e) {
      setProjectError(String(e));
    }
  };

  const onRenameProject = async (p: Project, name: string) => {
    setProjectError(null);
    try {
      await upsertProject({ ...p, name });
    } catch (e) {
      setProjectError(String(e));
    }
  };

  const onSetPrefix = async (p: Project, prefix: string) => {
    setProjectError(null);
    try {
      await upsertProject({ ...p, prefix: prefix.trim() || null });
    } catch (e) {
      setProjectError(String(e));
    }
  };

  const onToggleProject = async (p: Project, enabled: boolean) => {
    setProjectError(null);
    try {
      await setProjectEnabled(p.id, enabled);
    } catch (e) {
      setProjectError(String(e));
    }
  };

  const onDeleteProject = async (p: Project) => {
    setProjectError(null);
    try {
      await removeProject(p.id);
    } catch (e) {
      setProjectError(String(e));
    }
  };

  const onToggleMember = async (
    p: Project,
    repo: ProjectRepoRef,
    include: boolean,
  ) => {
    setProjectError(null);
    const next = include
      ? [...p.member_repos.filter((m) => !(m.owner === repo.owner && m.name === repo.name)), repo]
      : p.member_repos.filter((m) => !(m.owner === repo.owner && m.name === repo.name));
    try {
      await setProjectMembers(p.id, next);
    } catch (e) {
      setProjectError(String(e));
    }
  };

  const watchedKeys = new Set(
    snapshot.watched.map((w) => `${w.repo.owner}/${w.repo.name}`),
  );

  const onScan = async () => {
    setScanError(null);
    setScanInfo(null);
    if (!scanPath.trim()) {
      setScanError("Enter a path to scan");
      return;
    }
    setScanning(true);
    try {
      const results = await scanPathForRepos(scanPath.trim());
      setScanResults(results);
      const preselect = new Set<string>();
      for (const r of results) {
        const key = `${r.owner}/${r.name}`;
        if (!watchedKeys.has(key)) preselect.add(key);
      }
      setSelectedKeys(preselect);
      if (results.length === 0) {
        setScanInfo("No GitHub repos found under that path.");
      }
    } catch (e) {
      setScanError(String(e));
      setScanResults(null);
    } finally {
      setScanning(false);
    }
  };

  const onAddSelected = async () => {
    if (!scanResults) return;
    const toAdd = scanResults
      .filter((r) => selectedKeys.has(`${r.owner}/${r.name}`))
      .map((r) => ({ owner: r.owner, name: r.name, branch_filter: null }));
    if (toAdd.length === 0) {
      setScanError("Select at least one repo");
      return;
    }
    setScanError(null);
    try {
      const added = await addWatchedReposBulk(toAdd);
      setScanInfo(`Added ${added} new repo${added === 1 ? "" : "s"}.`);
      setScanResults(null);
      setSelectedKeys(new Set());
      setScanPath("");
    } catch (e) {
      setScanError(String(e));
    }
  };

  const toggleSelected = (key: string) => {
    setSelectedKeys((prev) => {
      const next = new Set(prev);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  };

  const onAdd = async () => {
    setError(null);
    const m = repoInput.trim().match(/^([^\/\s]+)\/([^\/\s]+)$/);
    if (!m) { setError("Use the format owner/repo"); return; }
    try {
      await addWatchedRepo(m[1], m[2], branchInput.trim() || undefined);
      setRepoInput("");
      setBranchInput("");
    } catch (e) {
      setError(String(e));
    }
  };

  const onPrefChange = async (patch: Partial<NotificationPrefs>) => {
    const next = { ...prefs, ...patch };
    onPrefsChange(next);
    try { await setNotificationPrefs(next); } catch (e) { setError(String(e)); }
  };

  const globalExcluded = snapshot.global_excluded_workflows ?? [];

  const onAddGlobalExclude = async () => {
    setExcludeError(null);
    const v = globalExcludeInput.trim();
    if (!v) return;
    if (globalExcluded.some((n) => n.toLowerCase() === v.toLowerCase())) {
      setGlobalExcludeInput("");
      return;
    }
    try {
      await setGlobalExcludedWorkflows([...globalExcluded, v]);
      setGlobalExcludeInput("");
    } catch (e) {
      setExcludeError(String(e));
    }
  };

  const onRemoveGlobalExclude = async (name: string) => {
    setExcludeError(null);
    try {
      await setGlobalExcludedWorkflows(globalExcluded.filter((n) => n !== name));
    } catch (e) {
      setExcludeError(String(e));
    }
  };

  const onAddRepoExclude = async (owner: string, name: string, current: string[]) => {
    setExcludeError(null);
    const key = `${owner}/${name}`;
    const v = (repoExcludeInputs[key] ?? "").trim();
    if (!v) return;
    if (current.some((n) => n.toLowerCase() === v.toLowerCase())) {
      setRepoExcludeInputs((p) => ({ ...p, [key]: "" }));
      return;
    }
    try {
      await setRepoExcludedWorkflows(owner, name, [...current, v]);
      setRepoExcludeInputs((p) => ({ ...p, [key]: "" }));
    } catch (e) {
      setExcludeError(String(e));
    }
  };

  const onRemoveRepoExclude = async (
    owner: string,
    name: string,
    current: string[],
    target: string,
  ) => {
    setExcludeError(null);
    try {
      await setRepoExcludedWorkflows(owner, name, current.filter((n) => n !== target));
    } catch (e) {
      setExcludeError(String(e));
    }
  };

  return (
    <div className="settings">
      <h2>Settings</h2>

      <div className="settings-tabs" role="tablist">
        <button
          role="tab"
          aria-selected={tab === "repos"}
          className={tab === "repos" ? "active" : ""}
          onClick={() => setTab("repos")}
        >
          Watched repos
        </button>
        <button
          role="tab"
          aria-selected={tab === "projects"}
          className={tab === "projects" ? "active" : ""}
          onClick={() => setTab("projects")}
        >
          Projects
        </button>
        <button
          role="tab"
          aria-selected={tab === "preferences"}
          className={tab === "preferences" ? "active" : ""}
          onClick={() => setTab("preferences")}
        >
          Preferences
        </button>
      </div>

      {tab === "repos" && (
        <>
      <h3 style={{ marginTop: 16 }}>Watched repos</h3>
      <ul className="repo-list">
        {snapshot.watched.map((w) => {
          const key = `${w.repo.owner}/${w.repo.name}`;
          const repoExcluded = w.excluded_workflows ?? [];
          return (
            <li key={key} style={{ flexDirection: "column", alignItems: "stretch", gap: 6 }}>
              <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                <span style={{ flex: 1 }}>
                  {w.repo.owner}/{w.repo.name}
                  {w.branch_filter && (
                    <span style={{ color: "var(--fg-muted)", marginLeft: 6 }}>
                      ({w.branch_filter})
                    </span>
                  )}
                </span>
                <button
                  className="btn-remove"
                  onClick={() => onRemove(w.repo.owner, w.repo.name)}
                >
                  Remove
                </button>
              </div>
              <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 6 }}>
                <span style={{ color: "var(--fg-muted)", fontSize: 12 }}>
                  Excluded actions:
                </span>
                {repoExcluded.length === 0 && (
                  <span style={{ color: "var(--fg-muted)", fontSize: 12 }}>none</span>
                )}
                {repoExcluded.map((n) => (
                  <span key={n} className="exclude-chip">
                    {n}
                    <button
                      type="button"
                      aria-label={`Remove ${n}`}
                      onClick={() =>
                        onRemoveRepoExclude(w.repo.owner, w.repo.name, repoExcluded, n)
                      }
                    >
                      ×
                    </button>
                  </span>
                ))}
                <input
                  type="text"
                  placeholder="Workflow name"
                  value={repoExcludeInputs[key] ?? ""}
                  onChange={(e) =>
                    setRepoExcludeInputs((p) => ({ ...p, [key]: e.target.value }))
                  }
                  onKeyDown={(e) => {
                    if (e.key === "Enter") {
                      e.preventDefault();
                      onAddRepoExclude(w.repo.owner, w.repo.name, repoExcluded);
                    }
                  }}
                  style={{ flex: "1 1 160px", minWidth: 120 }}
                />
                <button
                  onClick={() =>
                    onAddRepoExclude(w.repo.owner, w.repo.name, repoExcluded)
                  }
                >
                  Exclude
                </button>
              </div>
            </li>
          );
        })}
        {snapshot.watched.length === 0 && (
          <li style={{ color: "var(--fg-muted)" }}>None.</li>
        )}
      </ul>

      <div className="field" style={{ marginTop: 16 }}>
        <label>Add repo</label>
        <div style={{ display: "flex", gap: 8 }}>
          <input
            type="text"
            placeholder="owner/repo"
            value={repoInput}
            onChange={(e) => setRepoInput(e.target.value)}
            style={{ flex: 1 }}
          />
          <input
            type="text"
            placeholder="branch (optional)"
            value={branchInput}
            onChange={(e) => setBranchInput(e.target.value)}
            style={{ flex: 1 }}
          />
          <button className="btn-primary" onClick={onAdd}>Add</button>
        </div>
        {error && <div style={{ color: "#ef4444", fontSize: 12, marginTop: 4 }}>{error}</div>}
      </div>

      <h3 style={{ marginTop: 24 }}>Scan a folder for repos</h3>
      <div style={{ color: "var(--fg-muted)", fontSize: 12, marginBottom: 8 }}>
        Searches up to 5 levels deep for git repos with a GitHub remote.
      </div>
      <div className="field">
        <div style={{ display: "flex", gap: 8 }}>
          <input
            type="text"
            placeholder="/Users/you/Projects or ~/code"
            value={scanPath}
            onChange={(e) => setScanPath(e.target.value)}
            style={{ flex: 1 }}
          />
          <button onClick={onScan} disabled={scanning}>
            {scanning ? "Scanning…" : "Scan"}
          </button>
        </div>
        {scanError && (
          <div style={{ color: "#ef4444", fontSize: 12, marginTop: 4 }}>{scanError}</div>
        )}
        {scanInfo && (
          <div style={{ color: "var(--fg-muted)", fontSize: 12, marginTop: 4 }}>{scanInfo}</div>
        )}
      </div>

      {scanResults && scanResults.length > 0 && (
        <div style={{ marginTop: 12 }}>
          <ul className="repo-list">
            {scanResults.map((r) => {
              const key = `${r.owner}/${r.name}`;
              const already = watchedKeys.has(key);
              return (
                <li key={key}>
                  <label style={{ display: "flex", alignItems: "center", gap: 8, flex: 1 }}>
                    <input
                      type="checkbox"
                      checked={selectedKeys.has(key)}
                      disabled={already}
                      onChange={() => toggleSelected(key)}
                    />
                    <span style={{ flex: 1 }}>
                      {r.owner}/{r.name}
                      {already && (
                        <span style={{ color: "var(--fg-muted)", marginLeft: 6 }}>
                          (already watched)
                        </span>
                      )}
                      <div style={{ color: "var(--fg-muted)", fontSize: 11 }}>{r.path}</div>
                    </span>
                  </label>
                </li>
              );
            })}
          </ul>
          <div style={{ display: "flex", gap: 8, marginTop: 8 }}>
            <button className="btn-primary" onClick={onAddSelected}>
              Add selected ({selectedKeys.size})
            </button>
            <button onClick={() => { setScanResults(null); setSelectedKeys(new Set()); }}>
              Cancel
            </button>
          </div>
        </div>
      )}

        </>
      )}

      {tab === "projects" && (
        <>
      <h3 style={{ marginTop: 16 }}>Projects</h3>
      <div style={{ color: "var(--fg-muted)", fontSize: 12, marginBottom: 8 }}>
        Group repos under a project. A repo joins a project if it matches the
        project's prefix or is added explicitly. Toggle a project off to hide
        its repos from the dashboard and silence its notifications.
      </div>

      <ul className="repo-list">
        {projects.length === 0 && (
          <li style={{ color: "var(--fg-muted)" }}>No projects yet.</li>
        )}
        {projects.map((p) => (
          <li key={p.id} style={{ flexDirection: "column", alignItems: "stretch", gap: 8 }}>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <label className="toggle" title={p.enabled ? "Enabled" : "Disabled"}>
                <input
                  type="checkbox"
                  checked={p.enabled}
                  onChange={(e) => onToggleProject(p, e.target.checked)}
                />
                <span className="toggle-slider" />
              </label>
              <input
                type="text"
                value={p.name}
                onChange={(e) => onRenameProject(p, e.target.value)}
                style={{ flex: 1 }}
              />
              <button
                className="btn-remove"
                onClick={() => onDeleteProject(p)}
              >
                Delete
              </button>
            </div>
            <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
              <span style={{ color: "var(--fg-muted)", fontSize: 12, minWidth: 56 }}>
                Prefix:
              </span>
              <input
                type="text"
                placeholder="e.g. myapp-"
                defaultValue={p.prefix ?? ""}
                onBlur={(e) => {
                  const v = e.target.value;
                  if ((p.prefix ?? "") !== v) onSetPrefix(p, v);
                }}
                style={{ flex: 1 }}
              />
            </div>
            <div>
              <div style={{ color: "var(--fg-muted)", fontSize: 12, marginBottom: 4 }}>
                Members ({snapshot.watched.filter((w) => repoInProject(w.repo, p)).length}
                {" "}of {snapshot.watched.length} watched)
              </div>
              <div style={{ display: "flex", flexWrap: "wrap", gap: 6 }}>
                {snapshot.watched.length === 0 && (
                  <span style={{ color: "var(--fg-muted)", fontSize: 12 }}>
                    No watched repos to assign.
                  </span>
                )}
                {snapshot.watched.map((w) => {
                  const inProject = repoInProject(w.repo, p);
                  const explicit = p.member_repos.some(
                    (m) => m.owner === w.repo.owner && m.name === w.repo.name,
                  );
                  const matchedByPrefix = inProject && !explicit;
                  return (
                    <label
                      key={`${w.repo.owner}/${w.repo.name}`}
                      className="exclude-chip"
                      style={{
                        opacity: matchedByPrefix ? 0.7 : 1,
                        cursor: matchedByPrefix ? "default" : "pointer",
                      }}
                      title={matchedByPrefix ? "Matched by prefix" : ""}
                    >
                      <input
                        type="checkbox"
                        checked={inProject}
                        disabled={matchedByPrefix}
                        onChange={(e) =>
                          onToggleMember(
                            p,
                            { owner: w.repo.owner, name: w.repo.name },
                            e.target.checked,
                          )
                        }
                        style={{ marginRight: 4 }}
                      />
                      {w.repo.owner}/{w.repo.name}
                    </label>
                  );
                })}
              </div>
            </div>
          </li>
        ))}
      </ul>

      <div className="field" style={{ marginTop: 16 }}>
        <label>New project</label>
        <div style={{ display: "flex", gap: 8 }}>
          <input
            type="text"
            placeholder="Project name"
            value={newProjectName}
            onChange={(e) => setNewProjectName(e.target.value)}
            style={{ flex: 1 }}
          />
          <input
            type="text"
            placeholder="prefix (optional, e.g. myapp-)"
            value={newProjectPrefix}
            onChange={(e) => setNewProjectPrefix(e.target.value)}
            style={{ flex: 1 }}
          />
          <button className="btn-primary" onClick={onCreateProject}>
            Create
          </button>
        </div>
        {projectError && (
          <div style={{ color: "#ef4444", fontSize: 12, marginTop: 4 }}>{projectError}</div>
        )}
      </div>
        </>
      )}

      {tab === "preferences" && (
        <>
      <h3 style={{ marginTop: 16 }}>Excluded actions (global)</h3>
      <div style={{ color: "var(--fg-muted)", fontSize: 12, marginBottom: 8 }}>
        Workflow runs whose name matches any of these (case-insensitive) are
        hidden across all PRs and watched repos and never trigger notifications.
      </div>
      <div style={{ display: "flex", flexWrap: "wrap", alignItems: "center", gap: 6 }}>
        {globalExcluded.length === 0 && (
          <span style={{ color: "var(--fg-muted)", fontSize: 12 }}>none</span>
        )}
        {globalExcluded.map((n) => (
          <span key={n} className="exclude-chip">
            {n}
            <button
              type="button"
              aria-label={`Remove ${n}`}
              onClick={() => onRemoveGlobalExclude(n)}
            >
              ×
            </button>
          </span>
        ))}
        <input
          type="text"
          placeholder="Workflow name"
          value={globalExcludeInput}
          onChange={(e) => setGlobalExcludeInput(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter") {
              e.preventDefault();
              onAddGlobalExclude();
            }
          }}
          style={{ flex: "1 1 200px", minWidth: 140 }}
        />
        <button onClick={onAddGlobalExclude}>Exclude</button>
      </div>
      {excludeError && (
        <div style={{ color: "#ef4444", fontSize: 12, marginTop: 4 }}>{excludeError}</div>
      )}

      <h3 style={{ marginTop: 24 }}>Notifications</h3>
      <label>
        <input
          type="checkbox"
          checked={prefs.on_failure}
          onChange={(e) => onPrefChange({ on_failure: e.target.checked })}
        />
        Notify on transition to failure
      </label>
      <label>
        <input
          type="checkbox"
          checked={prefs.on_first_success}
          onChange={(e) => onPrefChange({ on_first_success: e.target.checked })}
        />
        Notify when a PR turns all green
      </label>
      <label>
        <input
          type="checkbox"
          checked={prefs.on_every_transition}
          onChange={(e) => onPrefChange({ on_every_transition: e.target.checked })}
        />
        Notify on every state transition (noisy)
      </label>

      <h3 style={{ marginTop: 24 }}>App</h3>
      <label>
        <input
          type="checkbox"
          checked={launchAtLogin ?? false}
          disabled={launchAtLogin === null}
          onChange={(e) => onToggleLaunchAtLogin(e.target.checked)}
        />
        Open Driftless at login
      </label>
      {launchAtLoginError && (
        <div style={{ color: "#ef4444", fontSize: 12, marginTop: 4 }}>
          {launchAtLoginError}
        </div>
      )}

      <h3 style={{ marginTop: 24 }}>Debug logging</h3>
      <div style={{ color: "var(--fg-muted)", fontSize: 12, marginBottom: 8 }}>
        Writes verbose logs and any crashes/panics to a file in the app log
        folder. Toggle takes effect on next launch.
      </div>
      <label>
        <input
          type="checkbox"
          checked={debugLogging}
          onChange={(e) => onToggleDebugLogging(e.target.checked)}
        />
        Enable debug logging
      </label>
      <div style={{ marginTop: 8 }}>
        <button onClick={onOpenLogsFolder}>Open logs folder</button>
      </div>
      {debugLoggingError && (
        <div style={{ color: "#ef4444", fontSize: 12, marginTop: 4 }}>
          {debugLoggingError}
        </div>
      )}

      <div style={{ marginTop: 12 }}>
        <button onClick={() => quitApp()}>Quit Driftless</button>
      </div>
        </>
      )}
    </div>
  );
}
