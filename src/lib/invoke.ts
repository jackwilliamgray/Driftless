import { invoke } from "@tauri-apps/api/core";
import type {
  AppSnapshot,
  AuthStatus,
  DiscoveredRepo,
  NotificationPrefs,
  Project,
  ProjectRepoRef,
  RepoRef,
  WatchedRepoConfig,
  WorkflowJob,
} from "./types";

export const getAuthStatus = () => invoke<AuthStatus>("auth_status");
export const getSnapshot = () => invoke<AppSnapshot>("get_state");
export const getRunJobs = (repo: RepoRef, runId: number) =>
  invoke<WorkflowJob[]>("get_run_jobs", { repo, runId });
export const addWatchedRepo = (owner: string, name: string, branchFilter?: string) =>
  invoke<void>("add_watched_repo", { owner, name, branchFilter: branchFilter ?? null });
export const addWatchedReposBulk = (repos: WatchedRepoConfig[]) =>
  invoke<number>("add_watched_repos_bulk", { repos });
export const scanPathForRepos = (path: string) =>
  invoke<DiscoveredRepo[]>("scan_path_for_repos", { path });
export const removeWatchedRepo = (owner: string, name: string) =>
  invoke<void>("remove_watched_repo", { owner, name });
export const setNotificationPrefs = (prefs: NotificationPrefs) =>
  invoke<void>("set_notification_prefs", { prefs });
export const setGlobalExcludedWorkflows = (names: string[]) =>
  invoke<string[]>("set_global_excluded_workflows", { names });
export const setRepoExcludedWorkflows = (
  owner: string,
  name: string,
  names: string[],
) => invoke<string[]>("set_repo_excluded_workflows", { owner, name, names });
export const openInBrowser = (url: string) => invoke<void>("open_run_in_browser", { url });
export const forceRefresh = () => invoke<void>("force_refresh");
export const showDashboard = () => invoke<void>("show_dashboard");
export const hidePopup = () => invoke<void>("hide_popup");
export const quitApp = () => invoke<void>("quit_app");
export const upsertProject = (project: Project) =>
  invoke<Project[]>("upsert_project", { project });
export const removeProject = (id: string) =>
  invoke<Project[]>("remove_project", { id });
export const setProjectEnabled = (id: string, enabled: boolean) =>
  invoke<Project[]>("set_project_enabled", { id, enabled });
export const setProjectMembers = (id: string, members: ProjectRepoRef[]) =>
  invoke<Project[]>("set_project_members", { id, members });
