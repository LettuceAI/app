import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { AlertTriangle, Check, FolderOpen, Layers, Plus, Trash2 } from "lucide-react";
import { EmptyState, icon, IconButton, ListGroup, ListRow, ListRowButton, LoadingState, Sheet, Slider } from "@lettuceai/crisp";
import type { InstalledLora } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useApi } from "@/api/use-api";
import type { ModelChoice } from "@/entities/model";
import { parseModelConfig } from "@/entities/model/model-config";
import { FailureState } from "@/shared/ui/FailureState";
import { SliderValue } from "@/shared/ui/SliderValue";
import type { PlaygroundDraft } from "./playground-data";

const defaultMultiplier = 0.8;
const fileName = (path: string) => path.split(/[\\/]/).at(-1) || path;

export function LoraSection({ choice, draft, update }: {
  choice: ModelChoice;
  draft: PlaygroundDraft;
  update: (patch: Partial<PlaygroundDraft>) => void;
}) {
  const { t } = useTranslation("playground");
  const api = useApi();
  const navigate = useNavigate();
  const [picking, setPicking] = useState(false);
  const profileId = parseModelConfig(choice.model.config).stable_diffusion.cpp?.profile_id ?? null;
  const library = useQuery({
    queryKey: [...queryKeys.loras.all, profileId],
    queryFn: () => api.call("lorasList", { profile_id: profileId }),
  });
  const files = library.data?.loras ?? [];
  const selected = draft.loras;
  const chosen = new Set(selected.map((lora) => lora.path));
  const architecture = (file: InstalledLora | undefined) => file?.architecture ?? t("loras.unknownBase");

  const add = (file: InstalledLora) => {
    if (chosen.has(file.path)) return;
    update({ loras: [...selected, { path: file.path, multiplier: defaultMultiplier, keywords: file.keywords }] });
    setPicking(false);
  };

  return (
    <>
      <ListGroup label={t("loras.title")}>
        {selected.length === 0 && <ListRow title={t("loras.empty")} />}
        {selected.map((lora) => {
          const file = files.find((candidate) => candidate.path === lora.path);
          const name = fileName(lora.path);
          const strength = lora.multiplier ?? defaultMultiplier;
          const change = (multiplier: number) => update({ loras: selected.map((item) => (item.path === lora.path ? { ...item, multiplier } : item)) });
          return (
            <div key={lora.path} className="space-y-2 px-4 py-3">
              <div className="flex items-start gap-2">
                <div className="min-w-0 flex-1">
                  <p className="truncate font-mono text-sm text-fg">{name}</p>
                  {file && <p className="text-xs text-fg-3">{architecture(file)}</p>}
                </div>
                <IconButton size="sm" label={t("loras.remove", { name })} onClick={() => update({ loras: selected.filter((item) => item.path !== lora.path) })}>
                  <Trash2 size={icon.sm} />
                </IconButton>
              </div>
              <div className="flex items-center gap-3">
                <Slider aria-label={t("loras.strength", { name })} value={strength} min={0} max={2} step={0.05} onChange={change} className="min-w-0 flex-1" />
                <SliderValue value={strength} text={strength.toFixed(2)} overridden min={0} max={2} step={0.05} label={t("loras.strength", { name })} onCommit={change} />
              </div>
            </div>
          );
        })}
        <ListRowButton icon={<Plus size={icon.lg} />} title={t("loras.add")} chevron={false} onClick={() => setPicking(true)} />
      </ListGroup>

      <Sheet open={picking} onClose={() => setPicking(false)} size="md" title={t("loras.pickerTitle")}>
        <div className="space-y-5">
          <ListGroup>
            <ListRowButton
              icon={<FolderOpen size={icon.lg} />}
              title={t("loras.openLibrary")}
              description={t("loras.libraryDescription")}
              onClick={() => void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: { localImageView: "loras" } })}
            />
          </ListGroup>
          {library.isError ? (
            <FailureState size="sm" error={library.error} onRetry={() => void library.refetch()} />
          ) : library.isPending ? (
            <LoadingState />
          ) : files.length === 0 ? (
            <EmptyState size="sm" title={t("loras.none")} />
          ) : (
            <ListGroup>
              {files.map((file) => {
                const incompatible = file.compatibility === "incompatible";
                const on = chosen.has(file.path);
                return (
                  <ListRowButton
                    key={file.path}
                    icon={<Layers size={icon.lg} />}
                    title={file.filename}
                    description={incompatible ? `${architecture(file)} · ${t("loras.incompatible")}` : architecture(file)}
                    chevron={false}
                    trailing={on ? <Check size={icon.md} className="text-accent" /> : incompatible ? <AlertTriangle size={icon.md} className="text-danger" /> : undefined}
                    disabled={on || incompatible}
                    onClick={() => add(file)}
                  />
                );
              })}
            </ListGroup>
          )}
        </div>
      </Sheet>
    </>
  );
}
