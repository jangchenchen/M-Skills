import type { AdapterStatusDto, ArtifactKind } from "../types";

interface Props {
  adapters: AdapterStatusDto[];
  selectedKind: ArtifactKind | null;
  onKindSelect: (kind: ArtifactKind | null) => void;
  onImportClick: () => void;
}

const KIND_LABELS: Record<ArtifactKind, string> = {
  Skill: "Skills",
  Extension: "Extensions",
  Workflow: "Workflows",
};

export function Sidebar({
  adapters,
  selectedKind,
  onKindSelect,
  onImportClick,
}: Props) {
  return (
    <aside className="w-52 flex-none bg-gray-900 text-gray-100 flex flex-col h-full">
      <div className="px-4 py-5 border-b border-gray-700">
        <h1 className="text-base font-semibold tracking-tight">M-Skills</h1>
      </div>

      <nav className="flex-1 overflow-y-auto px-2 py-3 space-y-1">
        <button
          onClick={() => onKindSelect(null)}
          className={`w-full text-left px-3 py-2 rounded text-sm ${
            selectedKind === null
              ? "bg-indigo-600 text-white"
              : "text-gray-300 hover:bg-gray-800"
          }`}
        >
          All artifacts
        </button>
        {(["Skill", "Extension", "Workflow"] as ArtifactKind[]).map((k) => (
          <button
            key={k}
            onClick={() => onKindSelect(k)}
            className={`w-full text-left px-3 py-2 rounded text-sm ${
              selectedKind === k
                ? "bg-indigo-600 text-white"
                : "text-gray-300 hover:bg-gray-800"
            }`}
          >
            {KIND_LABELS[k]}
          </button>
        ))}
      </nav>

      <div className="px-2 py-3 border-t border-gray-700 space-y-3">
        <div className="px-3">
          <p className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-2">
            Tools
          </p>
          <ul className="space-y-1">
            {adapters.map((a) => (
              <li key={a.adapterId} className="flex items-center gap-2 text-xs">
                <span
                  className={`h-2 w-2 rounded-full flex-none ${
                    a.presence.type === "Available"
                      ? "bg-emerald-400"
                      : "bg-gray-600"
                  }`}
                />
                <span className="truncate text-gray-400">{a.adapterId}</span>
              </li>
            ))}
          </ul>
        </div>

        <button
          onClick={onImportClick}
          className="w-full text-center text-sm bg-indigo-600 hover:bg-indigo-500 text-white px-3 py-2 rounded"
        >
          + Import
        </button>
      </div>
    </aside>
  );
}
