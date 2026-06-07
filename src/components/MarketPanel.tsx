import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { searchMarketSkills, previewMarketSkill } from "../api";
import type {
  ImportPreviewDto,
  MarketProviderId,
  MarketSkillCandidateDto,
  MarketProviderErrorDto,
  MarketSearchResultDto,
} from "../types";
import { useErrorMessage } from "../useErrorMessage";

type ProviderFilter = "all" | MarketProviderId;

const PROVIDER_FILTERS: ProviderFilter[] = [
  "all",
  "skillsmd",
  "agent-skills-index",
];

function providerIds(filter: ProviderFilter): MarketProviderId[] {
  if (filter === "all") return ["skillsmd", "agent-skills-index"];
  return [filter];
}

interface Props {
  onImportClick: () => void;
  onMarketPreview: (preview: ImportPreviewDto) => void;
}

export function MarketPanel({ onImportClick, onMarketPreview }: Props) {
  const { t } = useTranslation("common");
  const errorMessage = useErrorMessage();
  const [query, setQuery] = useState("");
  const [activeFilter, setActiveFilter] = useState<ProviderFilter>("all");
  const [searchResult, setSearchResult] =
    useState<MarketSearchResultDto | null>(null);
  const [loading, setLoading] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [previewLoading, setPreviewLoading] = useState<string | null>(null);

  async function handleSearch() {
    const trimmed = query.trim();
    if (!trimmed) return;
    setLoading(true);
    setSearchError(null);
    try {
      const result = await searchMarketSkills({
        query: trimmed,
        providers: providerIds(activeFilter),
      });
      setSearchResult(result);
    } catch (err) {
      setSearchError(errorMessage(err));
    } finally {
      setLoading(false);
    }
  }

  async function handlePreview(candidate: MarketSkillCandidateDto) {
    setPreviewLoading(candidate.externalId);
    try {
      const preview = await previewMarketSkill({
        providerId: candidate.providerId,
        externalId: candidate.externalId,
      });
      onMarketPreview(preview);
    } catch (err) {
      setSearchError(errorMessage(err));
    } finally {
      setPreviewLoading(null);
    }
  }

  function handleKeyDown(e: React.KeyboardEvent) {
    if (e.key === "Enter") handleSearch();
  }

  const results = searchResult?.results ?? [];
  const providerErrors = searchResult?.providerErrors ?? [];
  const hasSearched = searchResult !== null;

  return (
    <div className="flex-1 overflow-y-auto px-6 py-5">
      <div className="max-w-4xl space-y-5">
        {/* Header */}
        <header className="flex flex-wrap items-start justify-between gap-4">
          <div>
            <p className="text-xs font-medium uppercase tracking-wider text-gray-500">
              {t("market.kicker")}
            </p>
            <h2 className="mt-1 text-base font-semibold text-gray-100">
              {t("market.title")}
            </h2>
            <p className="mt-2 max-w-3xl text-sm leading-6 text-gray-400">
              {t("market.subtitle")}
            </p>
          </div>
          <button
            type="button"
            onClick={onImportClick}
            className="rounded bg-indigo-600 px-3 py-2 text-sm text-white hover:bg-indigo-500"
          >
            {t("market.importAction")}
          </button>
        </header>

        {/* Provider tabs */}
        <div className="flex gap-1 rounded-lg bg-gray-900 p-1">
          {PROVIDER_FILTERS.map((filter) => (
            <button
              key={filter}
              type="button"
              onClick={() => setActiveFilter(filter)}
              className={`rounded-md px-3 py-1.5 text-xs font-medium transition-colors ${
                activeFilter === filter
                  ? "bg-gray-700 text-white"
                  : "text-gray-400 hover:text-gray-200"
              }`}
            >
              {t(`market.providerTab.${filter}`)}
            </button>
          ))}
        </div>

        {/* Search bar */}
        <div className="flex gap-2">
          <input
            type="text"
            value={query}
            onChange={(e) => setQuery(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder={t("market.searchPlaceholder")}
            className="flex-1 rounded-md border border-gray-700 bg-gray-900 px-3 py-2 text-sm text-gray-100 placeholder-gray-500 focus:border-indigo-500 focus:outline-none focus:ring-1 focus:ring-indigo-500"
          />
          <button
            type="button"
            onClick={handleSearch}
            disabled={loading || !query.trim()}
            className="rounded-md bg-indigo-600 px-4 py-2 text-sm font-medium text-white hover:bg-indigo-500 disabled:opacity-50"
          >
            {loading ? t("market.searching") : t("market.searchButton")}
          </button>
        </div>

        {/* Global search error */}
        {searchError && (
          <div className="rounded-md border border-red-800 bg-red-900/30 px-4 py-3 text-sm text-red-300">
            {searchError}
          </div>
        )}

        {/* Provider errors */}
        {providerErrors.map((err) => (
          <ProviderErrorBanner
            key={err.providerId}
            error={err}
            onRetry={handleSearch}
          />
        ))}

        {/* Cache indicator */}
        {searchResult?.cached && !loading && results.length > 0 && (
          <div className="flex items-center gap-2 text-xs text-gray-500">
            <span className="inline-block h-1.5 w-1.5 rounded-full bg-gray-500" />
            {t("market.cachedResults")}
          </div>
        )}

        {/* Results */}
        {loading && (
          <p className="py-8 text-center text-sm text-gray-500">
            {t("market.searching")}
          </p>
        )}

        {!loading && hasSearched && results.length === 0 && (
          <p className="py-8 text-center text-sm text-gray-500">
            {t("market.noResults")}
          </p>
        )}

        {!loading && results.length > 0 && (
          <div className="space-y-3">
            {results.map((candidate) => (
              <CandidateCard
                key={`${candidate.providerId}:${candidate.externalId}`}
                candidate={candidate}
                onPreview={() => handlePreview(candidate)}
                isLoading={previewLoading === candidate.externalId}
              />
            ))}
          </div>
        )}

        {/* Info section (shown when no search yet) */}
        {!hasSearched && !loading && (
          <section className="border-l-4 border-gray-700 bg-gray-900/50 px-4 py-3">
            <p className="text-sm font-medium text-gray-300">
              {t("market.decisionTitle")}
            </p>
            <ul className="mt-2 space-y-1 text-sm leading-6 text-gray-400">
              {[0, 1, 2].map((index) => (
                <li key={index}>{t(`market.decision.${index}`)}</li>
              ))}
            </ul>
          </section>
        )}
      </div>
    </div>
  );
}

function CandidateCard({
  candidate,
  onPreview,
  isLoading,
}: {
  candidate: MarketSkillCandidateDto;
  onPreview: () => void;
  isLoading: boolean;
}) {
  const { t } = useTranslation("common");

  return (
    <article className="rounded-lg border border-gray-800 bg-gray-900 px-4 py-3">
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <h3 className="truncate text-sm font-semibold text-gray-100">
              {candidate.name}
            </h3>
            {candidate.stars != null && (
              <span className="shrink-0 text-xs text-amber-400">
                {t("market.stars", { count: candidate.stars })}
              </span>
            )}
          </div>
          {candidate.description && (
            <p className="mt-1 line-clamp-2 text-sm text-gray-400">
              {candidate.description}
            </p>
          )}
        </div>
        <button
          type="button"
          onClick={onPreview}
          disabled={isLoading}
          className="shrink-0 rounded bg-indigo-600 px-3 py-1.5 text-xs font-medium text-white hover:bg-indigo-500 disabled:opacity-50"
        >
          {isLoading ? t("market.previewing") : t("market.previewButton")}
        </button>
      </div>

      <div className="mt-2 flex flex-wrap items-center gap-2">
        <ProviderBadge providerId={candidate.providerId} />
        {candidate.hasSkillMd ? (
          <span className="rounded-full bg-emerald-400/10 px-2 py-0.5 text-xs font-medium text-emerald-300 ring-1 ring-emerald-400/30">
            {t("market.hasSkillMd")}
          </span>
        ) : (
          <span className="rounded-full bg-amber-400/10 px-2 py-0.5 text-xs font-medium text-amber-300 ring-1 ring-amber-400/30">
            {t("market.noSkillMd")}
          </span>
        )}
        {candidate.categories.map((cat) => (
          <span
            key={cat}
            className="rounded-full bg-gray-800 px-2 py-0.5 text-xs text-gray-400"
          >
            {cat}
          </span>
        ))}
      </div>

      {candidate.repoUrl && (
        <p className="mt-1.5 truncate text-xs text-gray-500">
          {candidate.externalId}
        </p>
      )}
    </article>
  );
}

function ProviderBadge({ providerId }: { providerId: string }) {
  const { t } = useTranslation("common");
  const colors =
    providerId === "skillsmd"
      ? "bg-blue-400/10 text-blue-300 ring-blue-400/30"
      : "bg-purple-400/10 text-purple-300 ring-purple-400/30";
  return (
    <span
      className={`rounded-full px-2 py-0.5 text-xs font-medium ring-1 ${colors}`}
    >
      {t(`market.providerBadge.${providerId}`)}
    </span>
  );
}

function ProviderErrorBanner({
  error,
  onRetry,
}: {
  error: MarketProviderErrorDto;
  onRetry: () => void;
}) {
  const { t } = useTranslation("common");
  const providerLabel = t(`market.providerBadge.${error.providerId}`);

  if (error.isRateLimited) {
    return (
      <RateLimitBanner
        providerLabel={providerLabel}
        retryAfterSecs={error.retryAfterSecs ?? 30}
        onRetry={onRetry}
      />
    );
  }

  return (
    <div className="rounded-md border border-amber-800 bg-amber-900/20 px-4 py-2 text-sm text-amber-300">
      {t("market.providerError", {
        provider: providerLabel,
        message: error.message,
      })}
    </div>
  );
}

function RateLimitBanner({
  providerLabel,
  retryAfterSecs,
  onRetry,
}: {
  providerLabel: string;
  retryAfterSecs: number;
  onRetry: () => void;
}) {
  const { t } = useTranslation("common");
  const [remaining, setRemaining] = useState(retryAfterSecs);
  const timerRef = useRef<ReturnType<typeof setInterval> | null>(null);

  useEffect(() => {
    setRemaining(retryAfterSecs);
    timerRef.current = setInterval(() => {
      setRemaining((prev) => {
        if (prev <= 1) {
          if (timerRef.current) clearInterval(timerRef.current);
          return 0;
        }
        return prev - 1;
      });
    }, 1000);
    return () => {
      if (timerRef.current) clearInterval(timerRef.current);
    };
  }, [retryAfterSecs]);

  return (
    <div className="flex items-center justify-between rounded-md border border-amber-800 bg-amber-900/20 px-4 py-2">
      <span className="text-sm text-amber-300">
        {remaining > 0
          ? t("market.rateLimitedCountdown", {
              provider: providerLabel,
              seconds: remaining,
            })
          : t("market.rateLimitedReady", { provider: providerLabel })}
      </span>
      {remaining === 0 && (
        <button
          type="button"
          onClick={onRetry}
          className="rounded bg-amber-600 px-3 py-1 text-xs font-medium text-white hover:bg-amber-500"
        >
          {t("market.retryButton")}
        </button>
      )}
    </div>
  );
}
