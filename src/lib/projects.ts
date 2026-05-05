import type { Project, RepoRef } from "./types";

export function repoInProject(repo: RepoRef, project: Project): boolean {
  if (
    project.member_repos.some(
      (m) => m.owner === repo.owner && m.name === repo.name,
    )
  ) {
    return true;
  }
  if (project.prefix && project.prefix.length > 0) {
    if (repo.name.startsWith(project.prefix)) return true;
  }
  return false;
}

export function projectForRepo(
  repo: RepoRef,
  projects: Project[],
): Project | null {
  for (const p of projects) {
    if (repoInProject(repo, p)) return p;
  }
  return null;
}

export function isRepoEnabled(repo: RepoRef, projects: Project[]): boolean {
  const p = projectForRepo(repo, projects);
  return p ? p.enabled : true;
}
