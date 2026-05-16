import { invoke } from "@tauri-apps/api/core";
import type {
  ImportPreviewDto,
  InstallationDto,
  InventoryDto,
  TargetDto,
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
