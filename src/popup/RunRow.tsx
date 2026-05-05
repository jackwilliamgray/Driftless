import { StatusIcon } from "../components/StatusIcon";
import type { WorkflowRun } from "../lib/types";
import { openInBrowser } from "../lib/invoke";

export function RunRow({ run }: { run: WorkflowRun }) {
  return (
    <button
      className="popup-row"
      onClick={() => openInBrowser(run.html_url)}
      title={`${run.workflow_name} · ${run.head_branch}`}
    >
      <StatusIcon status={run.status} conclusion={run.conclusion} />
      <span className="name">{run.workflow_name || run.name}</span>
    </button>
  );
}
