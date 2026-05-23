import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import { diffLines } from "diff";
import type { SkillDraftMode, SkillDraftPreviewDto } from "../types";
import { CompatibilityNotice } from "./CompatibilityNotice";

interface Props {
  mode: SkillDraftMode;
  preview: SkillDraftPreviewDto;
  busy: boolean;
  onConfirm: (override: { name: string }) => void;
  onCancel: () => void;
  errorMessage?: string;
}

export function SkillPreviewModal({
  mode,
  preview,
  busy,
  onConfirm,
  onCancel,
  errorMessage,
}: Props) {
  const { t } = useTranslation("artifact");
  const [name, setName] = useState(preview.adaptedName);
  const [showRaw, setShowRaw] = useState(false);

  const conflict = preview.nameConflict;
  const conflictUnresolved =
    !!conflict && name.trim() === preview.adaptedName.trim();

  const incompatible = preview.compatibilityReviews.some(
    (r) => r.status === "incompatible"
  );

  const titleKey =
    mode === "adapt"
      ? "adapt.title"
      : mode === "fork"
        ? "fork.title"
        : "edit.title";
  const subtitleKey =
    mode === "adapt"
      ? "adapt.subtitle"
      : mode === "fork"
        ? "fork.subtitle"
        : null;
  const confirmKey =
    mode === "adapt"
      ? busy
        ? "adapt.confirming"
        : "adapt.confirm"
      : mode === "fork"
        ? busy
          ? "adapt.confirming"
          : "fork.confirm"
        : busy
          ? "edit.saving"
          : "edit.saveReview";

  const diff = useMemo(
    () => diffLines(preview.originalContent, preview.adaptedContent),
    [preview.originalContent, preview.adaptedContent]
  );

  const noChanges = diff.every((part) => !part.added && !part.removed);

  const canConfirm = !busy && !conflictUnresolved && !incompatible;

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-900 border border-gray-700 rounded-lg w-full max-w-3xl shadow-xl max-h-[90vh] flex flex-col">
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-700">
          <h2 className="text-base font-semibold text-gray-100">{t(titleKey)}</h2>
          <button
            onClick={onCancel}
            className="text-gray-500 hover:text-gray-300 text-lg leading-none"
          >
            ✕
          </button>
        </div>

        <div className="px-5 py-4 overflow-y-auto space-y-4">
          {subtitleKey && (
            <p className="text-xs text-gray-400">{t(subtitleKey)}</p>
          )}

          <LineageBlock preview={preview} />

          <NameField
            value={name}
            onChange={setName}
            conflictPath={conflict?.existingPath}
            conflictUnresolved={conflictUnresolved}
          />

          <CompatibilityNotice reviews={preview.compatibilityReviews} />

          <DiffView diff={diff} noChanges={noChanges} />

          <details className="rounded border border-gray-800">
            <summary className="cursor-pointer px-3 py-2 text-xs text-gray-400 hover:text-gray-200">
              {t("diff.title")}
            </summary>
            <div className="grid grid-cols-2 divide-x divide-gray-800 border-t border-gray-800 text-[11px]">
              <pre className="p-3 overflow-x-auto text-gray-400 whitespace-pre-wrap">
                {preview.originalContent}
              </pre>
              <pre className="p-3 overflow-x-auto text-gray-200 whitespace-pre-wrap">
                {preview.adaptedContent}
              </pre>
            </div>
          </details>

          {showRaw && null /* placeholder slot if future raw toggle needed */}
          <button
            type="button"
            onClick={() => setShowRaw((v) => !v)}
            className="hidden"
          />

          {errorMessage && (
            <p className="text-xs text-red-400">{errorMessage}</p>
          )}
        </div>

        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-gray-700">
          <button
            onClick={onCancel}
            disabled={busy}
            className="text-xs text-gray-300 hover:text-gray-100 border border-gray-700 hover:border-gray-500 rounded px-3 py-1.5 disabled:opacity-40"
          >
            {t("cancel", { defaultValue: "Cancel" })}
          </button>
          <button
            onClick={() => onConfirm({ name: name.trim() })}
            disabled={!canConfirm}
            className="text-xs text-emerald-200 hover:text-emerald-100 border border-emerald-800 hover:border-emerald-600 bg-emerald-950/40 rounded px-3 py-1.5 disabled:opacity-40"
          >
            {t(confirmKey)}
          </button>
        </div>
      </div>
    </div>
  );
}

function LineageBlock({ preview }: { preview: SkillDraftPreviewDto }) {
  const { t } = useTranslation("artifact");
  const headerKey =
    preview.lineage.sourceKind === "adaptation"
      ? "lineage.adaptedFrom"
      : "lineage.forkedFrom";
  const sha = preview.lineage.sourceHash.slice(0, 8);
  return (
    <div className="rounded border border-gray-800 px-3 py-2 text-[11px] text-gray-400">
      <p className="text-gray-300">
        <span className="text-gray-500">{t("lineage.header")}: </span>
        {t(headerKey)} <code className="text-gray-200">{preview.lineage.parentName}</code>
      </p>
      <p className="text-gray-500 mt-0.5">
        {t("lineage.sha")}: <code>{sha}</code>
      </p>
    </div>
  );
}

function NameField({
  value,
  onChange,
  conflictPath,
  conflictUnresolved,
}: {
  value: string;
  onChange: (next: string) => void;
  conflictPath?: string;
  conflictUnresolved: boolean;
}) {
  const { t } = useTranslation("artifact");
  return (
    <div>
      <label className="block text-xs text-gray-400 mb-1">{t("name")}</label>
      <input
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="w-full bg-gray-950 border border-gray-700 rounded px-2 py-1 text-sm text-gray-100 font-mono focus:outline-none focus:border-gray-500"
      />
      {conflictPath && (
        <p
          className={`mt-1 text-xs ${
            conflictUnresolved ? "text-yellow-300" : "text-gray-500"
          }`}
        >
          {t("nameConflict", { path: conflictPath })}
        </p>
      )}
    </div>
  );
}

function DiffView({
  diff,
  noChanges,
}: {
  diff: { value: string; added?: boolean; removed?: boolean }[];
  noChanges: boolean;
}) {
  const { t } = useTranslation("artifact");
  if (noChanges) {
    return (
      <p className="text-xs text-gray-500 italic">{t("diff.noChanges")}</p>
    );
  }
  return (
    <pre className="rounded border border-gray-800 bg-gray-950 p-3 text-[11px] font-mono leading-tight overflow-x-auto whitespace-pre-wrap">
      {diff.map((part, i) => {
        if (part.added) {
          return (
            <span key={i} className="block text-emerald-300 bg-emerald-950/30">
              {prefixLines(part.value, "+ ")}
            </span>
          );
        }
        if (part.removed) {
          return (
            <span key={i} className="block text-red-300 bg-red-950/30">
              {prefixLines(part.value, "- ")}
            </span>
          );
        }
        return (
          <span key={i} className="block text-gray-400">
            {prefixLines(part.value, "  ")}
          </span>
        );
      })}
    </pre>
  );
}

function prefixLines(value: string, prefix: string): string {
  return value
    .split("\n")
    .map((line, idx, arr) => {
      if (idx === arr.length - 1 && line === "") return "";
      return `${prefix}${line}`;
    })
    .filter((_, idx, arr) => !(idx === arr.length - 1 && arr[idx] === ""))
    .join("\n") + "\n";
}
