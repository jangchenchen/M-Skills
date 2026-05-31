import { useEffect, useMemo, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  checkPathExists,
  classifySkillRequest,
  getTranslateConfig,
  install,
  previewImport,
  reviewImport,
  scan,
} from "../api";
import { useErrorMessage } from "../useErrorMessage";
import type {
  AdapterStatusDto,
  ErrorDto,
  ImportCandidateDto,
  ImportPreviewDto,
  InstallOutcomeDto,
  ReviewOutcomeDto,
  SkillIntentOutcomeDto,
  TargetDto,
} from "../types";
import { targetLabel } from "../types";
import { classifySmartAddInput, type SmartAddKind } from "../smartAdd";
import { CompatibilityNotice } from "./CompatibilityNotice";
import { isAtLeast, RiskBadge, WarningsSection } from "./AuditNotice";

interface Props {
  onClose: () => void;
  onInstalled: (name: string, kind: ImportCandidateDto["artifact"]["kind"]) => void;
  onOpenSettings: () => void;
}

type Step = "input" | "preview" | "done";
type ReviewState = "loading" | "ready" | "skipped" | "failed";
type IntentState = "idle" | "loading" | "ready" | "failed";

function targetKey(t: TargetDto): string {
  return `${t.tool}:${t.scope.type}`;
}

function intersectTargets(
  chipSelection: Set<string>,
  targets: TargetDto[]
): TargetDto[] {
  return targets.filter((t) => chipSelection.has(t.tool));
}

export function ImportWizard({ onClose, onInstalled, onOpenSettings }: Props) {
  const [step, setStep] = useState<Step>("input");
  const [pathOrUrl, setPathOrUrl] = useState("");
  const [preview, setPreview] = useState<ImportPreviewDto | null>(null);
  const [selectedCandidate, setSelectedCandidate] =
    useState<ImportCandidateDto | null>(null);
  const [selectedTargets, setSelectedTargets] = useState<TargetDto[]>([]);
  const [chipSelection, setChipSelection] = useState<Set<string>>(new Set());
  const [riskAck, setRiskAck] = useState(false);
  const [conflictAck, setConflictAck] = useState(false);
  const [outcomes, setOutcomes] = useState<InstallOutcomeDto[]>([]);
  const [reviewState, setReviewState] = useState<ReviewState>("loading");
  const [reviewOutcome, setReviewOutcome] = useState<ReviewOutcomeDto | null>(
    null
  );
  const [reviewError, setReviewError] = useState<string | null>(null);
  const [intentState, setIntentState] = useState<IntentState>("idle");
  const [intentOutcome, setIntentOutcome] =
    useState<SkillIntentOutcomeDto | null>(null);
  const [intentError, setIntentError] = useState<string | null>(null);
  const [pathExists, setPathExists] = useState(false);
  const qc = useQueryClient();
  const { t, i18n } = useTranslation("wizard");
  const errorMessage = useErrorMessage();

  const { data: inventory } = useQuery({
    queryKey: ["inventory"],
    queryFn: () => scan(),
  });
  const { data: translateConfig } = useQuery({
    queryKey: ["translate-config"],
    queryFn: getTranslateConfig,
    staleTime: 30_000,
  });
  const askAiConfigured = !!translateConfig?.apiKeyPresent;
  const availableAdapters = useMemo<AdapterStatusDto[]>(
    () =>
      (inventory?.adapters ?? []).filter(
        (a) =>
          a.presence.type === "available" && a.adapterId !== "shared-global"
      ),
    [inventory]
  );
  const detectedInputKind = useMemo<SmartAddKind>(
    () => classifySmartAddInput(pathOrUrl),
    [pathOrUrl]
  );
  const inputKind = pathExists && detectedInputKind === "askAi" ? "local" : detectedInputKind;

  useEffect(() => {
    const trimmed = pathOrUrl.trim();
    setPathExists(false);
    if (!trimmed || detectedInputKind === "url") return;
    let cancelled = false;
    const timer = window.setTimeout(() => {
      checkPathExists(trimmed)
        .then((exists) => {
          if (!cancelled) setPathExists(exists);
        })
        .catch(() => {
          if (!cancelled) setPathExists(false);
        });
    }, 180);
    return () => {
      cancelled = true;
      window.clearTimeout(timer);
    };
  }, [pathOrUrl, detectedInputKind]);

  const previewMut = useMutation({
    mutationFn: () => previewImport(pathOrUrl),
    onSuccess: (data) => {
      setPreview(data);
      setRiskAck(false);
      setConflictAck(false);
      setOutcomes([]);
      if (data.candidates.length > 0) {
        const first = data.candidates[0];
        setSelectedCandidate(first);
        setSelectedTargets(
          intersectTargets(chipSelection, first.compatibleTargets)
        );
      } else {
        setSelectedCandidate(null);
        setSelectedTargets([]);
      }
      setStep("preview");
    },
  });

  const classifyMut = useMutation({
    mutationFn: () => {
      const locale = i18n.resolvedLanguage ?? i18n.language ?? null;
      return classifySkillRequest(pathOrUrl, locale);
    },
    onMutate: () => {
      setIntentState("loading");
      setIntentOutcome(null);
      setIntentError(null);
    },
    onSuccess: (data) => {
      setIntentOutcome(data);
      setIntentState("ready");
    },
    onError: (err) => {
      setIntentError(errorMessage(err));
      setIntentState("failed");
    },
  });

  const installMut = useMutation({
    mutationFn: async () => {
      if (!selectedCandidate || selectedTargets.length === 0) {
        throw new Error("incomplete");
      }
      return install(selectedCandidate.index, selectedTargets);
    },
    onSuccess: (results) => {
      setOutcomes(results);
      qc.invalidateQueries({ queryKey: ["inventory"] });
      if (selectedCandidate && results.some((result) => result.ok)) {
        onInstalled(selectedCandidate.artifact.name, selectedCandidate.artifact.kind);
      }
      setStep("done");
    },
  });

  // Trigger LLM compatibility review automatically when entering preview step
  // or when the user picks a different candidate. Failures never block install.
  const candidateIndex = selectedCandidate?.index ?? null;
  useEffect(() => {
    if (step !== "preview" || candidateIndex === null) return;
    let cancelled = false;
    setReviewState("loading");
    setReviewOutcome(null);
    setReviewError(null);
    setConflictAck(false);
    const locale = i18n.resolvedLanguage ?? i18n.language ?? null;
    reviewImport(candidateIndex, locale)
      .then((outcome) => {
        if (cancelled) return;
        setReviewOutcome(outcome);
        setReviewState("ready");
      })
      .catch((err: unknown) => {
        if (cancelled) return;
        const code = (err as ErrorDto | undefined)?.code;
        if (code === "reviewNotConfigured") {
          setReviewState("skipped");
        } else {
          setReviewState("failed");
          setReviewError(errorMessage(err));
        }
      });
    return () => {
      cancelled = true;
    };
  }, [step, candidateIndex, i18n.resolvedLanguage, i18n.language, errorMessage]);

  const onCandidateChange = (c: ImportCandidateDto) => {
    setSelectedCandidate(c);
    setSelectedTargets(intersectTargets(chipSelection, c.compatibleTargets));
    setRiskAck(false);
    setConflictAck(false);
  };

  const onChipToggle = (id: string) => {
    setChipSelection((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const onInputChange = (next: string) => {
    setPathOrUrl(next);
    setIntentState("idle");
    setIntentOutcome(null);
    setIntentError(null);
  };

  const onInputSubmit = () => {
    if (inputKind === "askAi") {
      classifyMut.mutate();
      return;
    }
    previewMut.mutate();
  };

  const onTargetToggle = (target: TargetDto, checked: boolean) => {
    setSelectedTargets((prev) => {
      const k = targetKey(target);
      const filtered = prev.filter((t) => targetKey(t) !== k);
      return checked ? [...filtered, target] : filtered;
    });
  };

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-900 border border-gray-700 rounded-lg w-full max-w-xl shadow-xl max-h-[90vh] flex flex-col">
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-700">
          <h2 className="text-base font-semibold text-gray-100">
            {t("title")}
          </h2>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-300 text-lg leading-none"
          >
            ✕
          </button>
        </div>

        <div className="px-5 py-4 overflow-y-auto">
          {step === "input" && (
            <InputStep
              value={pathOrUrl}
              onChange={onInputChange}
              onSubmit={onInputSubmit}
              loading={previewMut.isPending || classifyMut.isPending}
              error={previewMut.error ? errorMessage(previewMut.error) : undefined}
              inputKind={inputKind}
              chipSelection={chipSelection}
              onChipToggle={onChipToggle}
              availableAdapters={availableAdapters}
              askAiConfigured={askAiConfigured}
              intentState={intentState}
              intentOutcome={intentOutcome}
              intentError={intentError}
              onOpenSettings={onOpenSettings}
            />
          )}

          {step === "preview" && preview && (
            <PreviewStep
              preview={preview}
              selectedCandidate={selectedCandidate}
              selectedTargets={selectedTargets}
              riskAck={riskAck}
              onRiskAckChange={setRiskAck}
              conflictAck={conflictAck}
              onConflictAckChange={setConflictAck}
              reviewState={reviewState}
              reviewOutcome={reviewOutcome}
              reviewError={reviewError}
              selectedTargetTools={chipSelection}
              onCandidateChange={onCandidateChange}
              onTargetToggle={onTargetToggle}
              onInstall={() => installMut.mutate()}
              onBack={() => setStep("input")}
              loading={installMut.isPending}
              error={installMut.error ? errorMessage(installMut.error) : undefined}
            />
          )}

          {step === "done" && (
            <DoneStep outcomes={outcomes} onClose={onClose} errorMessage={errorMessage} />
          )}
        </div>
      </div>
    </div>
  );
}

function InputStep({
  value,
  onChange,
  onSubmit,
  loading,
  error,
  inputKind,
  chipSelection,
  onChipToggle,
  availableAdapters,
  askAiConfigured,
  intentState,
  intentOutcome,
  intentError,
  onOpenSettings,
}: {
  value: string;
  onChange: (v: string) => void;
  onSubmit: () => void;
  loading: boolean;
  error?: string;
  inputKind: SmartAddKind;
  chipSelection: Set<string>;
  onChipToggle: (id: string) => void;
  availableAdapters: AdapterStatusDto[];
  askAiConfigured: boolean;
  intentState: IntentState;
  intentOutcome: SkillIntentOutcomeDto | null;
  intentError: string | null;
  onOpenSettings: () => void;
}) {
  const { t } = useTranslation("wizard");
  const hasTargets = availableAdapters.length > 0;
  const needsTargetSelection = hasTargets && chipSelection.size === 0;
  const isAskAi = inputKind === "askAi";
  const isEmpty = inputKind === "empty";
  const disabled =
    loading ||
    !value.trim() ||
    needsTargetSelection ||
    !hasTargets ||
    (isAskAi && !askAiConfigured);
  const actionLabel = isAskAi ? t("smartAdd.askAi.classify") : t("preview");

  return (
    <div className="space-y-4">
      <div>
        <label className="text-xs font-medium text-gray-400 block mb-1.5">
          {t("smartAdd.chipsLabel")}
        </label>
        {hasTargets ? (
          <div className="flex flex-wrap gap-1.5">
            {availableAdapters.map((adapter) => {
              const selected = chipSelection.has(adapter.adapterId);
              return (
                <button
                  key={adapter.adapterId}
                  type="button"
                  onClick={() => onChipToggle(adapter.adapterId)}
                  className={
                    selected
                      ? "px-2.5 py-1 rounded-full border text-xs font-medium bg-indigo-600 border-indigo-500 text-white"
                      : "px-2.5 py-1 rounded-full border text-xs font-medium bg-gray-900 border-gray-700 text-gray-300 hover:border-gray-500"
                  }
                >
                  {adapter.adapterId}
                </button>
              );
            })}
          </div>
        ) : (
          <p className="text-xs text-amber-400">
            {t("smartAdd.noToolsDetected")}
          </p>
        )}
      </div>

      <div>
        <textarea
          rows={4}
          className="w-full rounded bg-gray-800 border border-gray-600 px-3 py-2 text-sm text-gray-100 placeholder-gray-600 focus:outline-none focus:border-indigo-500 resize-none"
          placeholder={t("smartAdd.placeholder")}
          value={value}
          onChange={(e) => onChange(e.target.value)}
          onKeyDown={(e) => {
            if (e.key === "Enter" && (e.metaKey || e.ctrlKey) && !disabled) {
              e.preventDefault();
              onSubmit();
            }
          }}
        />
        <div className="mt-1 flex items-center justify-between gap-2 text-[11px] text-gray-500">
          {!isEmpty ? (
            <span
              className={
                isAskAi
                  ? "inline-flex items-center px-1.5 py-0.5 rounded border text-[10px] uppercase tracking-wide bg-amber-950/40 border-amber-900/60 text-amber-300"
                  : "inline-flex items-center px-1.5 py-0.5 rounded border text-[10px] uppercase tracking-wide bg-gray-800 border-gray-700 text-gray-300"
              }
            >
              {t(`smartAdd.kind.${inputKind}`)}
            </span>
          ) : (
            <span />
          )}
          <span>{t("smartAdd.submitHint")}</span>
        </div>
      </div>

      {isAskAi && !askAiConfigured && (
        <div className="rounded border border-amber-900/60 bg-amber-950/30 px-3 py-2.5 space-y-2">
          <p className="text-xs text-amber-200 leading-relaxed">
            {t("smartAdd.askAi.unconfigured")}
          </p>
          <button
            type="button"
            onClick={onOpenSettings}
            className="text-xs bg-amber-600 hover:bg-amber-500 text-white px-3 py-1 rounded"
          >
            {t("smartAdd.askAi.openSettings")}
          </button>
        </div>
      )}
      {isAskAi && askAiConfigured && (
        <div className="rounded border border-gray-700 bg-gray-900/60 px-3 py-2 space-y-2">
          <p className="text-xs text-gray-400">{t("smartAdd.askAi.ready")}</p>
          {intentState === "loading" && (
            <p className="text-xs text-gray-500">
              {t("smartAdd.askAi.classifying")}
            </p>
          )}
          {intentState === "failed" && intentError && (
            <p className="text-xs text-red-400">{intentError}</p>
          )}
          {intentState === "ready" && intentOutcome && (
            <IntentResult outcome={intentOutcome} />
          )}
        </div>
      )}

      {needsTargetSelection && !isEmpty && (
        <p className="text-xs text-amber-400">
          {t("smartAdd.submitGate.needTarget")}
        </p>
      )}

      {error && <p className="text-xs text-red-400">{error}</p>}

      <div className="flex justify-end">
        <button
          onClick={onSubmit}
          disabled={disabled}
          className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 text-white rounded"
        >
          {loading ? t("loading") : actionLabel}
        </button>
      </div>
    </div>
  );
}

function IntentResult({ outcome }: { outcome: SkillIntentOutcomeDto }) {
  const { t } = useTranslation("wizard");

  if (!outcome.isInstallRequest) {
    return (
      <div className="rounded border border-amber-900/50 bg-amber-950/20 px-3 py-2">
        <p className="text-xs font-medium text-amber-300">
          {t("smartAdd.askAi.notInstall")}
        </p>
        {outcome.reason && (
          <p className="text-xs text-amber-200 mt-1">{outcome.reason}</p>
        )}
      </div>
    );
  }

  return (
    <div className="rounded border border-indigo-900/60 bg-indigo-950/20 px-3 py-2">
      <p className="text-xs font-medium text-indigo-200">
        {t("smartAdd.askAi.searchQuery")}
      </p>
      <p className="text-sm text-gray-100 mt-1 font-mono">
        {outcome.searchQuery}
      </p>
      {outcome.reason && (
        <p className="text-xs text-gray-400 mt-1">{outcome.reason}</p>
      )}
      <p className="text-[10px] text-gray-600 font-mono mt-2">
        {outcome.providerKind} / {outcome.model}
      </p>
    </div>
  );
}

function PreviewStep({
  preview,
  selectedCandidate,
  selectedTargets,
  riskAck,
  onRiskAckChange,
  conflictAck,
  onConflictAckChange,
  reviewState,
  reviewOutcome,
  reviewError,
  selectedTargetTools,
  onCandidateChange,
  onTargetToggle,
  onInstall,
  onBack,
  loading,
  error,
}: {
  preview: ImportPreviewDto;
  selectedCandidate: ImportCandidateDto | null;
  selectedTargets: TargetDto[];
  riskAck: boolean;
  onRiskAckChange: (v: boolean) => void;
  conflictAck: boolean;
  onConflictAckChange: (v: boolean) => void;
  reviewState: ReviewState;
  reviewOutcome: ReviewOutcomeDto | null;
  reviewError: string | null;
  selectedTargetTools: Set<string>;
  onCandidateChange: (c: ImportCandidateDto) => void;
  onTargetToggle: (t: TargetDto, checked: boolean) => void;
  onInstall: () => void;
  onBack: () => void;
  loading: boolean;
  error?: string;
}) {
  const { t } = useTranslation("wizard");
  const { t: tc } = useTranslation("common");
  const { t: ta } = useTranslation("artifact");
  const { audit } = preview;

  const selectedKeys = useMemo(
    () => new Set(selectedTargets.map(targetKey)),
    [selectedTargets]
  );
  const visibleTargets = useMemo(
    () =>
      selectedCandidate
        ? selectedCandidate.compatibleTargets.filter((target) =>
            selectedTargetTools.has(target.tool)
          )
        : [],
    [selectedCandidate, selectedTargetTools]
  );

  if (preview.candidates.length === 0) {
    return (
      <div className="py-6 text-center text-sm text-gray-400">
        {t("noArtifact")}
        <div className="mt-4">
          <button onClick={onBack} className="text-xs text-gray-500 hover:text-gray-300">
            {tc("back")}
          </button>
        </div>
      </div>
    );
  }

  const requireRiskAck = isAtLeast(audit.riskLevel, "medium");
  const requireConflictAck =
    reviewState === "ready" && reviewOutcome?.rating === "conflict";
  const noCompatibleTargets =
    selectedCandidate !== null && selectedCandidate.compatibleTargets.length === 0;
  const installDisabled =
    loading ||
    !selectedCandidate ||
    selectedTargets.length === 0 ||
    (requireRiskAck && !riskAck) ||
    (requireConflictAck && !conflictAck);

  return (
    <div className="space-y-4">
      <RiskBadge level={audit.riskLevel} />

      <ReviewSection
        state={reviewState}
        outcome={reviewOutcome}
        error={reviewError}
      />

      {preview.commitSha && (
        <p className="text-xs text-gray-500">
          {t("commit", { sha: preview.commitSha.slice(0, 12) })}
        </p>
      )}

      {selectedCandidate && (
        <CompatibilityNotice reviews={selectedCandidate.compatibilityReviews} />
      )}

      <WarningsSection warnings={audit.warnings} />

      <div>
        <label className="text-xs text-gray-400 block mb-1">{t("artifact")}</label>
        <select
          className="w-full bg-gray-800 border border-gray-600 rounded px-2 py-1.5 text-sm text-gray-100"
          value={selectedCandidate?.index ?? 0}
          onChange={(e) => {
            const c = preview.candidates[Number(e.target.value)];
            if (c) onCandidateChange(c);
          }}
        >
          {preview.candidates.map((c) => (
            <option key={c.index} value={c.index}>
              {c.artifact.name} ({ta(`kind.${c.artifact.kind}`)})
            </option>
          ))}
        </select>
      </div>

      {selectedCandidate?.artifact.body && (
        <details className="text-xs">
          <summary className="text-gray-400 cursor-pointer hover:text-gray-200">
            {t("skillPreview")}
          </summary>
          <pre className="mt-2 max-h-48 overflow-auto whitespace-pre-wrap rounded bg-gray-950 border border-gray-800 px-3 py-2 text-xs leading-relaxed text-gray-300">
            {selectedCandidate.artifact.body}
          </pre>
        </details>
      )}

      {selectedCandidate && (
        <div>
          <label className="text-xs text-gray-400 block mb-2">
            {t("installTargets")}
          </label>
          {noCompatibleTargets ? (
            <p className="text-xs text-amber-400">{t("noCompatibleInstalled")}</p>
          ) : (
            <>
              {selectedTargets.length === 0 && (
                <p className="text-xs text-amber-400 mb-2">
                  {t("smartAdd.noIntersection")}
                </p>
              )}
              <TargetPlanTable
                visibleTargets={visibleTargets}
                selectedKeys={selectedKeys}
                candidate={selectedCandidate}
                reviewState={reviewState}
                reviewOutcome={reviewOutcome}
                onTargetToggle={onTargetToggle}
              />
            </>
          )}
        </div>
      )}

      <details className="text-xs">
        <summary className="text-gray-500 cursor-pointer hover:text-gray-400">
          {t("files", { count: audit.files.length })}
        </summary>
        <ul className="mt-1 max-h-32 overflow-y-auto space-y-0.5 pl-3">
          {audit.files.map((f, i) => (
            <li key={i} className="text-gray-600 font-mono truncate">
              {f.path}
            </li>
          ))}
        </ul>
      </details>

      {requireRiskAck && (
        <label className="flex items-start gap-2 text-xs text-gray-300 bg-gray-800/60 rounded px-3 py-2 border border-gray-700 cursor-pointer">
          <input
            type="checkbox"
            checked={riskAck}
            onChange={(e) => onRiskAckChange(e.target.checked)}
            className="mt-0.5 accent-amber-500"
          />
          <span>{t("acknowledge")}</span>
        </label>
      )}

      {requireConflictAck && (
        <label className="flex items-start gap-2 text-xs text-gray-300 bg-red-950/40 rounded px-3 py-2 border border-red-900/60 cursor-pointer">
          <input
            type="checkbox"
            checked={conflictAck}
            onChange={(e) => onConflictAckChange(e.target.checked)}
            className="mt-0.5 accent-red-500"
          />
          <span>{t("review.conflictAcknowledge")}</span>
        </label>
      )}

      {error && <p className="text-xs text-red-400">{error}</p>}

      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="text-xs text-gray-500 hover:text-gray-300"
        >
          {tc("back")}
        </button>
        <button
          onClick={onInstall}
          disabled={installDisabled}
          className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 text-white rounded"
        >
          {loading ? t("installing") : t("installAll", { count: selectedTargets.length })}
        </button>
      </div>
    </div>
  );
}

type TargetRowStatus = "ready" | "conflict" | "incompatible";

function TargetPlanTable({
  visibleTargets,
  selectedKeys,
  candidate,
  reviewState,
  reviewOutcome,
  onTargetToggle,
}: {
  visibleTargets: TargetDto[];
  selectedKeys: Set<string>;
  candidate: ImportCandidateDto;
  reviewState: ReviewState;
  reviewOutcome: ReviewOutcomeDto | null;
  onTargetToggle: (t: TargetDto, checked: boolean) => void;
}) {
  const { t } = useTranslation("wizard");

  return (
    <ul className="space-y-1.5">
      {visibleTargets.map((target) => {
        const k = targetKey(target);
        const checked = selectedKeys.has(k);

        const incompatible = candidate.compatibilityReviews.some(
          (r) =>
            r.status === "incompatible" &&
            (r.target == null || r.target.tool === target.tool)
        );
        const conflict =
          reviewState === "ready" &&
          reviewOutcome?.conflicts.some((c) => c.tool === target.tool);

        const status: TargetRowStatus = incompatible
          ? "incompatible"
          : conflict
            ? "conflict"
            : "ready";

        const statusBadge =
          status === "incompatible" ? (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-red-950 text-red-400 border border-red-900">
              {t("planStatus.incompatible", { defaultValue: "Incompatible" })}
            </span>
          ) : status === "conflict" ? (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-amber-950 text-amber-300 border border-amber-900">
              {t("planStatus.conflict", { defaultValue: "Conflict" })}
            </span>
          ) : reviewState === "loading" ? (
            <span className="text-[10px] text-gray-600 animate-pulse">
              {t("planStatus.checking", { defaultValue: "Checking…" })}
            </span>
          ) : (
            <span className="text-[10px] px-1.5 py-0.5 rounded bg-emerald-950 text-emerald-300 border border-emerald-900">
              {t("planStatus.ready", { defaultValue: "Ready" })}
            </span>
          );

        const conflictPath = conflict
          ? reviewOutcome?.conflicts.find((c) => c.tool === target.tool)?.name
          : undefined;

        return (
          <li key={k} className="rounded border border-gray-800 px-3 py-2">
            <label
              className={`flex items-center justify-between gap-2 ${
                status === "incompatible"
                  ? "opacity-40 cursor-not-allowed"
                  : "cursor-pointer"
              }`}
            >
              <span className="flex items-center gap-2 text-sm text-gray-200">
                <input
                  type="checkbox"
                  checked={checked}
                  disabled={status === "incompatible"}
                  onChange={(e) => onTargetToggle(target, e.target.checked)}
                  className="accent-indigo-500"
                />
                {targetLabel(target)}
              </span>
              {statusBadge}
            </label>
            {conflictPath && (
              <p className="mt-1 ml-5 text-[10px] text-amber-500 font-mono break-all">
                {t("planStatus.conflictWith", {
                  defaultValue: "Conflicts with: {{name}}",
                  name: conflictPath,
                })}
              </p>
            )}
          </li>
        );
      })}
    </ul>
  );
}

function ReviewSection({
  state,
  outcome,
  error,
}: {
  state: ReviewState;
  outcome: ReviewOutcomeDto | null;
  error: string | null;
}) {
  const { t } = useTranslation("wizard");

  if (state === "loading") {
    return (
      <div className="rounded border border-gray-800 bg-gray-900/60 px-3 py-2">
        <p className="text-xs font-semibold text-gray-400 mb-0.5">
          {t("review.title")}
        </p>
        <p className="text-xs text-gray-500">⟳ {t("review.loading")}</p>
      </div>
    );
  }

  if (state === "skipped") {
    return (
      <div className="rounded border border-gray-800 bg-gray-900/60 px-3 py-2">
        <p className="text-xs font-semibold text-gray-400 mb-0.5">
          {t("review.title")}
        </p>
        <p className="text-xs text-gray-500">{t("review.skipped")}</p>
        <p className="text-xs text-gray-600 mt-0.5">{t("review.skipNote")}</p>
      </div>
    );
  }

  if (state === "failed") {
    return (
      <div className="rounded border border-amber-900/50 bg-amber-950/30 px-3 py-2">
        <p className="text-xs font-semibold text-amber-300 mb-0.5">
          {t("review.title")}
        </p>
        <p className="text-xs text-amber-400">
          {t("review.failed")}
          {error ? `: ${error}` : ""}
        </p>
      </div>
    );
  }

  if (!outcome) return null;

  const ratingStyles: Record<typeof outcome.rating, string> = {
    safe: "bg-emerald-950 text-emerald-300 border-emerald-900",
    caution: "bg-amber-950 text-amber-300 border-amber-900",
    conflict: "bg-red-950 text-red-300 border-red-900",
  };

  return (
    <div className="rounded border border-gray-800 bg-gray-900/60 px-3 py-2 space-y-2">
      <div className="flex items-center justify-between gap-2">
        <p className="text-xs font-semibold text-gray-300">{t("review.title")}</p>
        <span
          className={`inline-flex items-center px-2 py-0.5 rounded border text-[10px] font-medium uppercase tracking-wide ${ratingStyles[outcome.rating]}`}
        >
          {t(`review.rating.${outcome.rating}`)}
        </span>
      </div>
      {outcome.skillPurpose && (
        <p className="text-xs text-gray-400">
          <span className="text-gray-500">{t("review.purpose")}: </span>
          {outcome.skillPurpose}
        </p>
      )}
      <p className="text-xs text-gray-300 leading-relaxed">{outcome.summary}</p>
      {outcome.conflicts.length > 0 && (
        <div>
          <p className="text-xs font-semibold text-gray-400 mb-1">
            {t("review.conflictsTitle", { count: outcome.conflicts.length })}
          </p>
          <ul className="space-y-1">
            {outcome.conflicts.map((c, i) => (
              <li key={i} className="text-xs text-gray-300">
                <span className="font-mono">{c.name}</span>{" "}
                <span className="text-gray-500">
                  ({c.kind} / {c.tool})
                </span>{" "}
                <span className="text-amber-400">
                  [{t(`review.reasonKind.${camelCaseReason(c.reasonKind)}`)}]
                </span>{" "}
                {c.reason}
              </li>
            ))}
          </ul>
        </div>
      )}
      <p className="text-[10px] text-gray-600 font-mono">
        {outcome.providerKind} / {outcome.model}
      </p>
    </div>
  );
}

function camelCaseReason(kind: string): string {
  // snake_case → camelCase for i18n keys
  return kind
    .split("_")
    .map((seg, i) => (i === 0 ? seg : seg.charAt(0).toUpperCase() + seg.slice(1)))
    .join("");
}

function DoneStep({
  outcomes,
  onClose,
  errorMessage,
}: {
  outcomes: InstallOutcomeDto[];
  onClose: () => void;
  errorMessage: (e: unknown) => string;
}) {
  const { t } = useTranslation("wizard");
  const { t: tc } = useTranslation("common");
  const allOk = outcomes.length > 0 && outcomes.every((o) => o.ok);
  const hasClaudeCodeInstall = outcomes.some(
    (o) => o.ok && o.target.tool === "claude-code"
  );
  return (
    <div className="space-y-3 py-2">
      <p
        className={`text-sm font-medium ${allOk ? "text-emerald-400" : "text-amber-400"}`}
      >
        {allOk ? t("successMessage") : t("partialSuccess")}
      </p>
      <ul className="space-y-1.5">
        {outcomes.map((o, i) => (
          <li key={i} className="text-sm flex items-start gap-2">
            <span
              className={`mt-0.5 ${o.ok ? "text-emerald-400" : "text-red-400"}`}
            >
              {o.ok ? "✓" : "✗"}
            </span>
            <span className="min-w-0 flex-1">
              <span className="block text-gray-200">{targetLabel(o.target)}</span>
              {o.ok && o.installation && (
                <span className="block text-xs text-gray-500 font-mono break-all">
                  {o.installation.onDiskPath}
                </span>
              )}
              {!o.ok && o.error && (
                <span className="block text-xs text-red-400">
                  {errorMessage(o.error)}
                </span>
              )}
            </span>
          </li>
        ))}
      </ul>
      {hasClaudeCodeInstall && (
        <p className="text-xs text-gray-500 leading-relaxed">
          {t("claudeReloadHint")}
        </p>
      )}
      <div className="pt-2 flex justify-end">
        <button
          onClick={onClose}
          className="text-sm text-gray-400 hover:text-gray-200"
        >
          {tc("close")}
        </button>
      </div>
    </div>
  );
}
