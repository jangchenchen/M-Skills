import { invoke } from "@tauri-apps/api/core";
import type {
  ArtifactDto,
  CompatibilityReviewDto,
  ConfirmDraftInstallRequest,
  DashboardDto,
  ForkPreviewRequest,
  GitHubSkillResultDto,
  ImportPreviewDto,
  InstallOutcomeDto,
  InstallationDto,
  InventoryDto,
  MarketPreviewRequest,
  MarketSearchRequest,
  MarketSearchResultDto,
  ReviewOutcomeDto,
  RewriteSkillOutcomeDto,
  RewriteSkillRequest,
  SaveCustomSkillEditRequest,
  SkillIntentOutcomeDto,
  SkillDraftPreviewDto,
  SkillSummaryDto,
  SkillSummaryRequest,
  SnapshotDto,
  TargetDto,
  TelemetryDto,
  TranslateConfigDto,
  TranslateOutcomeDto,
  UpdateStatusDto,
} from "./types";

export function scan(cwd?: string): Promise<InventoryDto> {
  return invoke("scan", { cwd: cwd ?? null });
}

export function getDashboard(cwd?: string): Promise<DashboardDto> {
  return invoke("get_dashboard", { cwd: cwd ?? null });
}

export function previewImport(pathOrUrl: string): Promise<ImportPreviewDto> {
  return invoke("preview_import", { pathOrUrl });
}

export function install(
  candidateIndex: number,
  targets: TargetDto[]
): Promise<InstallOutcomeDto[]> {
  return invoke("install", { candidateIndex, targets });
}

export function reviewImport(
  candidateIndex: number,
  locale: string | null
): Promise<ReviewOutcomeDto> {
  return invoke("review_import", { candidateIndex, locale });
}

export function checkPathExists(path: string): Promise<boolean> {
  return invoke("check_path_exists", { path });
}

export function classifySkillRequest(
  input: string,
  locale: string | null
): Promise<SkillIntentOutcomeDto> {
  return invoke("classify_skill_request", { input, locale });
}

export function searchGithubSkills(
  query: string
): Promise<GitHubSkillResultDto[]> {
  return invoke("search_github_skills", { query });
}

// ── Market search ───────────────────────────────────────────────────────────

export function searchMarketSkills(
  request: MarketSearchRequest
): Promise<MarketSearchResultDto> {
  return invoke("search_market_skills", { request });
}

export function previewMarketSkill(
  request: MarketPreviewRequest
): Promise<ImportPreviewDto> {
  return invoke("preview_market_skill", { request });
}

export function reviewArtifactCompatibility(
  artifact: ArtifactDto,
  targets: TargetDto[]
): Promise<CompatibilityReviewDto[]> {
  return invoke("review_artifact_compatibility", { artifact, targets });
}

export function previewAdaptSkillForCodex(
  artifact: ArtifactDto
): Promise<SkillDraftPreviewDto> {
  return invoke("preview_adapt_skill_for_codex", { artifact });
}

export function previewForkSkill(
  request: ForkPreviewRequest
): Promise<SkillDraftPreviewDto> {
  return invoke("preview_fork_skill", { request });
}

export function saveCustomSkillEdit(
  request: SaveCustomSkillEditRequest
): Promise<SkillDraftPreviewDto> {
  return invoke("save_custom_skill_edit", { request });
}

export function confirmInstallSkillDraft(
  request: ConfirmDraftInstallRequest
): Promise<InstallationDto> {
  return invoke("confirm_install_skill_draft", { request });
}

export function rewriteSkillWithLlm(
  request: RewriteSkillRequest
): Promise<RewriteSkillOutcomeDto> {
  return invoke("rewrite_skill_with_llm", { request });
}

export function getSkillSummary(
  artifact: ArtifactDto,
  locale: string
): Promise<SkillSummaryDto | null> {
  return invoke("get_skill_summary", { artifact, locale });
}

export function generateSkillSummary(
  request: SkillSummaryRequest
): Promise<SkillSummaryDto> {
  return invoke("generate_skill_summary", { request });
}

export function uninstall(installation: InstallationDto): Promise<void> {
  return invoke("uninstall", { installation });
}

export function enable(installation: InstallationDto): Promise<void> {
  return invoke("enable", { installation });
}

export function disable(installation: InstallationDto): Promise<void> {
  return invoke("disable", { installation });
}

export function translateArtifact(input: {
  artifactName: string;
  filePath: string;
  field: string;
  sourceText: string;
  locale: string;
  forceRefresh?: boolean;
}): Promise<TranslateOutcomeDto> {
  return invoke("translate_artifact", input);
}

export function clearTranslationCache(input: {
  artifactName: string;
  filePath: string;
  field: string;
  locale: string;
}): Promise<number> {
  return invoke("clear_translation_cache", input);
}

export function getTranslateConfig(): Promise<TranslateConfigDto> {
  return invoke("get_translate_config");
}

export function setTranslateConfig(
  config: TranslateConfigDto,
  apiKey: string | null
): Promise<TranslateConfigDto> {
  return invoke("set_translate_config", { config, apiKey });
}

export function testTranslateProvider(
  config: TranslateConfigDto,
  apiKey: string | null
): Promise<string> {
  return invoke("test_translate_provider", { config, apiKey });
}

// ── Telemetry ────────────────────────────────────────────────────────────────

export function getTelemetry(period?: string): Promise<TelemetryDto> {
  return invoke("get_telemetry", { period: period ?? null });
}

// ── Update Detection + Rollback ──────────────────────────────────────────────

export function checkForUpdates(
  installation: InstallationDto
): Promise<UpdateStatusDto> {
  return invoke("check_for_updates", { installation });
}

export function listSnapshots(
  installation: InstallationDto
): Promise<SnapshotDto[]> {
  return invoke("list_snapshots", { installation });
}

export function confirmRollback(
  installation: InstallationDto,
  snapshotId?: string
): Promise<InstallationDto> {
  return invoke("confirm_rollback", {
    installation,
    snapshotId: snapshotId ?? null,
  });
}

// ── Cross-tool Adaptation ────────────────────────────────────────────────────

export function previewAdaptSkill(
  artifact: ArtifactDto,
  targetTool: string
): Promise<SkillDraftPreviewDto> {
  return invoke("preview_adapt_skill", { artifact, targetTool });
}
