import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import type { ApiClient } from "@/api/client";
import type { PromptKind } from "@/api/generated/bindings";
import { PromptPickerSheet, promptsQuery } from "@/entities/prompt";

export function MemoryPromptSheet({
  api,
  kind,
  title,
  open,
  value,
  onChange,
  onClose,
}: {
  api: ApiClient;
  kind: PromptKind;
  title: string;
  open: boolean;
  value: string | null;
  onChange: (id: string | null) => void;
  onClose: () => void;
}) {
  const navigate = useNavigate();
  const prompts = useQuery({ ...promptsQuery(api, kind), enabled: open });
  return (
    <PromptPickerSheet
      open={open}
      kind={kind}
      title={title}
      prompts={prompts}
      value={value}
      onChoose={(id) => {
        onChange(id);
        onClose();
      }}
      onClose={onClose}
      onManage={() => {
        onClose();
        void navigate({ to: "/settings/$section", params: { section: "prompts" }, search: {} });
      }}
    />
  );
}
