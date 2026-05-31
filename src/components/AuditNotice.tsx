import { useTranslation } from "react-i18next";
import type { AuditSeverity, AuditWarningDto } from "../types";

export function isAtLeast(
  level: AuditSeverity,
  threshold: AuditSeverity
): boolean {
  const order: Record<AuditSeverity, number> = { low: 0, medium: 1, high: 2 };
  return order[level] >= order[threshold];
}

export function RiskBadge({ level }: { level: AuditSeverity }) {
  const { t } = useTranslation("wizard");
  const styles: Record<AuditSeverity, string> = {
    low: "bg-emerald-950 text-emerald-300 border-emerald-900",
    medium: "bg-amber-950 text-amber-300 border-amber-900",
    high: "bg-red-950 text-red-300 border-red-900",
  };
  return (
    <div
      className={`inline-flex items-center gap-2 px-3 py-1 rounded border text-xs font-medium ${styles[level]}`}
    >
      <span className="uppercase tracking-wide">{t("riskBadge")}</span>
      <span>{t(`risk.${level}`)}</span>
    </div>
  );
}

export function WarningsSection({
  warnings,
}: {
  warnings: AuditWarningDto[];
}) {
  const { t } = useTranslation("wizard");
  if (warnings.length === 0) return null;

  const buckets: Record<AuditSeverity, AuditWarningDto[]> = {
    high: warnings.filter((w) => w.severity === "high"),
    medium: warnings.filter((w) => w.severity === "medium"),
    low: warnings.filter((w) => w.severity === "low"),
  };
  const bucketStyles: Record<AuditSeverity, string> = {
    high: "bg-red-950/40 border-red-900/60 text-red-300",
    medium: "bg-amber-950/30 border-amber-900/50 text-amber-300",
    low: "bg-gray-900/40 border-gray-800 text-gray-400",
  };

  return (
    <div className="space-y-2">
      {(["high", "medium", "low"] as AuditSeverity[]).map((sev) =>
        buckets[sev].length === 0 ? null : (
          <div
            key={sev}
            className={`rounded border px-3 py-2 ${bucketStyles[sev]}`}
          >
            <p className="text-xs font-semibold mb-1">
              {t(`severity.${sev}`)} ({buckets[sev].length})
            </p>
            <ul className="space-y-1">
              {buckets[sev].map((w, i) => (
                <li key={i} className="text-xs">
                  <span className="font-mono opacity-80">
                    {w.path || "(total)"}
                  </span>{" "}
                  — [{t(`warningKind.${camelCaseKind(w.kind)}`)}] {w.message}
                </li>
              ))}
            </ul>
          </div>
        )
      )}
    </div>
  );
}

function camelCaseKind(kind: AuditWarningDto["kind"]): string {
  return kind.charAt(0).toLowerCase() + kind.slice(1);
}
