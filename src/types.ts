export type ArtifactKind = "Skill" | "Extension" | "Workflow";

export interface ErrorDto {
  code: string;
  params: Record<string, string>;
}

export type ScopeDto =
  | { type: "Global" }
  | { type: "Project"; path: string };

export interface TargetDto {
  tool: string;
  scope: ScopeDto;
}

export type SourceDto =
  | { type: "GitHub"; url: string; rev: string }
  | { type: "Url"; url: string }
  | { type: "Local"; path: string }
  | { type: "Bundled" }
  | { type: "Unknown" };

export interface ArtifactDto {
  id: string;
  name: string;
  description: string;
  body: string | null;
  version: string | null;
  kind: ArtifactKind;
  source: SourceDto;
  capabilities: CapabilityDto[];
  lineage?: LineageDto;
}

export interface CapabilityDto {
  name: string;
  description: string;
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
  body: string | null;
  version: string | null;
  capabilities: CapabilityDto[];
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
  kind:
    | "ExecutableCommand"
    | "McpConfig"
    | "DangerousShellPattern"
    | "PromptInjection"
    | "LargePayload";
  severity: AuditSeverity;
  message: string;
}

export type AuditSeverity = "low" | "medium" | "high";

export interface ImportAuditDto {
  files: AuditFileDto[];
  metadata: AuditMetadataDto[];
  warnings: AuditWarningDto[];
  riskLevel: AuditSeverity;
}

export interface InstallOutcomeDto {
  target: TargetDto;
  ok: boolean;
  installation: InstallationDto | null;
  error: ErrorDto | null;
}

export type ReviewRating = "safe" | "caution" | "conflict";

export type ReviewReasonKind =
  | "overlap"
  | "command_collision"
  | "behavior_conflict"
  | string;

export interface ReviewConflictDto {
  name: string;
  kind: string;
  tool: string;
  reasonKind: ReviewReasonKind;
  reason: string;
}

export interface ReviewOutcomeDto {
  rating: ReviewRating;
  summary: string;
  skillPurpose: string;
  conflicts: ReviewConflictDto[];
  providerKind: string;
  model: string;
}

export type ImportSourceDto =
  | { type: "Local"; path: string }
  | { type: "GitHub"; url: string }
  | { type: "RawUrl"; url: string };

export interface ImportCandidateDto {
  index: number;
  artifact: ArtifactDto;
  compatibleTargets: TargetDto[];
  compatibilityReviews: CompatibilityReviewDto[];
}

export type CompatibilityStatus = "compatible" | "warning" | "incompatible";
export type CompatibilityRiskLevel = "low" | "medium" | "high";

export interface CompatibilityReviewDto {
  target: TargetDto;
  status: CompatibilityStatus;
  riskLevel: CompatibilityRiskLevel;
  summary: string;
  reasons: string[];
  warnings: string[];
}

export interface ImportPreviewDto {
  source: ImportSourceDto;
  commitSha: string | null;
  candidates: ImportCandidateDto[];
  audit: ImportAuditDto;
}

export type TranslateProviderKind = "passthrough" | "openai-compat";

export type TranslateCacheStatus = "hit" | "miss" | "refreshed";

export type MarkdownWarningDto =
  | { kind: "fencedCodeBlockCount"; source: number; translated: number }
  | { kind: "linkCount"; source: number; translated: number }
  | { kind: "headingCount"; source: number; translated: number }
  | { kind: "listItemCount"; source: number; translated: number }
  | { kind: "codeBlockContentChanged"; index: number }
  | { kind: "frontmatterChanged" };

export interface TranslationValidationDto {
  ok: boolean;
  warnings: MarkdownWarningDto[];
}

export interface TranslateOutcomeDto {
  text: string;
  locale: string;
  field: string;
  sourceSha256: string;
  cacheStatus: TranslateCacheStatus;
  providerKind: TranslateProviderKind | string;
  usedFallback: boolean;
  validation: TranslationValidationDto;
}

export interface TranslateConfigDto {
  providerKind: TranslateProviderKind;
  baseUrl: string;
  model: string;
  fallbackModel: string | null;
  timeoutMs: number;
  maxRetries: number;
  apiKeyPresent: boolean;
}

export function targetLabel(t: TargetDto): string {
  const scope =
    t.scope.type === "Project" ? ` (project)` : "";
  return `${t.tool}${scope}`;
}

export function sourceLabel(s: SourceDto): string {
  if (s.type === "GitHub") return s.url;
  if (s.type === "Url") return s.url;
  if (s.type === "Local") return s.path;
  return s.type;
}

// ── Issue 007 Batch 2: skill draft preview ────────────────────────────────────

export type SkillDraftSourceKind = "fork" | "adaptation";

export interface LineageDto {
  sourceKind: SkillDraftSourceKind;
  sourceTool?: string;
  sourcePath?: string;
  sourceUrl?: string;
  sourceHash: string;
  parentName: string;
}

export interface NameConflictDto {
  existingPath: string;
  targetTool: string;
}

export interface SkillDraftPreviewDto {
  originalName: string;
  originalContent: string;
  adaptedName: string;
  adaptedDescription: string;
  adaptedVersion: string | null;
  adaptedContent: string;
  target: TargetDto;
  lineage: LineageDto;
  compatibilityReviews: CompatibilityReviewDto[];
  nameConflict?: NameConflictDto;
}

export interface ForkPreviewRequest {
  artifact: ArtifactDto;
  target: TargetDto;
}

export interface SaveCustomSkillEditRequest {
  content: string;
  target: TargetDto;
  lineage: LineageDto;
}

export interface ConfirmDraftInstallRequest {
  name: string;
  description: string;
  version: string | null;
  content: string;
  target: TargetDto;
  lineage: LineageDto;
}

export type SkillDraftMode = "adapt" | "fork" | "edit";

// ── Issue 007 Batch 3: LLM-assisted rewrite ───────────────────────────────────

export type RewriteMode =
  | "adapt_to_codex"
  | "complete_missing_info"
  | "reduce_risk"
  | "customize_workflow"
  | "simplify";

export interface RewriteSkillRequest {
  artifact: ArtifactDto;
  mode: RewriteMode;
  userInstruction: string;
  locale: string;
}

export interface RewriteSkillOutcomeDto {
  draftBody: string;
  summary: string;
  notes: string[];
  providerKind: string;
  model: string;
  compatibilityReviews: CompatibilityReviewDto[];
}

// ── AI skill summary (auto-generated post-install) ────────────────────────────

export interface SkillSummaryDto {
  commands: string[];
  capabilities: string;
  useCases: string[];
  examples: string[];
  locale: string;
  providerKind: string;
  model: string;
  generatedAt: string;
  cacheStatus: "hit" | "miss";
}

export interface SkillSummaryRequest {
  artifact: ArtifactDto;
  locale: string;
  forceRefresh?: boolean;
}
