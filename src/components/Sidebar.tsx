import { useTranslation } from "react-i18next";
import {
  SKILL_CATEGORY_MENU_IDS,
  type SkillCategoryCounts,
  type SkillCategoryId,
} from "../skillCategories";
import type { AdapterStatusDto, ArtifactKind } from "../types";

export type SidebarView = "dashboard" | "library" | "market";

interface Props {
  adapters: AdapterStatusDto[];
  skillCategoryCounts: SkillCategoryCounts;
  selectedKind: ArtifactKind | null;
  selectedSkillCategory: SkillCategoryId;
  selectedTool: string | null;
  activeView: SidebarView;
  onDashboardSelect: () => void;
  onMarketSelect: () => void;
  onKindSelect: (kind: ArtifactKind | null) => void;
  onSkillCategorySelect: (category: SkillCategoryId) => void;
  onImportClick: () => void;
  onSettingsClick: () => void;
}

const KINDS: ArtifactKind[] = ["Skill", "Extension", "Workflow"];

export function Sidebar({
  adapters,
  skillCategoryCounts,
  selectedKind,
  selectedSkillCategory,
  selectedTool,
  activeView,
  onDashboardSelect,
  onMarketSelect,
  onKindSelect,
  onSkillCategorySelect,
  onImportClick,
  onSettingsClick,
}: Props) {
  const { t, i18n } = useTranslation("common");
  const { t: ta } = useTranslation("artifact");
  const { t: ts } = useTranslation("settings");

  function handleLangToggle() {
    const next = i18n.language === "zh" ? "en" : "zh";
    i18n.changeLanguage(next);
    localStorage.setItem("m-skills-lang", next);
  }

  return (
    <aside className="w-52 flex-none bg-gray-900 text-gray-100 flex flex-col h-full">
      <div className="px-4 py-5 border-b border-gray-700 flex items-center justify-between">
        <h1 className="text-base font-semibold tracking-tight">M-Skills</h1>
        <div className="flex items-center gap-1">
          <button
            onClick={onSettingsClick}
            className="text-xs text-gray-500 hover:text-gray-300 px-1.5 py-0.5 rounded border border-gray-700 hover:border-gray-500"
            title={ts("open")}
            aria-label={ts("open")}
          >
            ⚙
          </button>
          <button
            onClick={handleLangToggle}
            className="text-xs text-gray-500 hover:text-gray-300 px-1.5 py-0.5 rounded border border-gray-700 hover:border-gray-500"
            title="Switch language / 切换语言"
          >
            {i18n.language === "zh" ? "EN" : "中"}
          </button>
        </div>
      </div>

      <nav className="flex-1 overflow-y-auto px-2 py-3 space-y-1">
        <button
          onClick={onDashboardSelect}
          className={`w-full text-left px-3 py-2 rounded text-sm ${
            activeView === "dashboard"
              ? "bg-indigo-600 text-white"
              : "text-gray-300 hover:bg-gray-800"
          }`}
        >
          {t("dashboard")}
        </button>
        <button
          onClick={() => onKindSelect(null)}
          className={`w-full text-left px-3 py-2 rounded text-sm ${
            activeView === "library" &&
            selectedKind === null &&
            selectedTool === null
              ? "bg-indigo-600 text-white"
              : "text-gray-300 hover:bg-gray-800"
          }`}
        >
          {t("allArtifacts")}
        </button>
        <button
          onClick={onMarketSelect}
          className={`w-full text-left px-3 py-2 rounded text-sm ${
            activeView === "market"
              ? "bg-indigo-600 text-white"
              : "text-gray-300 hover:bg-gray-800"
          }`}
        >
          {t("skillsMarket")}
        </button>
        {KINDS.map((k) =>
          k === "Skill" ? (
            <div key={k} className="space-y-1">
              <button
                onClick={() => onSkillCategorySelect("all")}
                className={`flex w-full items-center justify-between rounded px-3 py-2 text-left text-sm ${
                  activeView === "library" &&
                  selectedKind === "Skill" &&
                  selectedSkillCategory === "all"
                    ? "bg-indigo-600 text-white"
                    : "text-gray-300 hover:bg-gray-800"
                }`}
              >
                <span>{ta(`kindPlural.${k}`)}</span>
                <span className="text-xs opacity-70">
                  {skillCategoryCounts.all}
                </span>
              </button>
              {skillCategoryCounts.all > 0 && (
                <div className="ml-3 space-y-0.5 border-l border-gray-800 pl-2">
                  {SKILL_CATEGORY_MENU_IDS.map((category) => (
                    <button
                      key={category}
                      onClick={() => onSkillCategorySelect(category)}
                      className={`flex w-full items-center justify-between rounded px-2 py-1.5 text-left text-xs ${
                        activeView === "library" &&
                        selectedKind === "Skill" &&
                        selectedSkillCategory === category
                          ? "bg-gray-800 text-gray-100"
                          : "text-gray-500 hover:bg-gray-800/70 hover:text-gray-300"
                      }`}
                    >
                      <span className="truncate">
                        {t(`skillCategory.${category}`)}
                      </span>
                      <span className="ml-2 flex-none tabular-nums opacity-70">
                        {skillCategoryCounts[category]}
                      </span>
                    </button>
                  ))}
                </div>
              )}
            </div>
          ) : (
            <button
              key={k}
              onClick={() => onKindSelect(k)}
              className={`w-full text-left px-3 py-2 rounded text-sm ${
                activeView === "library" && selectedKind === k
                  ? "bg-indigo-600 text-white"
                  : "text-gray-300 hover:bg-gray-800"
              }`}
            >
              {ta(`kindPlural.${k}`)}
            </button>
          )
        )}
      </nav>

      <div className="px-2 py-3 border-t border-gray-700 space-y-3">
        <div className="px-3">
          <p className="text-xs font-medium text-gray-500 uppercase tracking-wider mb-2">
            {t("tools")}
          </p>
          <ul className="space-y-1">
            {adapters.map((a) => (
              <li key={a.adapterId} className="flex items-center gap-2 text-xs">
                <span
                  className={`h-2 w-2 rounded-full flex-none ${
                    a.presence.type === "available"
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
          {t("import")}
        </button>
      </div>
    </aside>
  );
}
