import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { getDashboard } from "../api";
import type {
  ArtifactKind,
  DashboardAttentionItemDto,
  DashboardKindSummaryDto,
  DashboardToolSummaryDto,
  RecentActionDto,
} from "../types";

interface Props {
  onImportClick: () => void;
  onSettingsClick: () => void;
  onRescan: () => void;
  onKindFilter: (kind: ArtifactKind) => void;
  onToolFilter: (tool: string) => void;
  onArtifactSelect: (name: string, kind: ArtifactKind) => void;
}

export function DashboardPanel({
  onImportClick,
  onSettingsClick,
  onRescan,
  onKindFilter,
  onToolFilter,
  onArtifactSelect,
}: Props) {
  const { t } = useTranslation("common");
  const { t: ta } = useTranslation("artifact");

  const { data, isLoading, error } = useQuery({
    queryKey: ["dashboard"],
    queryFn: () => getDashboard(),
  });

  if (isLoading) {
    return (
      <div className="flex h-full items-center justify-center text-gray-500 text-sm">
        {t("scanning")}
      </div>
    );
  }

  if (error || !data) {
    return (
      <div className="flex h-full items-center justify-center text-red-400 text-sm px-6">
        {String(error ?? "Dashboard unavailable")}
      </div>
    );
  }

  const availableCount = data.toolSummaries.filter((s) => s.available).length;
  const hasErrors = data.scanErrors.length > 0;
  const scanStatus = hasErrors
    ? t("scanErrors", { count: data.scanErrors.length })
    : t("scanOk");
  const tc = (key: string, options?: Record<string, unknown>) =>
    String(t(key, options));
  const taText = (key: string) => String(ta(key));

  return (
    <div className="flex flex-col h-full overflow-y-auto px-6 py-5 space-y-6">
      {/* Header */}
      <div>
        <h2 className="text-base font-semibold text-gray-100">
          {t("localLibrary")}
        </h2>
        <div className="mt-1 flex items-center gap-4 text-xs text-gray-400">
          <span>
            <span className="text-2xl font-bold text-gray-100 mr-1.5">
              {data.readyArtifactGroups}
            </span>
            {t("readyArtifacts")}
          </span>
          <span className="text-gray-600">·</span>
          <span className={hasErrors ? "text-amber-400" : "text-emerald-400"}>
            {scanStatus}
          </span>
          <span className="text-gray-600">·</span>
          <span>{t("toolsAvailable", { count: availableCount })}</span>
        </div>
      </div>

      {/* Kind tiles */}
      <div className="grid grid-cols-3 gap-3">
        {data.kindSummaries.map((ks) => (
          <KindTile
            key={ks.kind}
            summary={ks}
            t={tc}
            ta={taText}
            onSelect={() => onKindFilter(ks.kind)}
          />
        ))}
      </div>

      <div className="grid grid-cols-2 gap-4">
        {/* Tool coverage */}
        <div>
          <p className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-2">
            {t("toolCoverage")}
          </p>
          <div className="space-y-1.5">
            {data.toolSummaries.map((ts) => (
              <ToolRow
                key={ts.adapterId}
                tool={ts}
                t={tc}
                onSelect={() => onToolFilter(ts.adapterId)}
              />
            ))}
          </div>
        </div>

        {/* Needs attention */}
        <div>
          <p className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-2">
            {t("needsAttention")}
          </p>
          {data.attentionItems.length === 0 ? (
            <p className="text-xs text-gray-600">{t("noAttentionItems")}</p>
          ) : (
            <div className="space-y-2">
              {data.attentionItems.map((item, i) => (
                <AttentionRow
                  key={i}
                  item={item}
                  t={tc}
                  onImportClick={onImportClick}
                  onSettingsClick={onSettingsClick}
                  onRescan={onRescan}
                  onKindFilter={onKindFilter}
                  onToolFilter={onToolFilter}
                  onArtifactSelect={onArtifactSelect}
                />
              ))}
            </div>
          )}
        </div>
      </div>

      {/* Recent Safe Actions */}
      <div>
        <p className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-2">
          {t("recentSafeActions")}
        </p>
        {data.recentActions.length === 0 ? (
          <p className="text-xs text-gray-600">{t("noRecentActions")}</p>
        ) : (
          <div className="space-y-1">
            {data.recentActions.map((action, i) => (
              <RecentActionRow key={i} action={action} t={tc} />
            ))}
          </div>
        )}
      </div>

      {/* Empty state CTA */}
      {data.totalGroups === 0 && (
        <div className="border border-dashed border-gray-700 rounded-lg px-6 py-8 text-center">
          <p className="text-sm text-gray-500 mb-3">{t("noArtifacts")}</p>
          <button
            onClick={onImportClick}
            className="text-sm bg-indigo-600 hover:bg-indigo-500 text-white px-4 py-2 rounded"
          >
            {t("importFirst")}
          </button>
        </div>
      )}
    </div>
  );
}

function KindTile({
  summary,
  t,
  ta,
  onSelect,
}: {
  summary: DashboardKindSummaryDto;
  t: (k: string, options?: Record<string, unknown>) => string;
  ta: (k: string) => string;
  onSelect: () => void;
}) {
  const sharedCount = Math.max(
    0,
    summary.visibleInstallations - summary.ownedInstallations
  );

  return (
    <button
      type="button"
      onClick={onSelect}
      className="bg-gray-900 border border-gray-800 rounded-lg px-4 py-3 text-left hover:border-indigo-500/70 hover:bg-gray-900/80 focus:outline-none focus:ring-2 focus:ring-indigo-500/70 transition-colors"
      title={t("filterByKind", { kind: ta(`kindPlural.${summary.kind}`) })}
    >
      <p className="text-xs text-gray-500 font-medium uppercase tracking-wider">
        {ta(`kindPlural.${summary.kind}`)}
      </p>
      <p className="mt-1 text-xl font-bold text-gray-100">
        {summary.ownedInstallations}
      </p>
      {sharedCount > 0 && (
        <p className="text-xs text-gray-500 mt-0.5">
          {t("sharedCount", { count: sharedCount })}
        </p>
      )}
      <p className="text-xs text-gray-500 mt-0.5">
        {summary.groups} {summary.groups === 1 ? t("group") : t("groups")}
      </p>
      {summary.compatibleTargets.length > 0 && (
        <p className="text-xs text-gray-600 mt-1 truncate">
          {summary.compatibleTargets.map((t) => t.tool).join(" · ")}
        </p>
      )}
    </button>
  );
}

function ToolRow({
  tool,
  t,
  onSelect,
}: {
  tool: DashboardToolSummaryDto;
  t: (k: string, options?: Record<string, unknown>) => string;
  onSelect: () => void;
}) {
  const isMissing = !tool.available;
  const isEmpty = tool.available && tool.ownedInstallations === 0;
  const sharedCount = Math.max(
    0,
    tool.visibleInstallations - tool.ownedInstallations
  );

  const dotColor = isMissing
    ? "bg-gray-600"
    : isEmpty
      ? "bg-amber-400/60 ring-1 ring-amber-400/30"
      : tool.writable
        ? "bg-emerald-400"
        : "bg-blue-400";

  const nameClass = isMissing
    ? "text-gray-600 line-through"
    : isEmpty
      ? "text-gray-400"
      : "text-gray-300";

  const statusLabel = tool.available
    ? tool.writable
      ? t("writable")
      : t("readOnly")
    : t("missing");

  return (
    <button
      type="button"
      onClick={onSelect}
      className="flex w-full items-center gap-2 rounded px-1.5 py-1 text-left text-xs hover:bg-gray-900 focus:outline-none focus:ring-2 focus:ring-indigo-500/70"
      title={t("filterByTool", { tool: tool.adapterId })}
    >
      <span className={`h-2 w-2 rounded-full flex-none ${dotColor}`} />
      <span className={`w-28 truncate ${nameClass}`}>{tool.adapterId}</span>
      <span className="min-w-0 flex-1 truncate text-gray-600">
        {statusLabel}
        {isMissing && tool.missingReason ? (
          <span className="ml-1 text-gray-700">{tool.missingReason}</span>
        ) : null}
      </span>
      <span
        className={`ml-auto flex-none text-right ${
          isEmpty ? "text-amber-400/60" : "text-gray-500"
        }`}
      >
        {isMissing
          ? "—"
          : isEmpty
            ? t("empty")
            : `${tool.ownedInstallations}${
                sharedCount > 0
                  ? ` · ${t("sharedCount", { count: sharedCount })}`
                  : ""
              }`}
      </span>
    </button>
  );
}

function AttentionRow({
  item,
  t,
  onImportClick,
  onSettingsClick,
  onRescan,
  onKindFilter,
  onToolFilter,
  onArtifactSelect,
}: {
  item: DashboardAttentionItemDto;
  t: (k: string) => string;
  onImportClick: () => void;
  onSettingsClick: () => void;
  onRescan: () => void;
  onKindFilter: (kind: ArtifactKind) => void;
  onToolFilter: (tool: string) => void;
  onArtifactSelect: (name: string, kind: ArtifactKind) => void;
}) {
  const severityColor =
    item.severity === "critical"
      ? "text-red-400"
      : item.severity === "warning"
        ? "text-amber-400"
        : item.severity === "info"
          ? "text-blue-400"
        : "text-gray-400";

  const actionLabel =
    item.action === "open_settings"
      ? t("attentionActionOpenSettings")
      : item.action === "open_import"
        ? t("attentionActionOpenImport")
        : item.action === "rescan"
          ? t("attentionActionRescan")
          : item.action === "filter_kind" && item.kind
            ? t("attentionActionFilterKind")
            : item.action === "filter_tool" && item.tool
              ? t("attentionActionFilterTool")
              : item.action === "select_artifact" &&
                  item.kind &&
                  item.artifactName
                ? t("attentionActionSelectArtifact")
                : null;

  const handleAction = () => {
    if (item.action === "open_settings") onSettingsClick();
    else if (item.action === "open_import") onImportClick();
    else if (item.action === "rescan") onRescan();
    else if (item.action === "filter_kind" && item.kind) {
      onKindFilter(item.kind);
    } else if (item.action === "filter_tool" && item.tool) {
      onToolFilter(item.tool);
    } else if (
      item.action === "select_artifact" &&
      item.kind &&
      item.artifactName
    ) {
      onArtifactSelect(item.artifactName, item.kind);
    }
  };

  return (
    <div className="bg-gray-900 border border-gray-800 rounded px-3 py-2">
      <p className={`text-xs font-medium ${severityColor}`}>{item.title}</p>
      <p className="text-xs text-gray-500 mt-0.5">{item.body}</p>
      {actionLabel && (
        <button
          onClick={handleAction}
          className="mt-1 text-xs text-indigo-400 hover:text-indigo-300"
        >
          {actionLabel} →
        </button>
      )}
    </div>
  );
}

function RecentActionRow({
  action,
  t,
}: {
  action: RecentActionDto;
  t: (k: string) => string;
}) {
  const label =
    t(`actionType_${action.eventType}`) || action.eventType;
  const when = new Date(action.occurredAt).toLocaleString(undefined, {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  });
  return (
    <div className="flex items-baseline justify-between gap-2 text-xs">
      <span className="text-gray-400 truncate">
        <span className={action.succeeded ? "text-gray-300" : "text-amber-400"}>
          {label}
        </span>
        {action.artifactName && (
          <span className="text-gray-500 ml-1">{action.artifactName}</span>
        )}
        {action.target && (
          <span className="text-gray-600 ml-1">→ {action.target}</span>
        )}
        {!action.succeeded && (
          <span className="text-amber-500 ml-1">({t("actionFailed")})</span>
        )}
      </span>
      <span className="text-gray-600 shrink-0">{when}</span>
    </div>
  );
}
