import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ListRowButton, Skeleton } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { PromptPickerSheet, promptsQuery } from "@/entities/prompt";
import { inheritedPromptName, type CharacterPromptKind } from "../data/prompts";

export function PromptSetting({
  api,
  promptKind: kind,
  label,
  value,
  onChange,
}: {
  api: ApiClient;
  promptKind: CharacterPromptKind;
  label: string;
  value: string | null;
  onChange: (id: string | null) => void;
}) {
  const { t } = useTranslation("characters");
  const [open, setOpen] = useState(false);
  const navigate = useNavigate();
  const prompts = useQuery(promptsQuery(api, kind));
  const inherited = inheritedPromptName(prompts.data ?? [], kind);
  const source = t("details.promptDefault");
  const name =
    value === null
      ? (inherited ?? source)
      : (prompts.data?.find((prompt) => prompt.id === value)?.name ?? t("details.promptMissing"));
  const showSource = value === null && !prompts.isPending;
  const description = prompts.isPending ? (
    <Skeleton className="mt-1 h-3.5 w-32" />
  ) : prompts.isError ? (
    t("details.promptUnavailable")
  ) : (
    name
  );
  return (
    <>
      <ListRowButton
        title={label}
        description={description}
        aria-busy={prompts.isPending}
        disabled={prompts.isPending}
        chevron
        trailing={showSource ? <span className="text-sm text-fg-3">{source}</span> : undefined}
        onClick={() => setOpen(true)}
      />
      <PromptPickerSheet
        kind={kind}
        title={label}
        open={open}
        value={value}
        prompts={prompts}
        defaultChoice={{ title: source, description: inherited ?? t("pickers.promptDefaultBody") }}
        onChoose={(id) => {
          onChange(id);
          setOpen(false);
        }}
        onClose={() => setOpen(false)}
        onManage={() => {
          setOpen(false);
          void navigate({ to: "/settings/$section", params: { section: "prompts" }, search: {} });
        }}
      />
    </>
  );
}
