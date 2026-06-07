export type ArtifactKind = "Skill" | "Extension" | "Workflow";

export interface ErrorDto {
  code: string;
  params: Record<string, string>;
}

export type ScopeDto =
  | { type: "global" }
  | { type: "project"; path: string };

export interface TargetDto {
  tool: string;
  scope: ScopeDto;
}

export type SourceDto =
  | { type: "gitHub"; url: string; rev: string }
  | { type: "url"; url: string }
  | { type: "local"; path: string }
  | { type: "bundled" }
  | { type: "unknown" };

export interface ArtifactDto {
  id: string;
  name: string;
  description: string;
  body: string | null;
  version: string | null;
  kind: ArtifactKind;
  source: SourceDto;
  searchAliases: string[];
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
  searchAliases: string[];
  capabilities: CapabilityDto[];
  installations: ScannedInstallationDto[];
  alsoVisibleTo: string[];
}

export type PresenceDto =
  | { type: "available" }
  | { type: "missing"; reason: string };

export interface AdapterStatusDto {
  adapterId: string;
  presence: PresenceDto;
  supportedKinds: ArtifactKind[];
  writable: boolean;
  supportsDisable: boolean;
}

// ── Dashboard (Issue 014) ─────────────────────────────────────────────────────

export type DashboardMetricSeverity = "ok" | "info" | "warning" | "critical";

export interface DashboardKindSummaryDto {
  kind: ArtifactKind;
  groups: number;
  ownedInstallations: number;
  visibleInstallations: number;
  compatibleTargets: TargetDto[];
}

export interface DashboardToolSummaryDto {
  adapterId: string;
  available: boolean;
  writable: boolean;
  supportedKinds: ArtifactKind[];
  ownedInstallations: number;
  visibleInstallations: number;
  missingReason?: string;
}

export interface DashboardAttentionItemDto {
  severity: DashboardMetricSeverity;
  title: string;
  body: string;
  action:
    | "open_settings"
    | "open_import"
    | "rescan"
    | "filter_kind"
    | "filter_tool"
    | "select_artifact"
    | "none";
  kind?: ArtifactKind;
  artifactName?: string;
  tool?: string;
}

export interface RecentActionDto {
  eventType: string;
  artifactName?: string;
  target?: string;
  occurredAt: string;
  succeeded: boolean;
}

export interface DashboardDto {
  generatedAt: string;
  readyArtifactGroups: number;
  totalGroups: number;
  totalOwnedInstallations: number;
  scanErrors: string[];
  kindSummaries: DashboardKindSummaryDto[];
  toolSummaries: DashboardToolSummaryDto[];
  attentionItems: DashboardAttentionItemDto[];
  recentActions: RecentActionDto[];
  registryStaleCount: number;
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
  detail?: string;
  detailKey?: string;
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
  | { type: "local"; path: string }
  | { type: "gitHub"; url: string }
  | { type: "rawUrl"; url: string };

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
    t.scope.type === "project" ? ` (project)` : "";
  return `${t.tool}${scope}`;
}

export function sourceLabel(s: SourceDto): string {
  if (s.type === "gitHub") return s.url;
  if (s.type === "url") return s.url;
  if (s.type === "local") return s.path;
  return s.type;
}

// ── Issue 007 Batch 2: skill draft preview ────────────────────────────────────

export type SkillDraftSourceKind = "fork" | "adaptation" | "edit";

/// Union across draft + import provenance. The shared `LineageDto` (Issue 016)
/// is written by both the draft path and the import/market install path.
export type LineageSourceKind =
  | SkillDraftSourceKind
  | "market"
  | "github"
  | "url"
  | "local";

export interface LineageDto {
  sourceKind: LineageSourceKind;
  providerId?: string;
  externalId?: string;
  sourceTool?: string;
  sourcePath?: string;
  sourceUrl?: string;
  sourceRev?: string;
  sourceHash?: string;
  parentName?: string;
  fetchedAt?: string;
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
  audit: ImportAuditDto;
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
  audit: ImportAuditDto;
}

// ── Issue 011: Smart Add natural-language intent ─────────────────────────────

export interface SkillIntentOutcomeDto {
  isInstallRequest: boolean;
  searchQuery: string | null;
  reason: string | null;
  providerKind: string;
  model: string;
}

// ── GitHub skill search ──────────────────────────────────────────────────────

export interface GitHubSkillResultDto {
  name: string;
  owner: string;
  description: string | null;
  htmlUrl: string;
  stars: number;
}

// ── Market search ───────────────────────────────────────────────────────────

export type MarketProviderId = "skillsmd" | "agent-skills-index";

export interface MarketSearchRequest {
  query: string;
  providers: MarketProviderId[];
}

export interface MarketSkillCandidateDto {
  providerId: string;
  externalId: string;
  name: string;
  description: string | null;
  repoUrl: string | null;
  stars: number | null;
  updatedAt: string | null;
  categories: string[];
  hasSkillMd: boolean;
  providerIds?: string[];
  sourceCount?: number;
}

export interface MarketProviderErrorDto {
  providerId: string;
  message: string;
  isRateLimited: boolean;
  retryAfterSecs: number | null;
}

export interface MarketSearchResultDto {
  query: string;
  results: MarketSkillCandidateDto[];
  providerErrors: MarketProviderErrorDto[];
  cached: boolean;
}

export interface MarketPreviewRequest {
  providerId: string;
  externalId: string;
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

// ── Telemetry ────────────────────────────────────────────────────────────────

export interface TelemetryDto {
  periodLabel: string;
  scanCount: number;
  installCount: number;
  uninstallCount: number;
  adaptationCount: number;
  failureCount: number;
  topFailureReasons: { reason: string; count: number }[];
  targetDistribution: { target: string; count: number }[];
  riskDistribution: { riskLevel: string; count: number }[];
}

// ── Update Detection + Rollback ──────────────────────────────────────────────

export type UpdateStatus =
  | "upToDate"
  | "updateAvailable"
  | "locallyModified"
  | "diverged"
  | "sourceUnreachable"
  | "noSource";

export interface UpdateStatusDto {
  status: UpdateStatus;
  currentContentSha256?: string;
  upstreamRev?: string;
  storedRev?: string;
  snapshotCount: number;
}

export interface SnapshotDto {
  id: string;
  installationId: string;
  contentSha256: string;
  reason: string;
  createdAt: string;
}
