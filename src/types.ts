export type ArtifactKind = "Skill" | "Extension" | "Workflow";

export type ScopeDto =
  | { type: "Global" }
  | { type: "Project"; path: string };

export interface TargetDto {
  tool: string;
  scope: ScopeDto;
}

export type SourceDto =
  | { type: "GitHub"; url: string; rev: string }
  | { type: "Local"; path: string }
  | { type: "Bundled" }
  | { type: "Unknown" };

export interface ArtifactDto {
  id: string;
  name: string;
  description: string;
  version: string | null;
  kind: ArtifactKind;
  source: SourceDto;
}

export interface InstallationDto {
  id: string;
  artifactId: string;
  target: TargetDto;
  status: string;
  onDiskPath: string;
  installedAt: string;
  installedVersion: string | null;
}

export interface ScannedInstallationDto {
  artifact: ArtifactDto;
  installation: InstallationDto;
  provenance: string;
}

export interface ArtifactGroupDto {
  name: string;
  kind: ArtifactKind;
  description: string;
  version: string | null;
  installations: ScannedInstallationDto[];
  alsoVisibleTo: string[];
}

export type PresenceDto =
  | { type: "Available" }
  | { type: "Missing"; reason: string };

export interface AdapterStatusDto {
  adapterId: string;
  presence: PresenceDto;
}

export interface InventoryDto {
  groups: ArtifactGroupDto[];
  adapters: AdapterStatusDto[];
  errors: string[];
}

export interface AuditFileDto {
  path: string;
  sizeBytes: number;
}

export interface AuditMetadataDto {
  path: string;
  fields: Record<string, string>;
}

export interface AuditWarningDto {
  path: string;
  kind: "ExecutableCommand" | "McpConfig";
  message: string;
}

export interface ImportAuditDto {
  files: AuditFileDto[];
  metadata: AuditMetadataDto[];
  warnings: AuditWarningDto[];
}

export type ImportSourceDto =
  | { type: "Local"; path: string }
  | { type: "GitHub"; url: string };

export interface ImportCandidateDto {
  index: number;
  artifact: ArtifactDto;
  compatibleTargets: TargetDto[];
}

export interface ImportPreviewDto {
  source: ImportSourceDto;
  commitSha: string | null;
  candidates: ImportCandidateDto[];
  audit: ImportAuditDto;
}

export function targetLabel(t: TargetDto): string {
  const scope =
    t.scope.type === "Project" ? ` (project)` : "";
  return `${t.tool}${scope}`;
}

export function sourceLabel(s: SourceDto): string {
  if (s.type === "GitHub") return s.url;
  if (s.type === "Local") return s.path;
  return s.type;
}
