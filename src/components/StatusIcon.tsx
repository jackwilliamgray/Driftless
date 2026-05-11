import type { AggregateState, RunConclusion, RunStatus } from "../lib/types";

interface StatusIconProps {
  status?: RunStatus;
  conclusion?: RunConclusion;
  aggregate?: AggregateState;
  dismissed?: boolean;
  size?: number;
  title?: string;
}

function classifyRun(status?: RunStatus, conclusion?: RunConclusion): AggregateState {
  if (!status) return "idle";
  if (status === "completed") {
    if (conclusion === "success") return "success";
    if (conclusion === "failure" || conclusion === "timed_out") return "failure";
    if (conclusion === "cancelled" || conclusion === "skipped" || conclusion === "neutral") return "idle";
    return "idle";
  }
  return "pending";
}

export function StatusIcon({ status, conclusion, aggregate, dismissed, size = 14, title }: StatusIconProps) {
  const state = aggregate ?? classifyRun(status, conclusion);
  const stroke = 2;
  const r = size / 2 - stroke;
  const cx = size / 2;

  const colors: Record<AggregateState, string> = {
    success: "#22c55e",
    failure: "#ef4444",
    pending: "#eab308",
    idle: "#6b7280",
  };
  const dismissedColor = "#6b7280";
  const color = dismissed ? dismissedColor : colors[state];
  const label = dismissed ? `${state} (dismissed)` : state;

  return (
    <span
      role="img"
      aria-label={label}
      title={title ?? label}
      className={`status-icon status-${state}${dismissed ? " status-dismissed" : ""}`}
      style={{ display: "inline-flex", width: size, height: size }}
    >
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        <circle cx={cx} cy={cx} r={r} fill="none" stroke={color} strokeWidth={stroke} />
        {dismissed ? (
          <path
            d={`M${size * 0.28} ${size * 0.72} L${size * 0.72} ${size * 0.28}`}
            stroke={color}
            strokeWidth={stroke}
            strokeLinecap="round"
          />
        ) : (
          <>
            {state === "success" && (
              <path
                d={`M${size * 0.27} ${size * 0.52} L${size * 0.45} ${size * 0.7} L${size * 0.74} ${size * 0.32}`}
                fill="none"
                stroke={color}
                strokeWidth={stroke}
                strokeLinecap="round"
                strokeLinejoin="round"
              />
            )}
            {state === "failure" && (
              <>
                <path
                  d={`M${size * 0.32} ${size * 0.32} L${size * 0.68} ${size * 0.68}`}
                  stroke={color}
                  strokeWidth={stroke}
                  strokeLinecap="round"
                />
                <path
                  d={`M${size * 0.68} ${size * 0.32} L${size * 0.32} ${size * 0.68}`}
                  stroke={color}
                  strokeWidth={stroke}
                  strokeLinecap="round"
                />
              </>
            )}
            {state === "pending" && (
              <circle
                className="spin-half"
                cx={cx}
                cy={cx}
                r={r}
                fill="none"
                stroke={color}
                strokeWidth={stroke}
                strokeDasharray={`${Math.PI * r} ${Math.PI * r}`}
              />
            )}
          </>
        )}
      </svg>
    </span>
  );
}
