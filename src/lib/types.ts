export type RunStatus = "queued" | "in_progress" | "completed" | "waiting" | "requested" | "pending";

export type RunConclusion =
  | "success"
  | "failure"
  | "cancelled"
  | "skipped"
  | "timed_out"
  | "action_required"
  | "neutral"
  | "stale"
  | null;

export type AggregateState = "idle" | "pending" | "success" | "failure";

export type ReviewDecision = "APPROVED" | "CHANGES_REQUESTED" | "REVIEW_REQUIRED" | null;

export interface AuthStatus {
  logged_in: boolean;
  login: string | null;
  error: string | null;
}

export interface RepoRef {
  owner: string;
  name: string;
}

export interface WorkflowRun {
  id: number;
  name: string;
  status: RunStatus;
  conclusion: RunConclusion;
  head_branch: string;
  head_sha: string;
  event: string;
  html_url: string;
  workflow_id: number;
  workflow_name: string;
  created_at: string;
  updated_at: string;
}

export interface PullRequestSummary {
  repo: RepoRef;
  number: number;
  title: string;
  branch: string;
  head_sha: string;
  url: string;
  is_draft: boolean;
  state: "OPEN" | "CLOSED" | "MERGED";
  review_decision: ReviewDecision;
  runs: WorkflowRun[];
  aggregate: AggregateState;
}

export interface WatchedRepoState {
  repo: RepoRef;
  branch_filter: string | null;
  recent_runs: WorkflowRun[];
  aggregate: AggregateState;
  excluded_workflows: string[];
  dismissed: boolean;
}

export interface AppSnapshot {
  auth: AuthStatus;
  prs: PullRequestSummary[];
  watched: WatchedRepoState[];
  aggregate: AggregateState;
  last_polled_at: string | null;
  next_poll_at: string | null;
  rate_limit_remaining: number | null;
  global_excluded_workflows: string[];
  projects: Project[];
  debug_logging: boolean;
}

export interface ProjectRepoRef {
  owner: string;
  name: string;
}

export interface Project {
  id: string;
  name: string;
  prefix: string | null;
  member_repos: ProjectRepoRef[];
  enabled: boolean;
}

export interface JobStep {
  name: string;
  status: RunStatus;
  conclusion: RunConclusion;
  number: number;
}

export interface WorkflowJob {
  id: number;
  name: string;
  status: RunStatus;
  conclusion: RunConclusion;
  html_url: string;
  steps: JobStep[];
  started_at: string | null;
  completed_at: string | null;
}

export interface NotificationPrefs {
  on_failure: boolean;
  on_first_success: boolean;
  on_every_transition: boolean;
}

export interface PollError {
  message: string;
  at: string;
}

export interface DiscoveredRepo {
  owner: string;
  name: string;
  path: string;
  remote_url: string;
}

export interface WatchedRepoConfig {
  owner: string;
  name: string;
  branch_filter: string | null;
  excluded_workflows?: string[];
}
