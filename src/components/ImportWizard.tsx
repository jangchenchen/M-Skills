import { useState } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { install, previewImport } from "../api";
import type { ImportCandidateDto, ImportPreviewDto, TargetDto } from "../types";
import { targetLabel } from "../types";

interface Props {
  onClose: () => void;
}

type Step = "input" | "preview" | "done";

export function ImportWizard({ onClose }: Props) {
  const [step, setStep] = useState<Step>("input");
  const [pathOrUrl, setPathOrUrl] = useState("");
  const [preview, setPreview] = useState<ImportPreviewDto | null>(null);
  const [selectedCandidate, setSelectedCandidate] =
    useState<ImportCandidateDto | null>(null);
  const [selectedTarget, setSelectedTarget] = useState<TargetDto | null>(null);
  const qc = useQueryClient();

  const previewMut = useMutation({
    mutationFn: () => previewImport(pathOrUrl),
    onSuccess: (data) => {
      setPreview(data);
      if (data.candidates.length > 0) {
        setSelectedCandidate(data.candidates[0]);
        if (data.candidates[0].compatibleTargets.length > 0) {
          setSelectedTarget(data.candidates[0].compatibleTargets[0]);
        }
      }
      setStep("preview");
    },
  });

  const installMut = useMutation({
    mutationFn: () => {
      if (!selectedCandidate || !selectedTarget) throw new Error("incomplete");
      return install(selectedCandidate.index, selectedTarget);
    },
    onSuccess: () => {
      qc.invalidateQueries({ queryKey: ["inventory"] });
      setStep("done");
    },
  });

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/60">
      <div className="bg-gray-900 border border-gray-700 rounded-lg w-full max-w-xl shadow-xl">
        <div className="flex items-center justify-between px-5 py-4 border-b border-gray-700">
          <h2 className="text-base font-semibold text-gray-100">
            Import artifact
          </h2>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-300 text-lg leading-none"
          >
            ✕
          </button>
        </div>

        <div className="px-5 py-4">
          {step === "input" && (
            <InputStep
              value={pathOrUrl}
              onChange={setPathOrUrl}
              onSubmit={() => previewMut.mutate()}
              loading={previewMut.isPending}
              error={previewMut.error?.message}
            />
          )}

          {step === "preview" && preview && (
            <PreviewStep
              preview={preview}
              selectedCandidate={selectedCandidate}
              selectedTarget={selectedTarget}
              onCandidateChange={(c) => {
                setSelectedCandidate(c);
                setSelectedTarget(c.compatibleTargets[0] ?? null);
              }}
              onTargetChange={setSelectedTarget}
              onInstall={() => installMut.mutate()}
              onBack={() => setStep("input")}
              loading={installMut.isPending}
              error={installMut.error?.message}
            />
          )}

          {step === "done" && (
            <div className="py-6 text-center">
              <p className="text-emerald-400 text-sm font-medium">
                Installed successfully.
              </p>
              <button
                onClick={onClose}
                className="mt-4 text-sm text-gray-400 hover:text-gray-200"
              >
                Close
              </button>
            </div>
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
}: {
  value: string;
  onChange: (v: string) => void;
  onSubmit: () => void;
  loading: boolean;
  error?: string;
}) {
  return (
    <div className="space-y-4">
      <p className="text-sm text-gray-400">
        Enter a local directory path or GitHub URL.
      </p>
      <input
        type="text"
        className="w-full rounded bg-gray-800 border border-gray-600 px-3 py-2 text-sm text-gray-100 placeholder-gray-600 focus:outline-none focus:border-indigo-500"
        placeholder="/path/to/skill  or  https://github.com/org/repo"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onKeyDown={(e) => e.key === "Enter" && !loading && onSubmit()}
      />
      {error && <p className="text-xs text-red-400">{error}</p>}
      <div className="flex justify-end">
        <button
          onClick={onSubmit}
          disabled={loading || !value.trim()}
          className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 text-white rounded"
        >
          {loading ? "Loading…" : "Preview"}
        </button>
      </div>
    </div>
  );
}

function PreviewStep({
  preview,
  selectedCandidate,
  selectedTarget,
  onCandidateChange,
  onTargetChange,
  onInstall,
  onBack,
  loading,
  error,
}: {
  preview: ImportPreviewDto;
  selectedCandidate: ImportCandidateDto | null;
  selectedTarget: TargetDto | null;
  onCandidateChange: (c: ImportCandidateDto) => void;
  onTargetChange: (t: TargetDto) => void;
  onInstall: () => void;
  onBack: () => void;
  loading: boolean;
  error?: string;
}) {
  const { audit } = preview;

  if (preview.candidates.length === 0) {
    return (
      <div className="py-6 text-center text-sm text-gray-400">
        No supported artifact found in this source.
        <div className="mt-4">
          <button onClick={onBack} className="text-xs text-gray-500 hover:text-gray-300">
            ← Back
          </button>
        </div>
      </div>
    );
  }

  return (
    <div className="space-y-4">
      {preview.commitSha && (
        <p className="text-xs text-gray-500">
          commit: <span className="font-mono">{preview.commitSha.slice(0, 12)}</span>
        </p>
      )}

      {audit.warnings.length > 0 && (
        <div className="rounded bg-yellow-900/30 border border-yellow-700/50 px-3 py-2">
          <p className="text-xs font-semibold text-yellow-400 mb-1">Warnings</p>
          <ul className="space-y-1">
            {audit.warnings.map((w, i) => (
              <li key={i} className="text-xs text-yellow-300">
                [{w.kind}] {w.message}
              </li>
            ))}
          </ul>
        </div>
      )}

      <div>
        <label className="text-xs text-gray-400 block mb-1">Artifact</label>
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
              {c.artifact.name} ({c.artifact.kind})
            </option>
          ))}
        </select>
      </div>

      {selectedCandidate && (
        <div>
          <label className="text-xs text-gray-400 block mb-1">
            Install target
          </label>
          {selectedCandidate.compatibleTargets.length === 0 ? (
            <p className="text-xs text-red-400">No compatible targets available.</p>
          ) : (
            <select
              className="w-full bg-gray-800 border border-gray-600 rounded px-2 py-1.5 text-sm text-gray-100"
              value={
                selectedTarget
                  ? `${selectedTarget.tool}:${selectedTarget.scope.type}`
                  : ""
              }
              onChange={(e) => {
                const found = selectedCandidate.compatibleTargets.find(
                  (t) => `${t.tool}:${t.scope.type}` === e.target.value
                );
                if (found) onTargetChange(found);
              }}
            >
              {selectedCandidate.compatibleTargets.map((t) => (
                <option
                  key={`${t.tool}:${t.scope.type}`}
                  value={`${t.tool}:${t.scope.type}`}
                >
                  {targetLabel(t)}
                </option>
              ))}
            </select>
          )}
        </div>
      )}

      <details className="text-xs">
        <summary className="text-gray-500 cursor-pointer hover:text-gray-400">
          {audit.files.length} files
        </summary>
        <ul className="mt-1 max-h-32 overflow-y-auto space-y-0.5 pl-3">
          {audit.files.map((f, i) => (
            <li key={i} className="text-gray-600 font-mono truncate">
              {f.path}
            </li>
          ))}
        </ul>
      </details>

      {error && <p className="text-xs text-red-400">{error}</p>}

      <div className="flex justify-between items-center">
        <button
          onClick={onBack}
          className="text-xs text-gray-500 hover:text-gray-300"
        >
          ← Back
        </button>
        <button
          onClick={onInstall}
          disabled={loading || !selectedCandidate || !selectedTarget}
          className="px-4 py-2 text-sm bg-indigo-600 hover:bg-indigo-500 disabled:opacity-40 text-white rounded"
        >
          {loading ? "Installing…" : "Install"}
        </button>
      </div>
    </div>
  );
}
