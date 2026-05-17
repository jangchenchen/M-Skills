import { invoke } from "@tauri-apps/api/core";
import type {
  ImportPreviewDto,
  InstallationDto,
  InventoryDto,
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
  target: TargetDto
): Promise<InstallationDto> {
  return invoke("install", { candidateIndex, target });
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
