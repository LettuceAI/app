import { useLayoutEffect, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { FileText } from "lucide-react";
import { icon, ListRowButton, Skeleton } from "@lettuceai/crisp";
import type { SettingsImageGenerationSettings, SettingsView } from "@/api/generated/bindings";
import { PromptPickerSheet, promptsQuery } from "@/entities/prompt";
import { useSettingsApi } from "../data";

const imagePromptFields = {
  avatar_generation: "avatar_prompt_id",
  avatar_edit_request: "avatar_edit_prompt_id",
  scene_generation: "scene_prompt_id",
  scene_prompt_writer: "scene_writer_prompt_id",
  design_reference_writer: "design_reference_prompt_id",
} as const;
export type ImagePromptKind = keyof typeof imagePromptFields;

export function ImagePromptSetting({ kind, value, disabled, disabledReason, open, onOpen, onClose, onChange }: {
  kind: ImagePromptKind;
  value: SettingsImageGenerationSettings;
  disabled: boolean;
  disabledReason?: string;
  open: boolean;
  onOpen: (kind: ImagePromptKind) => void;
  onClose: () => void;
  onChange: (patch: Partial<SettingsImageGenerationSettings>) => Promise<SettingsView | null> | undefined;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const prompts = useQuery(promptsQuery(api, kind));
  const [pending, setPending] = useState(false);
  const visible = useRef(open);
  useLayoutEffect(() => { visible.current = open; }, [open]);
  const id = value[imagePromptFields[kind]];
  const chosen = prompts.data?.find((prompt) => prompt.id === id);
  const title = t(`image.prompts.${kind}`);
  const choose = async (id: string | null) => {
    if (disabled || pending) return;
    setPending(true);
    try {
      if (await onChange({ [imagePromptFields[kind]]: id }) && visible.current) onClose();
    } finally {
      setPending(false);
    }
  };
  return <>
    <ListRowButton icon={<FileText size={icon.lg} />} title={title}
      description={disabledReason ?? (prompts.isPending ? <Skeleton className="h-4 w-32" /> : prompts.isError ? t("image.prompts.unavailable") : id === null ? t("image.prompts.builtIn") : chosen?.name ?? t("image.prompts.unavailable"))}
      disabled={disabled || pending} onClick={() => onOpen(kind)} />
    <PromptPickerSheet open={open} title={title} kind={kind} prompts={prompts} value={id}
      busy={disabled || pending} onChoose={(id) => { void choose(id); }} onClose={onClose}
      onManage={() => void navigate({ to: "/settings/$section", params: { section: "prompts" }, search: {}, replace: true })} />
  </>;
}
