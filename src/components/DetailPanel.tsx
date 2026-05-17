import { useMutation, useQueryClient } from "@tanstack/react-query";
import { useCallback, useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { clearTranslationCache, translateArtifact } from "../api";
import { disable, enable, uninstall } from "../api";
import { useErrorMessage } from "../useErrorMessage";
import type {
  ArtifactGroupDto,
  ArtifactKind,
  InstallationDto,
  MarkdownWarningDto,
  ScannedInstallationDto,
  TranslateOutcomeDto,
} from "../types";
import { sourceLabel, targetLabel } from "../types";

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
  const [flash, setFlash] = useState<"copied" | "cleared" | null>(null);
  const flashTimer = useRef<ReturnType<typeof setTimeout> | null>(null);

  const group = groups.find(
    (g) => g.name === selectedName && g.kind === selectedKind
  );

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
    if (!group?.body || i18n.language !== TRANSLATE_LOCALE) return;

    const signal = { alive: true };
    runTranslate(false, signal);
    return () => {
      signal.alive = false;
    };
  }, [group?.body, group?.name, i18n.language, runTranslate]);

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

  const mutError =
    uninstallMut.error ?? enableMut.error ?? disableMut.error ?? null;

  const showTranslation = i18n.language === TRANSLATE_LOCALE && !!group.body;

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

      <Section title={t("source")}>
        {group.installations[0] ? (
          <p className="text-xs text-gray-400 break-all">
            {sourceLabel(group.installations[0].artifact.source)}
          </p>
        ) : (
          <p className="text-xs text-gray-600">{t("unknown")}</p>
        )}
      </Section>

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
                      <StatusBadge outcome={lastOutcome} label={ta(`badge.${badgeKey(lastOutcome)}`)} />
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
                <pre className="max-h-72 overflow-auto whitespace-pre-wrap rounded bg-gray-950 border border-gray-800 px-3 py-2 text-xs leading-relaxed text-gray-200">
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

      {owned.length > 0 && (
        <Section title={t("installed")}>
          <ul className="space-y-3">
            {owned.map((si) => (
              <InstallationRow
                key={si.installation.id}
                si={si}
                onUninstall={() => uninstallMut.mutate(si.installation)}
                onEnable={() => enableMut.mutate(si.installation)}
                onDisable={() => disableMut.mutate(si.installation)}
                busy={
                  uninstallMut.isPending ||
                  enableMut.isPending ||
                  disableMut.isPending
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
    </div>
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

function InstallationRow({
  si,
  onUninstall,
  onEnable,
  onDisable,
  busy,
}: {
  si: ScannedInstallationDto;
  onUninstall: () => void;
  onEnable: () => void;
  onDisable: () => void;
  busy: boolean;
}) {
  const { t } = useTranslation("common");
  const { installation } = si;
  const isDisabled = installation.status === "disabled";

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
      <div className="mt-2 flex gap-2">
        {isDisabled ? (
          <ActionButton onClick={onEnable} disabled={busy}>
            {t("enable")}
          </ActionButton>
        ) : (
          <ActionButton onClick={onDisable} disabled={busy}>
            {t("disable")}
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
