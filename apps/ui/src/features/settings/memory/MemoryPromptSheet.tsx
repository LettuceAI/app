import { useQuery } from "@tanstack/react-query";
import { Check, FileText } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRowButton, LoadingState, Sheet } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { PromptKind } from "@/api/generated/bindings";
import { promptsQuery } from "@/features/characters";
import { FailureState } from "@/shared/ui/FailureState";

const mark = <Check size={icon.md} />;

export function MemoryPromptSheet({ api, kind, title, open, value, onChange, onClose }: { api: ApiClient; kind: PromptKind; title: string; open: boolean; value: string | null; onChange: (id: string | null) => void; onClose: () => void }) {
  const { t } = useTranslation("settings");
  const prompts = useQuery({ ...promptsQuery(api, kind), enabled: open });
  const choose = (id: string | null) => {
    onChange(id);
    onClose();
  };
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={title}>
      {prompts.isPending ? (
        <LoadingState />
      ) : prompts.isError ? (
        <FailureState error={prompts.error} onRetry={() => void prompts.refetch()} />
      ) : (
        <ListGroup className="mb-1">
          <ListRowButton icon={<FileText size={icon.lg} />} title={t("memory.summarization.builtIn")} description={t("memory.summarization.promptBuiltInBody")} chevron={false} trailing={value === null ? mark : undefined} onClick={() => choose(null)} />
          {prompts.data.map((prompt) => (
            <ListRowButton key={prompt.id} icon={<FileText size={icon.lg} />} title={prompt.name} chevron={false} trailing={prompt.id === value ? mark : undefined} onClick={() => choose(prompt.id)} />
          ))}
        </ListGroup>
      )}
    </Sheet>
  );
}
