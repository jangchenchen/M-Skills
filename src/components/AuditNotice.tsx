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

function warningDescription(
  t: (key: string, opts?: Record<string, unknown>) => string,
  w: AuditWarningDto
): string {
  const kind = w.kind;
  const detailKey = w.detailKey;
  if (detailKey) {
    const specific = t(`warningExplain.${kind}.${detailKey}`, { defaultValue: "" });
    if (specific) return specific;
  }
  const fallback = t(`warningExplain.${kind}._default`, { defaultValue: "" });
  if (fallback) return fallback;
  return w.message;
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
            <p className="text-xs font-semibold mb-1.5">
              {t(`severity.${sev}`)} ({buckets[sev].length})
            </p>
            <ul className="space-y-2.5">
              {buckets[sev].map((w, i) => (
                <li key={i} className="text-xs">
                  <div className="flex items-center gap-1.5 mb-0.5">
                    <span className="font-mono opacity-70 text-[10px]">
                      {w.path || "(total)"}
                    </span>
                    <span className="opacity-50">·</span>
                    <span className="font-medium">
                      {t(`warningKind.${camelCaseKind(w.kind)}`)}
                    </span>
                  </div>
                  <p className="leading-relaxed opacity-90">
                    {warningDescription(t, w)}
                  </p>
                  {w.detail && (
                    <p className="mt-0.5 text-[10px] opacity-60">
                      {t("warningEvidence")}:
                      {" "}
                      <code className="px-1 py-0.5 rounded bg-black/30 font-mono">
                        {w.detail}
                      </code>
                    </p>
                  )}
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
