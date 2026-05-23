import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  clearTranslationCache,
  confirmInstallSkillDraft,
  generateSkillSummary,
  getSkillSummary,
  previewAdaptSkillForCodex,
  previewForkSkill,
  translateArtifact,
} from "../api";
import { disable, enable, uninstall } from "../api";
import { reviewArtifactCompatibility } from "../api";
import { useErrorMessage } from "../useErrorMessage";
import type {
  ArtifactDto,
  ArtifactGroupDto,
  ArtifactKind,
  CompatibilityReviewDto,
  ConfirmDraftInstallRequest,
  InstallationDto,
  LineageDto,
  MarkdownWarningDto,
  ScannedInstallationDto,
  SkillDraftMode,
  SkillDraftPreviewDto,
  SkillSummaryDto,
  TargetDto,
  TranslateOutcomeDto,
} from "../types";
import { sourceLabel, targetLabel } from "../types";
import { CompatibilityNotice } from "./CompatibilityNotice";
import { CustomSkillEditor } from "./CustomSkillEditor";
import { SkillPreviewModal } from "./SkillPreviewModal";

type DraftFlow =
  | { kind: "idle" }
  | { kind: "preview"; mode: SkillDraftMode; data: SkillDraftPreviewDto }
  | { kind: "forkPicker"; artifact: ArtifactDto }
  | {
      kind: "editor";
      initialContent: string;
      target: TargetDto;
      lineage: LineageDto;
    };

const FORK_TARGET_TOOLS = ["claude-code", "codex", "opencode"] as const;

const TRANSLATE_LOCALE = "zh";
const TRANSLATE_FIELD = "body";
const TRANSLATE_FILE = "SKILL.md";

interface Props {
  groups: ArtifactGroupDto[];
  selectedName: string | null;
  selectedKind: ArtifactKind | null;
}

type BadgeKey = "cached" | "justTranslated" | "refreshed" | "passthrough";

function badgeKey(outcome: TranslateOutcomeDto): BadgeKey {
  if (outcome.providerKind === "passthrough") return "passthrough";
  switch (outcome.cacheStatus) {
    case "hit":
      return "cached";
    case "refreshed":
      return "refreshed";
    case "miss":
    default:
      return "justTranslated";
  }
}

function matchesTranslateLocale(language: string | undefined) {
  return (
    language === TRANSLATE_LOCALE ||
    language?.startsWith(`${TRANSLATE_LOCALE}-`) ||
    false
  );
}

export function DetailPanel({ groups, selectedName, selectedKind }: Props) {
  const qc = useQueryClient();
  const { i18n } = useTranslation();
  const { t } = useTranslation("common");
  const { t: ta } = useTranslation("artifact");
  const errorMessage = useErrorMessage();
  const [lastOutcome, setLastOutcome] = useState<TranslateOutcomeDto | null>(
    null
  );
  const [bodyLoading, setBodyLoading] = useState(false);
  const [translateError, setTranslateError] = useState<string | null>(null);
  const [compatibilityReviews, setCompatibilityReviews] = useState<
    CompatibilityReviewDto[]
  >([]);
  const [compatibilityError, setCompatibilityError] = useState<string | null>(
    null
  );
  const [flash, setFlash] = useState<"copied" | "cleared" | null>(null);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | null>(null);
  const [draftFlow, setDraftFlow] = useState<DraftFlow>({ kind: "idle" });
  const [draftError, setDraftError] = useState<string | null>(null);
  const [currentSourceHash, setCurrentSourceHash] = useState<string | null>(
    null
  );
  const [summary, setSummary] = useState<SkillSummaryDto | null>(null);
  const [summaryLoading, setSummaryLoading] = useState(false);
  const [summaryError, setSummaryError] = useState<string | null>(null);

  const group = groups.find(
    (g) => g.name === selectedName && g.kind === selectedKind
  );
  const isTranslateLocale =
    matchesTranslateLocale(i18n.resolvedLanguage) ||
    matchesTranslateLocale(i18n.language);

  const invalidate = () => qc.invalidateQueries({ queryKey: ["inventory"] });

  const uninstallMut = useMutation({
    mutationFn: (i: InstallationDto) => uninstall(i),
    onSuccess: invalidate,
  });

  const enableMut = useMutation({
    mutationFn: (i: InstallationDto) => enable(i),
    onSuccess: invalidate,
  });

  const disableMut = useMutation({
    mutationFn: (i: InstallationDto) => disable(i),
    onSuccess: invalidate,
  });

  const previewAdaptCodexMut = useMutation({
    mutationFn: () => {
      const claude = owned.find(
        (si) => si.installation.target.tool === "claude-code"
      );
      if (!claude) throw new Error("No Claude Code installation selected");
      setDraftError(null);
      return previewAdaptSkillForCodex(claude.artifact);
    },
    onSuccess: (preview) =>
      setDraftFlow({ kind: "preview", mode: "adapt", data: preview }),
    onError: (err) => setDraftError(errorMessage(err)),
  });

  const previewForkMut = useMutation({
    mutationFn: ({
      artifact,
      targetTool,
    }: {
      artifact: ArtifactDto;
      targetTool: string;
    }) => {
      setDraftError(null);
      const target: TargetDto = { tool: targetTool, scope: { type: "global" } };
      return previewForkSkill({ artifact, target });
    },
    onSuccess: (preview) =>
      setDraftFlow({ kind: "preview", mode: "fork", data: preview }),
    onError: (err) => setDraftError(errorMessage(err)),
  });

  const confirmDraftMut = useMutation({
    mutationFn: (request: ConfirmDraftInstallRequest) =>
      confirmInstallSkillDraft(request),
    onSuccess: () => {
      setDraftFlow({ kind: "idle" });
      setDraftError(null);
      invalidate();
    },
    onError: (err) => setDraftError(errorMessage(err)),
  });

  const handleDraftConfirm = useCallback(
    (override: { name: string }) => {
      if (draftFlow.kind !== "preview") return;
      const p = draftFlow.data;
      const request: ConfirmDraftInstallRequest = {
        name: override.name,
        description: p.adaptedDescription,
        version: p.adaptedVersion,
        content: p.adaptedContent,
        target: p.target,
        lineage: p.lineage,
      };
      confirmDraftMut.mutate(request);
    },
    [draftFlow, confirmDraftMut]
  );

  const flashOnce = useCallback((kind: "copied" | "cleared") => {
    if (flashTimer.current) clearTimeout(flashTimer.current);
    setFlash(kind);
    flashTimer.current = setTimeout(() => setFlash(null), 1500);
  }, []);

  useEffect(() => () => {
    if (flashTimer.current) clearTimeout(flashTimer.current);
  }, []);

  const runTranslate = useCallback(
    async (force: boolean, signal?: { alive: boolean }) => {
      if (!group?.body) return;
      setBodyLoading(true);
      setTranslateError(null);
      try {
        const outcome = await translateArtifact({
          artifactName: group.name,
          filePath: TRANSLATE_FILE,
          field: TRANSLATE_FIELD,
          sourceText: group.body,
          locale: TRANSLATE_LOCALE,
          forceRefresh: force,
        });
        if (signal && !signal.alive) return;
        setLastOutcome(outcome);
      } catch (e) {
        if (signal && !signal.alive) return;
        setLastOutcome(null);
        setTranslateError(errorMessage(e));
      } finally {
        if (!signal || signal.alive) setBodyLoading(false);
      }
    },
    [group?.body, group?.name, errorMessage]
  );

  useEffect(() => {
    setLastOutcome(null);
    setTranslateError(null);
    if (!group?.body || !isTranslateLocale) return;

    const signal = { alive: true };
    runTranslate(false, signal);
    return () => {
      signal.alive = false;
    };
  }, [group?.body, group?.name, isTranslateLocale, runTranslate]);

  useEffect(() => {
    setCompatibilityReviews([]);
    setCompatibilityError(null);
    if (!group || group.installations.length === 0) return;
    const artifact = group.installations[0].artifact;
    const targets = group.installations.map((si) => si.installation.target);
    let cancelled = false;
    reviewArtifactCompatibility(artifact, targets)
      .then((reviews) => {
        if (!cancelled) setCompatibilityReviews(reviews);
      })
      .catch((e) => {
        if (!cancelled) setCompatibilityError(errorMessage(e));
      });
    return () => {
      cancelled = true;
    };
  }, [group, errorMessage]);

  useEffect(() => {
    setCurrentSourceHash(null);
    if (!group) return;
    const claude = group.installations.find(
      (si) =>
        si.provenance === "owned" &&
        si.installation.target.tool === "claude-code"
    );
    if (!claude) return;
    let cancelled = false;
    sha256Hex(composeSkillMd(claude.artifact)).then((hash) => {
      if (!cancelled) setCurrentSourceHash(hash);
    });
    return () => {
      cancelled = true;
    };
  }, [group]);

  useEffect(() => {
    setSummary(null);
    setSummaryError(null);
    setSummaryLoading(false);
    if (!group || group.kind !== "Skill") return;
    const owned = group.installations.find((si) => si.provenance === "owned");
    const target = owned ?? group.installations[0];
    if (!target) return;
    const artifact = target.artifact;
    const locale = i18n.resolvedLanguage ?? i18n.language ?? "en";
    const signal = { alive: true };

    (async () => {
      try {
        const cached = await getSkillSummary(artifact, locale);
        if (!signal.alive) return;
        if (cached) {
          setSummary(cached);
          return;
        }
        setSummaryLoading(true);
        const fresh = await generateSkillSummary({
          artifact,
          locale,
          forceRefresh: false,
        });
        if (!signal.alive) return;
        setSummary(fresh);
      } catch (e) {
        if (!signal.alive) return;
        setSummary(null);
        setSummaryError(errorMessage(e));
      } finally {
        if (signal.alive) setSummaryLoading(false);
      }
    })();

    return () => {
      signal.alive = false;
    };
  }, [group, i18n.resolvedLanguage, i18n.language, errorMessage]);

  if (!group) {
    return (
      <div className="flex items-center justify-center h-full text-gray-600 text-sm">
        {t("selectPrompt")}
      </div>
    );
  }

  const owned = group.installations.filter((i) => i.provenance === "owned");
  const shared = group.installations.filter((i) =>
    i.provenance.startsWith("shared:")
  );
  const hasClaudeOwned = owned.some(
    (si) => si.installation.target.tool === "claude-code"
  );
  const hasDirectCodexOwned = owned.some(
    (si) => si.installation.target.tool === "codex"
  );
  const adaptedCodexInstallations = findCodexAdaptations(groups, group.name);
  const hasCodexAdaptation = adaptedCodexInstallations.length > 0;

  const mutError =
    uninstallMut.error ??
    enableMut.error ??
    disableMut.error ??
    null;

  const showTranslation = isTranslateLocale && !!group.body;

  async function handleCopy() {
    if (!lastOutcome?.text) return;
    try {
      await navigator.clipboard.writeText(lastOutcome.text);
      flashOnce("copied");
    } catch (e) {
      setTranslateError(errorMessage(e));
    }
  }

  async function handleClear() {
    if (!group) return;
    try {
      await clearTranslationCache({
        artifactName: group.name,
        filePath: TRANSLATE_FILE,
        field: TRANSLATE_FIELD,
        locale: TRANSLATE_LOCALE,
      });
      setLastOutcome(null);
      setTranslateError(null);
      flashOnce("cleared");
    } catch (e) {
      setTranslateError(errorMessage(e));
    }
  }

  const translationBody = translateError
    ? group.body
    : lastOutcome?.text ?? (bodyLoading ? "…" : ta("noTranslationYet"));

  return (
    <div className="p-5 overflow-y-auto h-full">
      <div className="mb-4">
        <h2 className="text-lg font-semibold text-gray-100">{group.name}</h2>
        {group.version && (
          <p className="text-xs text-gray-500">{ta("version", { version: group.version })}</p>
        )}
        {group.description && (
          <p className="mt-1 text-sm text-gray-400">{group.description}</p>
        )}
      </div>

      {group.kind === "Skill" && (
        <Section title={ta("summary.title")}>
          <SkillSummaryBlock
            summary={summary}
            loading={summaryLoading}
            error={summaryError}
          />
        </Section>
      )}

      <Section title={t("source")}>
        {group.installations[0] ? (
          <p className="text-xs text-gray-400 break-all">
            {sourceLabel(group.installations[0].artifact.source)}
          </p>
        ) : (
          <p className="text-xs text-gray-600">{t("unknown")}</p>
        )}
      </Section>

      {(compatibilityReviews.length > 0 || compatibilityError) && (
        <Section title={ta("compatibility")}>
          {compatibilityError ? (
            <p className="text-xs text-red-400">{compatibilityError}</p>
          ) : (
            <CompatibilityNotice reviews={compatibilityReviews} />
          )}
        </Section>
      )}

      {group.body && (
        <Section title={ta("body")}>
          <div className="space-y-2">
            <div>
              <p className="mb-1 text-[11px] uppercase tracking-wider text-gray-500">
                {t("source")}
              </p>
              <pre className="max-h-72 overflow-auto whitespace-pre-wrap rounded bg-gray-900 border border-gray-800 px-3 py-2 text-xs leading-relaxed text-gray-300">
                {group.body}
              </pre>
            </div>
            {showTranslation && (
              <div>
                <div className="mb-1 flex items-center justify-between gap-2">
                  <div className="flex items-center gap-2 text-[11px] uppercase tracking-wider text-gray-500">
                    <span>{bodyLoading ? ta("translating") : "ZH"}</span>
                    {!bodyLoading && lastOutcome && (
                      <>
                        <StatusBadge outcome={lastOutcome} label={ta(`badge.${badgeKey(lastOutcome)}`)} />
                        {lastOutcome.usedFallback && (
                          <span className="normal-case px-1.5 py-0.5 rounded border text-[10px] bg-amber-950 text-amber-300 border-amber-900">
                            {ta("badge.fallback")}
                          </span>
                        )}
                      </>
                    )}
                  </div>
                  <div className="flex items-center gap-1">
                    <TranslateButton
                      onClick={() => runTranslate(false)}
                      disabled={bodyLoading}
                    >
                      {ta("translate")}
                    </TranslateButton>
                    <TranslateButton
                      onClick={() => runTranslate(true)}
                      disabled={bodyLoading}
                    >
                      {ta("retranslate")}
                    </TranslateButton>
                    <TranslateButton
                      onClick={handleCopy}
                      disabled={!lastOutcome?.text || bodyLoading}
                    >
                      {flash === "copied" ? ta("copied") : ta("copy")}
                    </TranslateButton>
                    <TranslateButton
                      onClick={handleClear}
                      disabled={bodyLoading}
                    >
                      {flash === "cleared" ? ta("cleared") : ta("clearCache")}
                    </TranslateButton>
                  </div>
                </div>
                <pre className="whitespace-pre-wrap break-words rounded bg-gray-950 border border-gray-800 px-3 py-2 text-xs leading-relaxed text-gray-200">
                  {translationBody}
                </pre>
                {translateError && (
                  <p className="mt-1 text-xs text-red-400 break-all">
                    {translateError}
                  </p>
                )}
                {!translateError &&
                  lastOutcome &&
                  !lastOutcome.validation.ok && (
                    <ValidationWarnings
                      warnings={lastOutcome.validation.warnings}
                      onRetranslate={() => runTranslate(true)}
                      retranslating={bodyLoading}
                    />
                  )}
              </div>
            )}
          </div>
        </Section>
      )}

      {group.capabilities.length > 0 && (
        <Section title={ta("capabilities")}>
          <ul className="space-y-1.5">
            {group.capabilities.map((cap) => (
              <li key={cap.name} className="text-xs">
                <span className="font-mono text-gray-200">{cap.name}</span>
                {cap.description && (
                  <span className="text-gray-500"> — {cap.description}</span>
                )}
              </li>
            ))}
          </ul>
        </Section>
      )}

      {owned.length > 0 && (
        <Section title={t("installed")}>
          {group.kind === "Skill" && hasClaudeOwned && hasCodexAdaptation && (
            <div className={`mb-3 rounded border px-3 py-2 text-xs ${
              adaptationIsCurrent(adaptedCodexInstallations, currentSourceHash)
                ? "border-emerald-800 bg-emerald-950/35 text-emerald-100"
                : "border-yellow-800 bg-yellow-950/40 text-yellow-100"
            }`}>
              <p className="font-medium">
                {adaptationIsCurrent(adaptedCodexInstallations, currentSourceHash)
                  ? ta("adaptedCodexCurrent")
                  : ta("adaptedCodexOld")}
              </p>
              <ul className={`mt-1 space-y-0.5 ${
                adaptationIsCurrent(adaptedCodexInstallations, currentSourceHash)
                  ? "text-emerald-200/90"
                  : "text-yellow-200/90"
              }`}>
                {adaptedCodexInstallations.map((si) => (
                  <li key={si.installation.id}>
                    {ta("adaptedCodexName", {
                      name: si.artifact.name,
                    })}
                  </li>
                ))}
              </ul>
              {!adaptationIsCurrent(adaptedCodexInstallations, currentSourceHash) && (
                <button
                  onClick={() => previewAdaptCodexMut.mutate()}
                  disabled={previewAdaptCodexMut.isPending}
                  className="mt-2 rounded border border-yellow-800 px-2 py-1 text-[11px] text-yellow-100 hover:border-yellow-700 disabled:opacity-40"
                >
                  {previewAdaptCodexMut.isPending
                    ? ta("adaptingCodex")
                    : ta("reviewNewCodexAdaptation")}
                </button>
              )}
            </div>
          )}
          {group.kind === "Skill" &&
            hasClaudeOwned &&
            !hasDirectCodexOwned &&
            !hasCodexAdaptation && (
              <button
                onClick={() => previewAdaptCodexMut.mutate()}
                disabled={previewAdaptCodexMut.isPending}
                className="mb-3 mr-2 rounded border border-yellow-800 bg-yellow-950/40 px-3 py-1.5 text-xs text-yellow-100 hover:border-yellow-700 disabled:opacity-40"
              >
                {previewAdaptCodexMut.isPending
                  ? ta("adaptingCodex")
                  : ta("adaptToCodex")}
              </button>
            )}
          {group.kind === "Skill" && owned[0] && (
            <button
              onClick={() =>
                setDraftFlow({ kind: "forkPicker", artifact: owned[0].artifact })
              }
              disabled={previewForkMut.isPending}
              className="mb-3 rounded border border-gray-700 bg-gray-900 px-3 py-1.5 text-xs text-gray-200 hover:border-gray-500 disabled:opacity-40"
            >
              {ta("forkButton")}
            </button>
          )}
          <ul className="space-y-3">
            {owned.map((si) => (
              <InstallationRow
                key={si.installation.id}
                si={si}
                onUninstall={() => uninstallMut.mutate(si.installation)}
                onEnable={() => enableMut.mutate(si.installation)}
                onDisable={() => disableMut.mutate(si.installation)}
                onEdit={
                  si.artifact.lineage?.sourceKind === "fork"
                    ? () =>
                        setDraftFlow({
                          kind: "editor",
                          initialContent: composeSkillMd(si.artifact),
                          target: si.installation.target,
                          lineage: si.artifact.lineage!,
                        })
                    : undefined
                }
                busy={
                  uninstallMut.isPending ||
                  enableMut.isPending ||
                  disableMut.isPending ||
                  previewAdaptCodexMut.isPending ||
                  previewForkMut.isPending ||
                  confirmDraftMut.isPending
                }
              />
            ))}
          </ul>
        </Section>
      )}

      {shared.length > 0 && (
        <Section title={t("alsoVisibleTo")}>
          <ul className="space-y-1">
            {shared.map((si) => (
              <li key={si.installation.id} className="text-xs text-gray-500">
                {si.provenance.replace("shared:", "")} —{" "}
                <span className="text-gray-600 break-all">
                  {si.installation.onDiskPath}
                </span>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {owned.length === 0 && shared.length === 0 && (
        <p className="text-sm text-gray-600">{t("notInstalled")}</p>
      )}

      {mutError && (
        <p className="mt-2 text-xs text-red-400">{errorMessage(mutError)}</p>
      )}

      {draftFlow.kind === "forkPicker" && (
        <ForkTargetPicker
          defaultTool={
            group.installations[0]?.installation.target.tool ?? "claude-code"
          }
          busy={previewForkMut.isPending}
          onPick={(tool) =>
            previewForkMut.mutate({
              artifact: draftFlow.artifact,
              targetTool: tool,
            })
          }
          onCancel={() => setDraftFlow({ kind: "idle" })}
        />
      )}

      {draftFlow.kind === "editor" && (
        <CustomSkillEditor
          initialContent={draftFlow.initialContent}
          target={draftFlow.target}
          lineage={draftFlow.lineage}
          onReviewed={(preview) =>
            setDraftFlow({ kind: "preview", mode: "edit", data: preview })
          }
          onCancel={() => setDraftFlow({ kind: "idle" })}
        />
      )}

      {draftFlow.kind === "preview" && (
        <SkillPreviewModal
          mode={draftFlow.mode}
          preview={draftFlow.data}
          busy={confirmDraftMut.isPending}
          onConfirm={handleDraftConfirm}
          onCancel={() => {
            setDraftFlow({ kind: "idle" });
            setDraftError(null);
          }}
          errorMessage={draftError ?? undefined}
        />
      )}
    </div>
  );
}

function findCodexAdaptations(
  groups: ArtifactGroupDto[],
  parentName: string
): ScannedInstallationDto[] {
  return groups.flatMap((group) =>
    group.installations.filter(
      (si) =>
        si.provenance === "owned" &&
        si.installation.target.tool === "codex" &&
        si.artifact.lineage?.sourceKind === "adaptation" &&
        si.artifact.lineage.parentName === parentName
    )
  );
}

function adaptationIsCurrent(
  installations: ScannedInstallationDto[],
  currentSourceHash: string | null
): boolean {
  return (
    currentSourceHash !== null &&
    installations.some(
      (si) => si.artifact.lineage?.sourceHash === currentSourceHash
    )
  );
}

function StatusBadge({
  outcome,
  label,
}: {
  outcome: TranslateOutcomeDto;
  label: string;
}) {
  const tone =
    outcome.providerKind === "passthrough"
      ? "bg-gray-800 text-gray-400 border-gray-700"
      : outcome.cacheStatus === "hit"
        ? "bg-gray-800 text-gray-300 border-gray-700"
        : outcome.cacheStatus === "refreshed"
          ? "bg-sky-950 text-sky-300 border-sky-900"
          : "bg-emerald-950 text-emerald-300 border-emerald-900";
  return (
    <span className={`normal-case px-1.5 py-0.5 rounded border text-[10px] ${tone}`}>
      {label}
    </span>
  );
}

function ValidationWarnings({
  warnings,
  onRetranslate,
  retranslating,
}: {
  warnings: MarkdownWarningDto[];
  onRetranslate: () => void;
  retranslating: boolean;
}) {
  const { t: ta } = useTranslation("artifact");
  return (
    <div className="mt-1 rounded border border-yellow-900 bg-yellow-950/40 px-3 py-2 text-xs text-yellow-200">
      <div className="flex items-center justify-between gap-2">
        <p className="font-medium">{ta("validation.header")}</p>
        <button
          onClick={onRetranslate}
          disabled={retranslating}
          className="text-[11px] text-yellow-200 hover:text-yellow-100 border border-yellow-900 hover:border-yellow-700 rounded px-1.5 py-0.5 disabled:opacity-40"
        >
          {ta("retranslate")}
        </button>
      </div>
      <ul className="mt-1 list-disc list-inside space-y-0.5 text-yellow-300/90">
        {warnings.map((w, i) => (
          <li key={i}>{formatWarning(w, ta)}</li>
        ))}
      </ul>
    </div>
  );
}

function formatWarning(
  w: MarkdownWarningDto,
  ta: (key: string, opts?: Record<string, unknown>) => string
): string {
  switch (w.kind) {
    case "fencedCodeBlockCount":
      return ta("validation.fencedCodeBlockCount", {
        source: w.source,
        translated: w.translated,
      });
    case "linkCount":
      return ta("validation.linkCount", {
        source: w.source,
        translated: w.translated,
      });
    case "headingCount":
      return ta("validation.headingCount", {
        source: w.source,
        translated: w.translated,
      });
    case "listItemCount":
      return ta("validation.listItemCount", {
        source: w.source,
        translated: w.translated,
      });
    case "codeBlockContentChanged":
      return ta("validation.codeBlockContentChanged", { index: w.index });
    case "frontmatterChanged":
      return ta("validation.frontmatterChanged");
  }
}

function TranslateButton({
  onClick,
  disabled,
  children,
}: {
  onClick: () => void;
  disabled?: boolean;
  children: React.ReactNode;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className="text-[11px] text-gray-400 hover:text-gray-200 border border-gray-800 hover:border-gray-600 rounded px-1.5 py-0.5 disabled:opacity-40"
    >
      {children}
    </button>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-5">
      <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-500 mb-2">
        {title}
      </h3>
      {children}
    </div>
  );
}

function SkillSummaryBlock({
  summary,
  loading,
  error,
}: {
  summary: SkillSummaryDto | null;
  loading: boolean;
  error: string | null;
}) {
  const { t: ta } = useTranslation("artifact");

  if (loading && !summary) {
    return (
      <p className="text-xs text-gray-500">{ta("summary.loading")}</p>
    );
  }
  if (error) {
    return <p className="text-xs text-gray-500">{error}</p>;
  }
  if (!summary) {
    return <p className="text-xs text-gray-600">{ta("summary.empty")}</p>;
  }

  return (
    <div className="space-y-3 text-xs text-gray-300">
      {summary.commands.length > 0 && (
        <div>
          <p className="mb-1 text-[11px] uppercase tracking-wider text-gray-500">
            {ta("summary.commands")}
          </p>
          <ul className="space-y-0.5 list-disc list-inside marker:text-gray-600">
            {summary.commands.map((cmd, i) => (
              <li key={i} className="font-mono text-gray-200 break-all">
                {cmd}
              </li>
            ))}
          </ul>
        </div>
      )}
      {summary.capabilities && (
        <div>
          <p className="mb-1 text-[11px] uppercase tracking-wider text-gray-500">
            {ta("summary.capabilities")}
          </p>
          <p className="text-gray-200 leading-relaxed whitespace-pre-wrap">
            {summary.capabilities}
          </p>
        </div>
      )}
      {summary.useCases.length > 0 && (
        <div>
          <p className="mb-1 text-[11px] uppercase tracking-wider text-gray-500">
            {ta("summary.useCases")}
          </p>
          <ul className="space-y-0.5 list-disc list-inside marker:text-gray-600">
            {summary.useCases.map((uc, i) => (
              <li key={i} className="text-gray-200">
                {uc}
              </li>
            ))}
          </ul>
        </div>
      )}
      {summary.examples.length > 0 && (
        <div>
          <p className="mb-1 text-[11px] uppercase tracking-wider text-gray-500">
            {ta("summary.examples")}
          </p>
          <ul className="space-y-0.5 list-disc list-inside marker:text-gray-600">
            {summary.examples.map((ex, i) => (
              <li key={i} className="font-mono text-gray-200 break-all">
                {ex}
              </li>
            ))}
          </ul>
        </div>
      )}
    </div>
  );
}

function composeSkillMd(artifact: ArtifactDto): string {
  const lines = ["---", `name: ${artifact.name}`];
  if (artifact.description) lines.push(`description: ${artifact.description}`);
  if (artifact.version) lines.push(`version: ${artifact.version}`);
  lines.push("---", "");
  if (artifact.body) lines.push(artifact.body);
  return lines.join("\n") + "\n";
}

async function sha256Hex(input: string): Promise<string> {
  const bytes = new TextEncoder().encode(input);
  const digest = await crypto.subtle.digest("SHA-256", bytes);
  return Array.from(new Uint8Array(digest))
    .map((byte) => byte.toString(16).padStart(2, "0"))
    .join("");
}

function InstallationRow({
  si,
  onUninstall,
  onEnable,
  onDisable,
  onEdit,
  busy,
}: {
  si: ScannedInstallationDto;
  onUninstall: () => void;
  onEnable: () => void;
  onDisable: () => void;
  onEdit?: () => void;
  busy: boolean;
}) {
  const { t } = useTranslation("common");
  const { t: ta } = useTranslation("artifact");
  const { installation } = si;
  const isDisabled = installation.status === "disabled";
  const isFork = si.artifact.lineage?.sourceKind === "fork";

  return (
    <li className="rounded bg-gray-800 px-3 py-2 text-xs">
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium text-gray-200">
          {targetLabel(installation.target)}
        </span>
        <span
          className={`px-1.5 py-0.5 rounded text-xs ${
            isDisabled
              ? "bg-yellow-900 text-yellow-300"
              : installation.status.startsWith("broken")
                ? "bg-red-900 text-red-300"
                : "bg-emerald-900 text-emerald-300"
          }`}
        >
          {installation.status}
        </span>
      </div>
      <p className="mt-1 text-gray-500 break-all">{installation.onDiskPath}</p>
      <div className="mt-2 flex gap-2 flex-wrap">
        {isDisabled ? (
          <ActionButton onClick={onEnable} disabled={busy}>
            {t("enable")}
          </ActionButton>
        ) : (
          <ActionButton onClick={onDisable} disabled={busy}>
            {t("disable")}
          </ActionButton>
        )}
        {isFork && onEdit && (
          <ActionButton onClick={onEdit} disabled={busy}>
            {ta("editButton")}
          </ActionButton>
        )}
        <ActionButton
          onClick={onUninstall}
          disabled={busy}
          className="text-red-400 hover:text-red-300"
        >
          {t("uninstall")}
        </ActionButton>
      </div>
    </li>
  );
}

function ActionButton({
  onClick,
  disabled,
  children,
  className = "text-gray-400 hover:text-gray-200",
}: {
  onClick: () => void;
  disabled: boolean;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`text-xs disabled:opacity-40 ${className}`}
    >
      {children}
    </button>
  );
}

function ForkTargetPicker({
  defaultTool,
  busy,
  onPick,
  onCancel,
}: {
  defaultTool: string;
  busy: boolean;
  onPick: (tool: string) => void;
  onCancel: () => void;
}) {
  const { t: ta } = useTranslation("artifact");
  const initial = FORK_TARGET_TOOLS.includes(defaultTool as (typeof FORK_TARGET_TOOLS)[number])
    ? defaultTool
    : FORK_TARGET_TOOLS[0];
  const [tool, setTool] = useState<string>(initial);
  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-900 border border-gray-700 rounded-lg w-full max-w-md shadow-xl flex flex-col">
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-700">
          <h2 className="text-base font-semibold text-gray-100">
            {ta("fork.title")}
          </h2>
          <button
            onClick={onCancel}
            className="text-gray-500 hover:text-gray-300 text-lg leading-none"
          >
            ✕
          </button>
        </div>
        <div className="px-5 py-4 space-y-3">
          <p className="text-xs text-gray-400">{ta("fork.pickTarget")}</p>
          <div className="space-y-1.5">
            {FORK_TARGET_TOOLS.map((opt) => (
              <label
                key={opt}
                className="flex items-center gap-2 text-sm text-gray-200"
              >
                <input
                  type="radio"
                  name="fork-target"
                  value={opt}
                  checked={tool === opt}
                  onChange={() => setTool(opt)}
                />
                <span>{opt}</span>
              </label>
            ))}
          </div>
        </div>
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-gray-700">
          <button
            onClick={onCancel}
            disabled={busy}
            className="text-xs text-gray-300 hover:text-gray-100 border border-gray-700 hover:border-gray-500 rounded px-3 py-1.5 disabled:opacity-40"
          >
            ✕
          </button>
          <button
            onClick={() => onPick(tool)}
            disabled={busy}
            className="text-xs text-emerald-200 hover:text-emerald-100 border border-emerald-800 hover:border-emerald-600 bg-emerald-950/40 rounded px-3 py-1.5 disabled:opacity-40"
          >
            {ta("fork.confirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
