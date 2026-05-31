import { useMemo, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  matchesSkillCategory,
  type SkillCategoryId,
} from "../skillCategories";
import type {
  ArtifactDto,
  ArtifactGroupDto,
  ArtifactKind,
  SourceDto,
} from "../types";

interface Props {
  groups: ArtifactGroupDto[];
  selectedKind: ArtifactKind | null;
  selectedSkillCategory: SkillCategoryId;
  selectedName: string | null;
  selectedTool: string | null;
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
  selectedSkillCategory,
  selectedName,
  selectedTool,
  onSelect,
}: Props) {
  const { t } = useTranslation("artifact");
  const { t: tc } = useTranslation("common");
  const [query, setQuery] = useState("");

  const visible = useMemo(
    () =>
      groups.filter((g) => {
        if (selectedKind && g.kind !== selectedKind) return false;
        if (
          selectedKind === "Skill" &&
          !matchesSkillCategory(g, selectedSkillCategory)
        )
          return false;
        if (selectedTool && !isVisibleForTool(g, selectedTool)) return false;
        return matchesArtifactSearch(g, query);
      }),
    [groups, query, selectedKind, selectedSkillCategory, selectedTool]
  );

  if (visible.length === 0) {
    return (
      <div className="flex h-full flex-col">
        <SearchBox query={query} setQuery={setQuery} tc={tc} />
        <div className="flex flex-1 items-center justify-center px-4 text-center text-gray-500 text-sm">
          {query.trim() ? tc("noSearchResults") : tc("noArtifacts")}
        </div>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col">
      <SearchBox query={query} setQuery={setQuery} tc={tc} />
      <ul className="divide-y divide-gray-800 overflow-y-auto">
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
                  <span className="text-emerald-600">
                    {t("installedCount", { count: installed })}
                  </span>
                  {selectedTool && (
                    <span>{tc("installedIn", { tool: selectedTool })}</span>
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
    </div>
  );
}

function SearchBox({
  query,
  setQuery,
  tc,
}: {
  query: string;
  setQuery: (query: string) => void;
  tc: (key: string) => string;
}) {
  return (
    <div className="sticky top-0 z-10 border-b border-gray-800 bg-gray-950/95 p-3">
      <input
        value={query}
        onChange={(event) => setQuery(event.target.value)}
        placeholder={tc("searchArtifacts")}
        className="w-full rounded border border-gray-800 bg-gray-900 px-3 py-2 text-sm text-gray-100 placeholder:text-gray-600 focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
      />
    </div>
  );
}

export function isVisibleForTool(group: ArtifactGroupDto, tool: string): boolean {
  return (
    group.installations.some((i) => i.installation.target.tool === tool) ||
    group.alsoVisibleTo.includes(tool)
  );
}

export function matchesArtifactSearch(
  group: ArtifactGroupDto,
  rawQuery: string
): boolean {
  const query = rawQuery.trim();
  if (!query) return true;

  const haystack = normalizeSearchText([
    group.name,
    group.description,
    group.body ?? "",
    group.kind,
    ...group.searchAliases,
    group.alsoVisibleTo.join(" "),
    ...group.capabilities.flatMap((capability) => [
      capability.name,
      capability.description,
    ]),
    ...group.installations.flatMap((installation) => [
      installation.provenance,
        installation.installation.target.tool,
        installation.installation.onDiskPath,
        artifactSourceText(installation.artifact),
        installation.artifact.name,
        installation.artifact.description,
        installation.artifact.body ?? "",
        ...installation.artifact.searchAliases,
      ]),
  ]);
  const normalizedQuery = normalizeSearchText([query]);
  if (haystack.includes(normalizedQuery)) return true;

  const tokens = searchTokens(query);
  if (tokens.length === 0) return true;
  if (tokens.every((token) => haystack.includes(token))) return true;

  return false;
}

function artifactSourceText(artifact: ArtifactDto): string {
  return sourceText(artifact.source);
}

function sourceText(source: SourceDto): string {
  switch (source.type) {
    case "gitHub":
      return `${source.url} ${source.rev}`;
    case "url":
      return source.url;
    case "local":
      return source.path;
    default:
      return source.type;
  }
}

function normalizeSearchText(parts: string[]): string {
  return parts.join(" ").toLowerCase();
}

function searchTokens(query: string): string[] {
  return Array.from(
    new Set(
      query
        .toLowerCase()
        .split(/[^a-z0-9\u4e00-\u9fff]+/u)
        .map((token) => token.trim())
        .filter((token) => token.length >= 3)
        .filter(
          (token) => !["skill", "skills", "plugin", "plugins"].includes(token)
        )
    )
  );
}
