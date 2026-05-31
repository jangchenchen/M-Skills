import type { ArtifactGroupDto } from "./types";

export type SkillCategoryId =
  | "all"
  | "testing"
  | "engineering"
  | "product"
  | "design"
  | "data"
  | "marketing"
  | "security"
  | "tools"
  | "other";

export type PrimarySkillCategoryId = Exclude<SkillCategoryId, "all">;

export type SkillCategoryCounts = Record<SkillCategoryId, number>;

export const SKILL_CATEGORY_MENU_IDS: PrimarySkillCategoryId[] = [
  "testing",
  "engineering",
  "product",
  "design",
  "data",
  "marketing",
  "security",
  "tools",
  "other",
];

const CATEGORY_KEYWORDS: Record<PrimarySkillCategoryId, string[]> = {
  testing: [
    "test scenario",
    "test case",
    "unit test",
    "integration test",
    "e2e",
    "playwright",
    "vitest",
    "jest",
    "pytest",
    "junit",
    "qa",
    "quality",
    "regression",
    "tdd",
    "测试",
    "用例",
    "质量",
    "回归",
    "验收",
  ],
  engineering: [
    "code",
    "coding",
    "architecture",
    "refactor",
    "debug",
    "diagnose",
    "repository",
    "github",
    "git",
    "api",
    "android",
    "reverse engineering",
    "database",
    "sql",
    "developer",
    "工程",
    "代码",
    "架构",
    "调试",
    "仓库",
  ],
  product: [
    "product",
    "prd",
    "roadmap",
    "prioritization",
    "prioritize",
    "backlog",
    "okr",
    "user story",
    "user stories",
    "persona",
    "customer journey",
    "opportunity",
    "job stories",
    "assumption",
    "sprint",
    "feature request",
    "产品",
    "需求",
    "路线图",
    "用户故事",
    "优先级",
    "画像",
  ],
  design: [
    "design",
    "ui",
    "ux",
    "prototype",
    "frontend",
    "front end",
    "animation",
    "motion",
    "gsap",
    "layout",
    "typography",
    "color",
    "visual",
    "impeccable",
    "设计",
    "原型",
    "界面",
    "交互",
    "动画",
    "视觉",
  ],
  data: [
    "data",
    "analytics",
    "analysis",
    "metric",
    "dashboard",
    "cohort",
    "a b test",
    "ab test",
    "a/b",
    "experiment",
    "spreadsheet",
    "dataset",
    "bayesian",
    "sql",
    "数据",
    "分析",
    "指标",
    "实验",
    "表格",
  ],
  marketing: [
    "marketing",
    "growth",
    "gtm",
    "go to market",
    "sales",
    "pricing",
    "positioning",
    "competitor",
    "competitive",
    "battlecard",
    "market",
    "segment",
    "swot",
    "brand",
    "monetization",
    "营销",
    "增长",
    "销售",
    "定价",
    "定位",
    "竞品",
    "市场",
  ],
  security: [
    "security",
    "privacy",
    "policy",
    "audit",
    "risk",
    "compliance",
    "nda",
    "prompt injection",
    "安全",
    "隐私",
    "审计",
    "风险",
    "合规",
  ],
  tools: [
    "utility",
    "helper",
    "browser",
    "chrome",
    "computer use",
    "document",
    "presentation",
    "slides",
    "image",
    "diagram",
    "mindmap",
    "network",
    "infographic",
    "installer",
    "creator",
    "generate",
    "workflow",
    "automation",
    "工具",
    "浏览器",
    "文档",
    "幻灯片",
    "图片",
    "图表",
    "自动化",
  ],
  other: [],
};

export function buildSkillCategoryCounts(
  groups: ArtifactGroupDto[]
): SkillCategoryCounts {
  const counts = emptySkillCategoryCounts();

  for (const group of groups) {
    if (group.kind !== "Skill") continue;
    counts.all += 1;
    counts[classifySkillCategory(group)] += 1;
  }

  return counts;
}

export function matchesSkillCategory(
  group: ArtifactGroupDto,
  category: SkillCategoryId
): boolean {
  if (category === "all") return true;
  return group.kind === "Skill" && classifySkillCategory(group) === category;
}

export function classifySkillCategory(
  group: ArtifactGroupDto
): PrimarySkillCategoryId {
  if (group.kind !== "Skill") return "other";

  const haystack = skillCategoryText(group);
  let best: PrimarySkillCategoryId = "other";
  let bestScore = 0;

  for (const category of SKILL_CATEGORY_MENU_IDS) {
    if (category === "other") continue;
    const score = scoreCategory(haystack, CATEGORY_KEYWORDS[category]);
    if (score > bestScore) {
      best = category;
      bestScore = score;
    }
  }

  return bestScore > 0 ? best : "other";
}

function emptySkillCategoryCounts(): SkillCategoryCounts {
  return {
    all: 0,
    testing: 0,
    engineering: 0,
    product: 0,
    design: 0,
    data: 0,
    marketing: 0,
    security: 0,
    tools: 0,
    other: 0,
  };
}

function scoreCategory(haystack: string, keywords: string[]): number {
  return keywords.reduce((score, keyword) => {
    const normalized = normalizeSkillText([keyword]);
    if (!normalized || !haystack.includes(normalized)) return score;
    return score + (normalized.includes(" ") ? 3 : 1);
  }, 0);
}

function skillCategoryText(group: ArtifactGroupDto): string {
  return normalizeSkillText([
    group.name,
    group.description,
    group.body?.slice(0, 3000) ?? "",
    ...group.searchAliases,
    ...group.capabilities.flatMap((capability) => [
      capability.name,
      capability.description,
    ]),
    ...group.installations.flatMap((installation) => [
      installation.artifact.name,
      installation.artifact.description,
      installation.artifact.body?.slice(0, 1500) ?? "",
      ...installation.artifact.searchAliases,
      ...installation.artifact.capabilities.flatMap((capability) => [
        capability.name,
        capability.description,
      ]),
    ]),
  ]);
}

function normalizeSkillText(parts: string[]): string {
  return parts
    .join(" ")
    .toLowerCase()
    .replace(/[-_/]+/g, " ")
    .replace(/\s+/g, " ")
    .trim();
}
