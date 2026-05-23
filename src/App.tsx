import { useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect } from "react";
import { useTranslation } from "react-i18next";
import { scan } from "./api";
import { Sidebar } from "./components/Sidebar";
import { ArtifactList } from "./components/ArtifactList";
import { DetailPanel } from "./components/DetailPanel";
import { ImportWizard } from "./components/ImportWizard";
import { SettingsModal } from "./components/SettingsModal";
import { useErrorMessage } from "./useErrorMessage";
import type { ArtifactKind } from "./types";

export function App() {
  const qc = useQueryClient();
  const [selectedKind, setSelectedKind] = useState<ArtifactKind | null>(null);
  const [selectedName, setSelectedName] = useState<string | null>(null);
  const [wizardOpen, setWizardOpen] = useState(false);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { t } = useTranslation("common");
  const errorMessage = useErrorMessage();

  const { data, isLoading, error } = useQuery({
    queryKey: ["inventory"],
    queryFn: () => scan(),
  });

  useEffect(() => {
    const unlisten = listen("installation-changed", () => {
      qc.invalidateQueries({ queryKey: ["inventory"] });
    });
    return () => {
      unlisten.then((fn) => fn());
    };
  }, [qc]);

  const handleSelect = (name: string, kind: ArtifactKind) => {
    setSelectedName(name);
    setSelectedKind(kind);
  };

  if (isLoading) {
    return (
      <div className="flex h-screen items-center justify-center bg-gray-950 text-gray-500 text-sm">
        {t("scanning")}
      </div>
    );
  }

  if (error) {
    return (
      <div className="flex h-screen items-center justify-center bg-gray-950 text-red-400 text-sm">
        {errorMessage(error)}
      </div>
    );
  }

  const inventory = data!;

  return (
    <div className="flex h-screen bg-gray-950 text-gray-100 overflow-hidden">
      <Sidebar
        adapters={inventory.adapters}
        selectedKind={selectedKind}
        onKindSelect={(k) => {
          setSelectedKind(k);
          setSelectedName(null);
        }}
        onImportClick={() => setWizardOpen(true)}
        onSettingsClick={() => setSettingsOpen(true)}
      />

      <main className="flex flex-1 overflow-hidden">
        <div className="w-72 flex-none border-r border-gray-800 overflow-y-auto">
          {inventory.groups.length === 0 ? (
            <EmptyState onImport={() => setWizardOpen(true)} />
          ) : (
            <ArtifactList
              groups={inventory.groups}
              selectedKind={selectedKind}
              selectedName={selectedName}
              onSelect={handleSelect}
            />
          )}
        </div>

        <div className="flex-1 overflow-hidden">
          <DetailPanel
            groups={inventory.groups}
            selectedName={selectedName}
            selectedKind={selectedKind}
          />
        </div>
      </main>

      {wizardOpen && (
        <ImportWizard
          onClose={() => setWizardOpen(false)}
          onOpenSettings={() => setSettingsOpen(true)}
        />
      )}
      {settingsOpen && <SettingsModal onClose={() => setSettingsOpen(false)} />}
    </div>
  );
}

function EmptyState({ onImport }: { onImport: () => void }) {
  const { t } = useTranslation("common");
  return (
    <div className="flex flex-col items-center justify-center h-full px-6 py-12 text-center">
      <p className="text-gray-500 text-sm mb-4">{t("noArtifacts")}</p>
      <button
        onClick={onImport}
        className="text-sm bg-indigo-600 hover:bg-indigo-500 text-white px-4 py-2 rounded"
      >
        {t("importFirst")}
      </button>
    </div>
  );
}
