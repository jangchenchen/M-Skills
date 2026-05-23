import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import {
  getTranslateConfig,
  setTranslateConfig,
  testTranslateProvider,
} from "../api";
import type { TranslateConfigDto, TranslateProviderKind } from "../types";
import { useErrorMessage } from "../useErrorMessage";

interface Props {
  onClose: () => void;
}

type TestState =
  | { kind: "idle" }
  | { kind: "running" }
  | { kind: "ok"; result: string }
  | { kind: "error"; message: string };

const DEFAULT_CONFIG: TranslateConfigDto = {
  providerKind: "passthrough",
  baseUrl: "https://api.deepseek.com/v1",
  model: "deepseek-chat",
  fallbackModel: null,
  timeoutMs: 30_000,
  maxRetries: 2,
  apiKeyPresent: false,
};

export function SettingsModal({ onClose }: Props) {
  const { t } = useTranslation("settings");
  const { t: tc } = useTranslation("common");
  const errorMessage = useErrorMessage();

  const [config, setConfig] = useState<TranslateConfigDto>(DEFAULT_CONFIG);
  const [apiKeyInput, setApiKeyInput] = useState("");
  const [clearKey, setClearKey] = useState(false);
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [test, setTest] = useState<TestState>({ kind: "idle" });
  const [saveError, setSaveError] = useState<string | null>(null);

  useEffect(() => {
    let alive = true;
    getTranslateConfig()
      .then((c) => {
        if (alive) setConfig(c);
      })
      .catch((e) => {
        if (alive) setSaveError(errorMessage(e));
      })
      .finally(() => {
        if (alive) setLoading(false);
      });
    return () => {
      alive = false;
    };
  }, []);

  function update<K extends keyof TranslateConfigDto>(
    key: K,
    value: TranslateConfigDto[K]
  ) {
    setConfig((prev) => ({ ...prev, [key]: value }));
    setTest({ kind: "idle" });
  }

  function apiKeyToSend(): string | null {
    if (clearKey) return "";
    if (apiKeyInput.length > 0) return apiKeyInput;
    return null;
  }

  async function handleTest() {
    setTest({ kind: "running" });
    try {
      const result = await testTranslateProvider(config, apiKeyToSend());
      setTest({ kind: "ok", result });
    } catch (e) {
      setTest({ kind: "error", message: errorMessage(e) });
    }
  }

  async function handleSave() {
    setSaving(true);
    setSaveError(null);
    try {
      await setTranslateConfig(config, apiKeyToSend());
      onClose();
    } catch (e) {
      setSaveError(errorMessage(e));
    } finally {
      setSaving(false);
    }
  }

  const isOpenAi = config.providerKind === "openai-compat";

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center bg-black/60"
      onClick={onClose}
    >
      <div
        className="w-full max-w-lg bg-gray-900 border border-gray-700 rounded-lg shadow-xl"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="px-5 py-4 border-b border-gray-800 flex items-center justify-between">
          <h2 className="text-base font-semibold text-gray-100">{t("title")}</h2>
          <button
            onClick={onClose}
            className="text-gray-500 hover:text-gray-300 text-lg leading-none"
            aria-label={t("cancel")}
          >
            ×
          </button>
        </div>

        <div className="px-5 py-4 space-y-4 max-h-[70vh] overflow-y-auto">
          <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-500">
            {t("translationSection")}
          </h3>

          {loading ? (
            <p className="text-sm text-gray-500">{tc("scanning")}</p>
          ) : (
            <>
              <Field label={t("provider")}>
                <select
                  className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1.5 text-sm text-gray-100"
                  value={config.providerKind}
                  onChange={(e) =>
                    update("providerKind", e.target.value as TranslateProviderKind)
                  }
                >
                  <option value="passthrough">{t("providerPassthrough")}</option>
                  <option value="openai-compat">{t("providerOpenAiCompat")}</option>
                </select>
              </Field>

              {isOpenAi && (
                <>
                  <Field label={t("baseUrl")} hint={t("baseUrlHint")}>
                    <input
                      type="text"
                      className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1.5 text-sm text-gray-100"
                      value={config.baseUrl}
                      onChange={(e) => update("baseUrl", e.target.value)}
                    />
                  </Field>

                  <Field label={t("model")}>
                    <input
                      type="text"
                      className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1.5 text-sm text-gray-100"
                      value={config.model}
                      onChange={(e) => update("model", e.target.value)}
                    />
                  </Field>

                  <Field label={t("fallbackModel")} hint={t("fallbackModelHint")}>
                    <input
                      type="text"
                      className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1.5 text-sm text-gray-100"
                      value={config.fallbackModel ?? ""}
                      onChange={(e) =>
                        update("fallbackModel", e.target.value || null)
                      }
                    />
                  </Field>

                  <Field label={t("apiKey")}>
                    <input
                      type="password"
                      className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1.5 text-sm text-gray-100"
                      value={apiKeyInput}
                      placeholder={
                        config.apiKeyPresent && !clearKey
                          ? t("apiKeyPlaceholderSet")
                          : t("apiKeyPlaceholderEmpty")
                      }
                      onChange={(e) => {
                        setApiKeyInput(e.target.value);
                        setClearKey(false);
                        setTest({ kind: "idle" });
                      }}
                    />
                    {config.apiKeyPresent && (
                      <label className="mt-1 flex items-center gap-2 text-xs text-gray-400">
                        <input
                          type="checkbox"
                          checked={clearKey}
                          onChange={(e) => {
                            setClearKey(e.target.checked);
                            if (e.target.checked) setApiKeyInput("");
                          }}
                        />
                        {t("apiKeyClear")}
                      </label>
                    )}
                  </Field>

                  <div className="grid grid-cols-2 gap-3">
                    <Field label={t("timeoutMs")}>
                      <input
                        type="number"
                        min={1000}
                        step={500}
                        className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1.5 text-sm text-gray-100"
                        value={config.timeoutMs}
                        onChange={(e) =>
                          update("timeoutMs", Number(e.target.value) || 0)
                        }
                      />
                    </Field>
                    <Field label={t("maxRetries")}>
                      <input
                        type="number"
                        min={0}
                        max={10}
                        className="w-full bg-gray-800 border border-gray-700 rounded px-2 py-1.5 text-sm text-gray-100"
                        value={config.maxRetries}
                        onChange={(e) =>
                          update("maxRetries", Number(e.target.value) || 0)
                        }
                      />
                    </Field>
                  </div>
                </>
              )}

              {test.kind === "ok" && (
                <p className="text-xs text-emerald-400 break-all">
                  {t("testSuccess", { result: test.result })}
                </p>
              )}
              {test.kind === "error" && (
                <p className="text-xs text-red-400 break-all">{test.message}</p>
              )}
              {saveError && (
                <p className="text-xs text-red-400 break-all">{saveError}</p>
              )}
            </>
          )}
        </div>

        <div className="px-5 py-3 border-t border-gray-800 flex items-center justify-end gap-2">
          {isOpenAi && (
            <button
              onClick={handleTest}
              disabled={loading || test.kind === "running" || saving}
              className="text-sm text-gray-300 hover:text-gray-100 px-3 py-1.5 rounded border border-gray-700 hover:border-gray-500 disabled:opacity-40"
            >
              {test.kind === "running" ? t("testing") : t("test")}
            </button>
          )}
          <button
            onClick={onClose}
            disabled={saving}
            className="text-sm text-gray-300 hover:text-gray-100 px-3 py-1.5 rounded border border-gray-700 hover:border-gray-500 disabled:opacity-40"
          >
            {t("cancel")}
          </button>
          <button
            onClick={handleSave}
            disabled={loading || saving}
            className="text-sm bg-indigo-600 hover:bg-indigo-500 text-white px-3 py-1.5 rounded disabled:opacity-40"
          >
            {saving ? t("saving") : t("save")}
          </button>
        </div>
      </div>
    </div>
  );
}

function Field({
  label,
  hint,
  children,
}: {
  label: string;
  hint?: string;
  children: React.ReactNode;
}) {
  return (
    <div>
      <label className="block text-xs font-medium text-gray-400 mb-1">
        {label}
      </label>
      {children}
      {hint && <p className="mt-1 text-[11px] text-gray-600">{hint}</p>}
    </div>
  );
}
