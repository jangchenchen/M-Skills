import type { CompatibilityReviewDto } from "../types";
import { targetLabel } from "../types";

interface Props {
  reviews: CompatibilityReviewDto[];
  /** When true, also list per-target reasons under the summary. Default true. */
  showReasons?: boolean;
  /** Max number of warnings to render (after summary/reasons). Default 5. */
  maxWarnings?: number;
}

export function CompatibilityNotice({
  reviews,
  showReasons = true,
  maxWarnings = 5,
}: Props) {
  if (reviews.length === 0) return null;
  const main =
    reviews.find((r) => r.status === "incompatible") ??
    reviews.find((r) => r.status === "warning") ??
    reviews[0];
  const warnings = reviews.flatMap((r) =>
    r.warnings.map((warning) => `${targetLabel(r.target)}: ${warning}`)
  );
  return (
    <div className="rounded border border-yellow-800 bg-yellow-950/40 px-3 py-2 text-xs text-yellow-100">
      <p className="font-medium">{main.summary}</p>
      {(showReasons || warnings.length > 0) && (
        <ul className="mt-1 list-disc list-inside space-y-0.5 text-yellow-200/90">
          {showReasons &&
            main.reasons.map((reason, i) => (
              <li key={`reason-${i}`}>{reason}</li>
            ))}
          {warnings.slice(0, maxWarnings).map((warning, i) => (
            <li key={`warning-${i}`}>{warning}</li>
          ))}
        </ul>
      )}
    </div>
  );
}
