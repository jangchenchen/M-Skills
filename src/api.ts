import { invoke } from "@tauri-apps/api/core";
import type {
  ArtifactDto,
  CompatibilityReviewDto,
  ConfirmDraftInstallRequest,
  ForkPreviewRequest,
  ImportPreviewDto,
  InstallOutcomeDto,
  InstallationDto,
  InventoryDto,
  ReviewOutcomeDto,
  RewriteSkillOutcomeDto,
  RewriteSkillRequest,
  SaveCustomSkillEditRequest,
  SkillDraftPreviewDto,
  SkillSummaryDto,
  SkillSummaryRequest,
  TargetDto,
  TranslateConfigDto,
  TranslateOutcomeDto,
} from "./types";

export function scan(cwd?: string): Promise<InventoryDto> {
  return invoke("scan", { cwd: cwd ?? null });
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
