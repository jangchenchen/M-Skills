import { useMemo, useState } from "react";
import { useMutation } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { diffLines } from "diff";
import { rewriteSkillWithLlm, saveCustomSkillEdit } from "../api";
import { useErrorMessage } from "../useErrorMessage";
import type {
  ArtifactDto,
  LineageDto,
  RewriteMode,
  RewriteSkillOutcomeDto,
  SkillDraftPreviewDto,
  TargetDto,
} from "../types";
import { CompatibilityNotice } from "./CompatibilityNotice";

interface Props {
  initialContent: string;
  target: TargetDto;
  lineage: LineageDto;
  onReviewed: (preview: SkillDraftPreviewDto) => void;
  onCancel: () => void;
}

const REWRITE_MODES: RewriteMode[] = [
  "adapt_to_codex",
  "complete_missing_info",
  "reduce_risk",
  "customize_workflow",
  "simplify",
];

export function CustomSkillEditor({
  initialContent,
  target,
  lineage,
  onReviewed,
  onCancel,
}: Props) {
  const { t: ta } = useTranslation("artifact");
  const { i18n } = useTranslation();
  const errorMessage = useErrorMessage();
  const [content, setContent] = useState(initialContent);
  const [parseError, setParseError] = useState<string | null>(null);
  const [rewriteOpen, setRewriteOpen] = useState(false);
  const [rewriteMode, setRewriteMode] = useState<RewriteMode>("adapt_to_codex");
  const [rewriteInstruction, setRewriteInstruction] = useState("");
  const [rewriteOutcome, setRewriteOutcome] =
    useState<RewriteSkillOutcomeDto | null>(null);
  const [rewriteError, setRewriteError] = useState<string | null>(null);
  const [rewriteApplied, setRewriteApplied] = useState(false);

  const saveMut = useMutation({
    mutationFn: () =>
      saveCustomSkillEdit({ content, target, lineage }),
    onSuccess: (preview) => {
      setParseError(null);
      onReviewed(preview);
    },
    onError: (err) => setParseError(errorMessage(err)),
  });

  const rewriteMut = useMutation({
    mutationFn: () => {
      const artifact: ArtifactDto = {
        id: "00000000-0000-0000-0000-000000000000",
        name: lineage.parentName,
        description: "",
        body: content,
        version: null,
        kind: "Skill",
        source: { type: "Unknown" },
        capabilities: [],
      };
      return rewriteSkillWithLlm({
        artifact,
        mode: rewriteMode,
        userInstruction: rewriteInstruction,
        locale: i18n.resolvedLanguage ?? i18n.language ?? "en",
      });
    },
    onMutate: () => {
      setRewriteError(null);
      setRewriteApplied(false);
    },
    onSuccess: (outcome) => {
      setRewriteOutcome(outcome);
      setRewriteError(null);
    },
    onError: (err) => {
      setRewriteOutcome(null);
      setRewriteError(errorMessage(err));
    },
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-900 border border-gray-700 rounded-lg w-full max-w-4xl shadow-xl flex flex-col max-h-[90vh]">
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-700">
          <h2 className="text-base font-semibold text-gray-100">
            {ta("edit.title")}
          </h2>
          <button
            onClick={onCancel}
            className="text-gray-500 hover:text-gray-300 text-lg leading-none"
          >
            ✕
          </button>
        </div>
        <div className="px-5 py-4 space-y-3 overflow-y-auto">
          <textarea
            value={content}
            onChange={(e) => {
              setContent(e.target.value);
              setRewriteApplied(false);
            }}
            spellCheck={false}
            className="w-full h-72 bg-gray-950 border border-gray-700 rounded p-3 text-xs font-mono text-gray-100 focus:outline-none focus:border-gray-500"
          />
          {parseError && (
            <p className="text-xs text-red-400">{parseError}</p>
          )}

          <div className="rounded border border-gray-800">
            <button
              type="button"
              onClick={() => setRewriteOpen((v) => !v)}
              className="w-full flex items-center justify-between px-3 py-2 text-xs text-gray-300 hover:text-gray-100"
            >
              <span>
                {rewriteOpen ? ta("rewrite.closeButton") : ta("rewrite.openButton")}
              </span>
              <span className="text-gray-500">{rewriteOpen ? "▾" : "▸"}</span>
            </button>
            {rewriteOpen && (
              <RewritePanel
                mode={rewriteMode}
                onModeChange={setRewriteMode}
                instruction={rewriteInstruction}
                onInstructionChange={setRewriteInstruction}
                outcome={rewriteOutcome}
                error={rewriteError}
                busy={rewriteMut.isPending}
                applied={rewriteApplied}
                editorContent={content}
                onSubmit={() => rewriteMut.mutate()}
                onApplyDraft={(draft) => {
                  setContent(draft);
                  setRewriteApplied(true);
                }}
                onDiscardDraft={() => {
                  setRewriteOutcome(null);
                  setRewriteError(null);
                  setRewriteApplied(false);
                }}
              />
            )}
          </div>
        </div>
        <div className="flex items-center justify-end gap-2 px-5 py-3 border-t border-gray-700">
          <button
            onClick={onCancel}
            disabled={saveMut.isPending}
            className="text-xs text-gray-300 hover:text-gray-100 border border-gray-700 hover:border-gray-500 rounded px-3 py-1.5 disabled:opacity-40"
          >
            ✕
          </button>
          <button
            onClick={() => saveMut.mutate()}
            disabled={saveMut.isPending}
            className="text-xs text-emerald-200 hover:text-emerald-100 border border-emerald-800 hover:border-emerald-600 bg-emerald-950/40 rounded px-3 py-1.5 disabled:opacity-40"
          >
            {saveMut.isPending ? ta("edit.saving") : ta("edit.saveReview")}
          </button>
        </div>
      </div>
    </div>
  );
}

function RewritePanel({
  mode,
  onModeChange,
  instruction,
  onInstructionChange,
  outcome,
  error,
  busy,
  applied,
  editorContent,
  onSubmit,
  onApplyDraft,
  onDiscardDraft,
}: {
  mode: RewriteMode;
  onModeChange: (next: RewriteMode) => void;
  instruction: string;
  onInstructionChange: (next: string) => void;
  outcome: RewriteSkillOutcomeDto | null;
  error: string | null;
  busy: boolean;
  applied: boolean;
  editorContent: string;
  onSubmit: () => void;
  onApplyDraft: (draftBody: string) => void;
  onDiscardDraft: () => void;
}) {
  const { t: ta } = useTranslation("artifact");

  const diff = useMemo(() => {
    if (!outcome) return [];
    return diffLines(editorContent, outcome.draftBody);
  }, [editorContent, outcome]);
  const noChanges =
    outcome != null && diff.every((part) => !part.added && !part.removed);

  return (
    <div className="border-t border-gray-800 px-3 py-3 space-y-3 text-xs">
      <p className="text-gray-400">{ta("rewrite.subtitle")}</p>
      <div className="grid grid-cols-1 sm:grid-cols-2 gap-3">
        <label className="block">
          <span className="block text-gray-400 mb-1">
            {ta("rewrite.modeLabel")}
          </span>
          <select
            value={mode}
            onChange={(e) => onModeChange(e.target.value as RewriteMode)}
            disabled={busy}
            className="w-full bg-gray-950 border border-gray-700 rounded px-2 py-1 text-gray-100 focus:outline-none focus:border-gray-500"
          >
            {REWRITE_MODES.map((m) => (
              <option key={m} value={m}>
                {ta(`rewrite.mode.${m}`)}
              </option>
            ))}
          </select>
        </label>
        <div className="flex items-end">
          <button
            type="button"
            onClick={onSubmit}
            disabled={busy}
            className="ml-auto text-xs text-emerald-200 hover:text-emerald-100 border border-emerald-800 hover:border-emerald-600 bg-emerald-950/40 rounded px-3 py-1.5 disabled:opacity-40"
          >
            {busy ? ta("rewrite.submitting") : ta("rewrite.submit")}
          </button>
        </div>
      </div>
      <label className="block">
        <span className="block text-gray-400 mb-1">
          {ta("rewrite.instructionLabel")}
        </span>
        <textarea
          value={instruction}
          onChange={(e) => onInstructionChange(e.target.value)}
          placeholder={ta("rewrite.instructionPlaceholder")}
          disabled={busy}
          spellCheck={false}
          rows={3}
          className="w-full bg-gray-950 border border-gray-700 rounded p-2 text-gray-100 focus:outline-none focus:border-gray-500"
        />
      </label>

      {error && <p className="text-red-400 break-words">{error}</p>}

      {outcome && (
        <div className="space-y-2">
          <p className="text-gray-500">
            {ta("rewrite.providedBy", {
              providerKind: outcome.providerKind,
              model: outcome.model,
            })}
          </p>
          <div>
            <p className="font-medium text-gray-200">{ta("rewrite.summary")}</p>
            <p className="mt-0.5 text-gray-300 whitespace-pre-wrap">
              {outcome.summary}
            </p>
          </div>
          {outcome.notes.length > 0 && (
            <div>
              <p className="font-medium text-gray-200">{ta("rewrite.notes")}</p>
              <ul className="mt-0.5 list-disc list-inside text-gray-300 space-y-0.5">
                {outcome.notes.map((note, i) => (
                  <li key={i}>{note}</li>
                ))}
              </ul>
            </div>
          )}

          {outcome.compatibilityReviews.length > 0 && (
            <CompatibilityNotice reviews={outcome.compatibilityReviews} />
          )}

          <div>
            <p className="font-medium text-gray-200 mb-1">
              {ta("rewrite.diffHeader")}
            </p>
            {noChanges ? (
              <p className="text-gray-500 italic">{ta("rewrite.noChanges")}</p>
            ) : (
              <pre className="rounded border border-gray-800 bg-gray-950 p-3 text-[11px] font-mono leading-tight overflow-x-auto whitespace-pre-wrap max-h-72">
                {diff.map((part, i) => {
                  if (part.added) {
                    return (
                      <span
                        key={i}
                        className="block text-emerald-300 bg-emerald-950/30"
                      >
                        {prefixLines(part.value, "+ ")}
                      </span>
                    );
                  }
                  if (part.removed) {
                    return (
                      <span
                        key={i}
                        className="block text-red-300 bg-red-950/30"
                      >
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
            )}
          </div>

          <details className="rounded border border-gray-800">
            <summary className="cursor-pointer px-3 py-2 text-gray-400 hover:text-gray-200">
              {ta("rewrite.draftHeader")}
            </summary>
            <pre className="px-3 py-2 text-gray-300 whitespace-pre-wrap text-[11px] max-h-72 overflow-auto">
              {outcome.draftBody}
            </pre>
          </details>

          {applied && (
            <p className="text-emerald-300">{ta("rewrite.appliedNote")}</p>
          )}

          <div className="flex gap-2">
            <button
              type="button"
              onClick={() => onApplyDraft(outcome.draftBody)}
              disabled={busy || applied || noChanges}
              className="text-xs text-emerald-200 hover:text-emerald-100 border border-emerald-800 hover:border-emerald-600 bg-emerald-950/40 rounded px-3 py-1.5 disabled:opacity-40"
            >
              {ta("rewrite.applyDraft")}
            </button>
            <button
              type="button"
              onClick={onDiscardDraft}
              disabled={busy}
              className="text-xs text-gray-300 hover:text-gray-100 border border-gray-700 hover:border-gray-500 rounded px-3 py-1.5 disabled:opacity-40"
            >
              {ta("rewrite.discardDraft")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}

function prefixLines(value: string, prefix: string): string {
  return (
    value
      .split("\n")
      .map((line, idx, arr) => {
        if (idx === arr.length - 1 && line === "") return "";
        return `${prefix}${line}`;
      })
      .filter((_, idx, arr) => !(idx === arr.length - 1 && arr[idx] === ""))
      .join("\n") + "\n"
  );
}
