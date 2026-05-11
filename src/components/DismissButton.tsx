import type { MouseEvent } from "react";
import { dismissWatchedRepo, undismissWatchedRepo } from "../lib/invoke";
import type { RepoRef } from "../lib/types";

interface DismissButtonProps {
  repo: RepoRef;
  dismissed: boolean;
  size?: number;
}

export function DismissButton({ repo, dismissed, size = 14 }: DismissButtonProps) {
  const onClick = (e: MouseEvent) => {
    e.stopPropagation();
    e.preventDefault();
    const action = dismissed ? undismissWatchedRepo : dismissWatchedRepo;
    action(repo.owner, repo.name).catch(() => {});
  };

  const label = dismissed
    ? "Restore — show this repo in status again"
    : "Dismiss — acknowledge failure";
  const stroke = 1.6;
  const cx = size / 2;
  const color = dismissed ? "var(--fg-muted)" : "currentColor";

  return (
    <button
      type="button"
      className={`dismiss-btn${dismissed ? " is-dismissed" : ""}`}
      onClick={onClick}
      title={label}
      aria-label={label}
    >
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`} aria-hidden>
        {dismissed ? (
          <path
            d={`M${size * 0.22} ${cx} L${size * 0.44} ${size * 0.72} L${size * 0.78} ${size * 0.3}`}
            fill="none"
            stroke={color}
            strokeWidth={stroke}
            strokeLinecap="round"
            strokeLinejoin="round"
          />
        ) : (
          <>
            <path
              d={`M${size * 0.28} ${size * 0.28} L${size * 0.72} ${size * 0.72}`}
              stroke={color}
              strokeWidth={stroke}
              strokeLinecap="round"
            />
            <path
              d={`M${size * 0.72} ${size * 0.28} L${size * 0.28} ${size * 0.72}`}
              stroke={color}
              strokeWidth={stroke}
              strokeLinecap="round"
            />
          </>
        )}
      </svg>
    </button>
  );
}
