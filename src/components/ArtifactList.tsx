import { useTranslation } from "react-i18next";
import type { ArtifactGroupDto, ArtifactKind } from "../types";

interface Props {
  groups: ArtifactGroupDto[];
  selectedKind: ArtifactKind | null;
  selectedName: string | null;
  onSelect: (name: string, kind: ArtifactKind) => void;
}

const KIND_BADGE: Record<ArtifactKind, string> = {
  Skill: "bg-blue-900 text-blue-300",
  Extension: "bg-purple-900 text-purple-300",
  Workflow: "bg-amber-900 text-amber-300",
};

export function ArtifactList({
  groups,
  selectedKind,
  selectedName,
  onSelect,
}: Props) {
  const { t } = useTranslation("artifact");
  const { t: tc } = useTranslation("common");

  const visible = selectedKind
    ? groups.filter((g) => g.kind === selectedKind)
    : groups;

  if (visible.length === 0) {
    return (
      <div className="flex items-center justify-center h-full text-gray-500 text-sm">
        {tc("noArtifacts")}
      </div>
    );
  }

  return (
    <ul className="divide-y divide-gray-800">
      {visible.map((g) => {
        const isSelected = g.name === selectedName && g.kind === selectedKind;
        const installed = g.installations.filter(
          (i) => i.provenance === "owned"
        ).length;

        return (
          <li key={`${g.kind}/${g.name}`}>
            <button
              onClick={() => onSelect(g.name, g.kind)}
              className={`w-full text-left px-4 py-3 hover:bg-gray-800 transition-colors ${
                isSelected ? "bg-gray-800 border-l-2 border-indigo-500" : ""
              }`}
            >
              <div className="flex items-center gap-2">
                <span
                  className={`text-xs px-1.5 py-0.5 rounded font-medium ${KIND_BADGE[g.kind]}`}
                >
                  {t(`kind.${g.kind}`)}
                </span>
                <span className="text-sm font-medium text-gray-100 truncate">
                  {g.name}
                </span>
              </div>
              {g.description && (
                <p className="mt-0.5 text-xs text-gray-500 truncate">
                  {g.description}
                </p>
              )}
              <div className="mt-1 flex gap-2 text-xs text-gray-600">
                {installed > 0 && (
                  <span className="text-emerald-600">
                    {t("installedCount", { count: installed })}
                  </span>
                )}
                {g.alsoVisibleTo.length > 0 && (
                  <span>
                    {t("visibleTo", { tools: g.alsoVisibleTo.join(", ") })}
                  </span>
                )}
              </div>
            </button>
          </li>
        );
      })}
    </ul>
  );
}
