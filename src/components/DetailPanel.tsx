import { useMutation, useQueryClient } from "@tanstack/react-query";
import { disable, enable, uninstall } from "../api";
import type {
  ArtifactGroupDto,
  ArtifactKind,
  InstallationDto,
  ScannedInstallationDto,
} from "../types";
import { sourceLabel, targetLabel } from "../types";

interface Props {
  groups: ArtifactGroupDto[];
  selectedName: string | null;
  selectedKind: ArtifactKind | null;
}

export function DetailPanel({ groups, selectedName, selectedKind }: Props) {
  const qc = useQueryClient();
  const group = groups.find(
    (g) => g.name === selectedName && g.kind === selectedKind
  );

  const invalidate = () => qc.invalidateQueries({ queryKey: ["inventory"] });

  const uninstallMut = useMutation({
    mutationFn: (i: InstallationDto) => uninstall(i),
    onSuccess: invalidate,
  });

  const enableMut = useMutation({
    mutationFn: (i: InstallationDto) => enable(i),
    onSuccess: invalidate,
  });

  const disableMut = useMutation({
    mutationFn: (i: InstallationDto) => disable(i),
    onSuccess: invalidate,
  });

  if (!group) {
    return (
      <div className="flex items-center justify-center h-full text-gray-600 text-sm">
        Select an artifact to see details.
      </div>
    );
  }

  const owned = group.installations.filter((i) => i.provenance === "owned");
  const shared = group.installations.filter((i) =>
    i.provenance.startsWith("shared:")
  );

  return (
    <div className="p-5 overflow-y-auto h-full">
      <div className="mb-4">
        <h2 className="text-lg font-semibold text-gray-100">{group.name}</h2>
        {group.version && (
          <p className="text-xs text-gray-500">v{group.version}</p>
        )}
        {group.description && (
          <p className="mt-1 text-sm text-gray-400">{group.description}</p>
        )}
      </div>

      <Section title="Source">
        {group.installations[0] ? (
          <p className="text-xs text-gray-400 break-all">
            {sourceLabel(group.installations[0].artifact.source)}
          </p>
        ) : (
          <p className="text-xs text-gray-600">Unknown</p>
        )}
      </Section>

      {owned.length > 0 && (
        <Section title="Installed">
          <ul className="space-y-3">
            {owned.map((si) => (
              <InstallationRow
                key={si.installation.id}
                si={si}
                onUninstall={() => uninstallMut.mutate(si.installation)}
                onEnable={() => enableMut.mutate(si.installation)}
                onDisable={() => disableMut.mutate(si.installation)}
                busy={
                  uninstallMut.isPending ||
                  enableMut.isPending ||
                  disableMut.isPending
                }
              />
            ))}
          </ul>
        </Section>
      )}

      {shared.length > 0 && (
        <Section title="Also visible to">
          <ul className="space-y-1">
            {shared.map((si) => (
              <li key={si.installation.id} className="text-xs text-gray-500">
                {si.provenance.replace("shared:", "")} —{" "}
                <span className="text-gray-600 break-all">
                  {si.installation.onDiskPath}
                </span>
              </li>
            ))}
          </ul>
        </Section>
      )}

      {owned.length === 0 && shared.length === 0 && (
        <p className="text-sm text-gray-600">Not installed on any target.</p>
      )}
    </div>
  );
}

function Section({
  title,
  children,
}: {
  title: string;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-5">
      <h3 className="text-xs font-semibold uppercase tracking-wider text-gray-500 mb-2">
        {title}
      </h3>
      {children}
    </div>
  );
}

function InstallationRow({
  si,
  onUninstall,
  onEnable,
  onDisable,
  busy,
}: {
  si: ScannedInstallationDto;
  onUninstall: () => void;
  onEnable: () => void;
  onDisable: () => void;
  busy: boolean;
}) {
  const { installation } = si;
  const isDisabled = installation.status === "disabled";

  return (
    <li className="rounded bg-gray-800 px-3 py-2 text-xs">
      <div className="flex items-center justify-between gap-2">
        <span className="font-medium text-gray-200">
          {targetLabel(installation.target)}
        </span>
        <span
          className={`px-1.5 py-0.5 rounded text-xs ${
            isDisabled
              ? "bg-yellow-900 text-yellow-300"
              : installation.status.startsWith("broken")
                ? "bg-red-900 text-red-300"
                : "bg-emerald-900 text-emerald-300"
          }`}
        >
          {installation.status}
        </span>
      </div>
      <p className="mt-1 text-gray-500 break-all">{installation.onDiskPath}</p>
      <div className="mt-2 flex gap-2">
        {isDisabled ? (
          <ActionButton onClick={onEnable} disabled={busy}>
            Enable
          </ActionButton>
        ) : (
          <ActionButton onClick={onDisable} disabled={busy}>
            Disable
          </ActionButton>
        )}
        <ActionButton
          onClick={onUninstall}
          disabled={busy}
          className="text-red-400 hover:text-red-300"
        >
          Uninstall
        </ActionButton>
      </div>
    </li>
  );
}

function ActionButton({
  onClick,
  disabled,
  children,
  className = "text-gray-400 hover:text-gray-200",
}: {
  onClick: () => void;
  disabled: boolean;
  children: React.ReactNode;
  className?: string;
}) {
  return (
    <button
      onClick={onClick}
      disabled={disabled}
      className={`text-xs disabled:opacity-40 ${className}`}
    >
      {children}
    </button>
  );
}
