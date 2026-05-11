import type { ReactElement } from "react";
import type { ReviewDecision } from "../lib/types";

interface ReviewIconProps {
  decision: ReviewDecision;
  size?: number;
}

const colors = {
  approved: "#22c55e",
  changes: "#ef4444",
  pending: "#eab308",
};

export function ReviewIcon({ decision, size = 12 }: ReviewIconProps) {
  const stroke = 2;
  const r = size / 2 - stroke;
  const cx = size / 2;

  let color: string;
  let label: string;
  let glyph: ReactElement;

  if (decision === "APPROVED") {
    color = colors.approved;
    label = "Approved";
    glyph = (
      <path
        d={`M${size * 0.27} ${size * 0.52} L${size * 0.45} ${size * 0.7} L${size * 0.74} ${size * 0.32}`}
        fill="none"
        stroke={color}
        strokeWidth={stroke}
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    );
  } else if (decision === "CHANGES_REQUESTED") {
    color = colors.changes;
    label = "Changes requested";
    glyph = (
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
    );
  } else {
    color = colors.pending;
    label = decision === "REVIEW_REQUIRED" ? "Review required" : "Review pending";
    glyph = <circle cx={cx} cy={cx} r={stroke * 0.6} fill={color} />;
  }

  return (
    <span
      role="img"
      aria-label={label}
      title={label}
      className="review-icon"
      style={{ display: "inline-flex", width: size, height: size, flex: "0 0 auto" }}
    >
      <svg width={size} height={size} viewBox={`0 0 ${size} ${size}`}>
        <circle cx={cx} cy={cx} r={r} fill="none" stroke={color} strokeWidth={stroke} />
        {glyph}
      </svg>
    </span>
  );
}
