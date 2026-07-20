import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { useNavigate } from "react-router-dom";
import { Layers, Plus, X } from "lucide-react";

import { cn } from "../../design-tokens";
import { BottomMenu } from "../../components/BottomMenu";
import { NumberInput } from "../../components/NumberInput";
import { useI18n } from "../../../core/i18n/context";
import {
  loraArchitectureLabel,
  type SdcppLoraFile,
} from "../../../core/image-generation/loras";
import type { PlaygroundLoraSelection } from "../../../core/image-generation/playground";
import { Routes } from "../../navigation";
import type { PlaygroundSettingsController } from "./usePlaygroundSettings";

export function PlaygroundLoraSection({
  controller,
}: {
  controller: PlaygroundSettingsController;
}) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const { selectedModel, draft, updateDraft } = controller;
  const [files, setFiles] = useState<SdcppLoraFile[]>([]);
  const [pickerOpen, setPickerOpen] = useState(false);

  const profileId =
    (selectedModel?.advancedModelSettings?.sdcppProfileId as string | undefined) ?? null;

  useEffect(() => {
    let cancelled = false;
    invoke<SdcppLoraFile[]>("sdcpp_loras", { profileId })
      .then((loaded) => {
        if (!cancelled) setFiles(loaded);
      })
      .catch(() => {
        if (!cancelled) setFiles([]);
      });
    return () => {
      cancelled = true;
    };
  }, [profileId, pickerOpen]);

  const selections = draft.loras ?? [];
  const selectedPaths = new Set(selections.map((lora) => lora.path));

  const updateSelections = (next: PlaygroundLoraSelection[]) => {
    updateDraft({ loras: next });
  };

  const addLora = (file: SdcppLoraFile) => {
    if (selectedPaths.has(file.path)) return;
    updateSelections([
      ...selections,
      { path: file.path, multiplier: 0.8, keywords: file.keywords },
    ]);
    setPickerOpen(false);
  };

  return (
    <div className="space-y-2.5 rounded-2xl border border-fg/10 bg-fg/4 p-3.5">
      <div className="flex items-center justify-between">
        <p className="text-[11px] font-medium uppercase tracking-wide text-fg/40">
          {t("playground.loras.title")}
        </p>
        <button
          type="button"
          onClick={() => setPickerOpen(true)}
          className="flex items-center gap-1 rounded-lg border border-fg/10 bg-fg/5 px-2 py-1 text-[11px] font-medium text-fg/65 transition-all hover:border-fg/15 hover:bg-fg/[0.07] hover:text-fg active:scale-95"
        >
          <Plus size={11} />
          {t("playground.loras.add")}
        </button>
      </div>
      {selections.length === 0 ? (
        <p className="text-[11.5px] text-fg/35">{t("playground.loras.empty")}</p>
      ) : (
        <div className="space-y-1.5">
          {selections.map((lora) => {
            const file = files.find((candidate) => candidate.path === lora.path);
            const fileName = lora.path.split(/[\\/]/).pop() || lora.path;
            return (
              <div
                key={lora.path}
                className="flex items-center gap-2 rounded-xl border border-fg/10 bg-fg/5 px-2.5 py-2"
              >
                <Layers size={12} className="shrink-0 text-accent/60" />
                <div className="min-w-0 flex-1">
                  <p className="truncate font-mono text-[11.5px] text-fg/80">{fileName}</p>
                  {file?.architecture && (
                    <p className="text-[10px] text-fg/40">
                      {loraArchitectureLabel(
                        file.architecture,
                        t("editModel.sdcpp.architectureUnknown"),
                      )}
                    </p>
                  )}
                </div>
                <NumberInput
                  min={0}
                  max={2}
                  step={0.05}
                  value={lora.multiplier}
                  onChange={(value) =>
                    updateSelections(
                      selections.map((item) =>
                        item.path === lora.path ? { ...item, multiplier: value ?? 0.8 } : item,
                      ),
                    )
                  }
                  className="h-7 w-16 rounded-md border border-fg/10 bg-surface px-1.5 text-center text-[11.5px] text-fg transition focus:border-accent/40 focus:outline-none"
                />
                <button
                  type="button"
                  onClick={() =>
                    updateSelections(selections.filter((item) => item.path !== lora.path))
                  }
                  className="shrink-0 rounded-md p-1 text-fg/35 transition hover:bg-fg/8 hover:text-danger"
                >
                  <X size={12} />
                </button>
              </div>
            );
          })}
        </div>
      )}

      <BottomMenu
        isOpen={pickerOpen}
        onClose={() => setPickerOpen(false)}
        title={t("playground.loras.pickerTitle")}
      >
        <div className="max-h-[55vh] space-y-1.5 overflow-y-auto">
          {files.length === 0 ? (
            <p className="rounded-xl border border-dashed border-fg/10 bg-fg/2 px-4 py-6 text-center text-[12px] text-fg/45">
              {t("playground.loras.none")}
            </p>
          ) : (
            files.map((file) => {
              const selected = selectedPaths.has(file.path);
              const incompatible = file.compatibility === "incompatible";
              return (
                <button
                  key={file.path}
                  type="button"
                  disabled={selected || incompatible}
                  onClick={() => addLora(file)}
                  className={cn(
                    "flex w-full items-center justify-between gap-3 rounded-xl border px-3.5 py-2.5 text-left transition",
                    selected
                      ? "border-accent/30 bg-accent/8 opacity-60"
                      : incompatible
                        ? "border-fg/8 bg-fg/2 opacity-45"
                        : "border-fg/10 bg-fg/3 hover:border-fg/20",
                  )}
                >
                  <div className="min-w-0 flex-1">
                    <p className="truncate font-mono text-[12px] text-fg/85">{file.filename}</p>
                    <p className="text-[10.5px] text-fg/45">
                      {loraArchitectureLabel(
                        file.architecture,
                        t("editModel.sdcpp.architectureUnknown"),
                      )}
                      {incompatible ? ` · ${t("editModel.sdcpp.loraIncompatible")}` : ""}
                    </p>
                  </div>
                </button>
              );
            })
          )}
        </div>
        <button
          type="button"
          onClick={() => navigate(`${Routes.settingsModelsLoras}?returnTo=${Routes.playground}`)}
          className="mt-3 w-full rounded-xl border border-fg/10 bg-fg/4 px-4 py-2.5 text-[12.5px] font-medium text-fg/70 transition hover:border-fg/20 hover:text-fg"
        >
          {t("playground.loras.openLibrary")}
        </button>
      </BottomMenu>
    </div>
  );
}
