import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { BookOpen, Check, Cpu, FileText } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Checkbox, EmptyState, icon, ListGroup, ListRow, ListRowButton, LoadingState, SearchField, Sheet } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { modelsQuery, useChatModels } from "@/features/conversations";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";
import { inheritedPromptName, type CharacterPromptKind } from "../data/prompts";
import { lorebooksQuery, promptsQuery } from "../data/queries";

const mark = <Check size={icon.md} />;

interface ChoiceSheetProps {
  api: ApiClient;
  open: boolean;
  value: string | null;
  onChange: (id: string | null) => void;
  onClose: () => void;
}

/** One model for the character's chats, or the app default. */
export function ModelChoiceSheet({ api, open, value, onChange, onClose }: ChoiceSheetProps) {
  const { t } = useTranslation("characters");
  const status = useQuery(modelsQuery(api));
  const chat = useChatModels(api);
  const [query, setQuery] = useState("");
  const needle = query.trim().toLowerCase();
  const shown = chat.models.filter((model) => !needle || model.name.toLowerCase().includes(needle));
  const choose = (id: string | null) => {
    onChange(id);
    onClose();
  };
  return (
    <Sheet open={open} onClose={onClose} size="sm" title={t("pickers.model")}>
      {status.isPending ? (
        <LoadingState />
      ) : status.isError ? (
        <FailureState error={status.error} onRetry={() => void status.refetch()} />
      ) : (
        <div className="space-y-3 pb-1">
          {chat.models.length > 8 && <SearchField value={query} onChange={setQuery} placeholder={t("pickers.modelSearch")} aria-label={t("pickers.modelSearch")} />}
          <ListGroup>
            <ListRowButton
              icon={<Cpu size={icon.lg} />}
              title={t("details.modelDefault")}
              description={t("pickers.modelDefaultBody")}
              chevron={false}
              trailing={value === null ? mark : undefined}
              onClick={() => choose(null)}
            />
            {shown.map((model) => (
              <ListRowButton
                key={model.id}
                icon={<ModelMark modelId={model.externalId} providerKind={model.providerKind ?? ""} name={model.name} />}
                title={model.name}
                description={model.local ? t("pickers.onDevice") : model.provider}
                chevron={false}
                trailing={model.id === value ? mark : undefined}
                onClick={() => choose(model.id)}
              />
            ))}
          </ListGroup>
          {chat.models.length === 0 && <p className="py-2 text-center text-sm text-fg-3">{t("pickers.noModels")}</p>}
        </div>
      )}
    </Sheet>
  );
}

/** A prompt for one character slot, or that slot's inherited default. */
export function PromptChoiceSheet({ api, promptKind: kind, title, open, value, onChange, onClose }: ChoiceSheetProps & { promptKind: CharacterPromptKind; title: string }) {
  const { t } = useTranslation("characters");
  const prompts = useQuery(promptsQuery(api, kind));
  const inherited = inheritedPromptName(prompts.data ?? [], kind);
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
          <ListRowButton
            icon={<FileText size={icon.lg} />}
            title={t("details.promptDefault")}
            description={inherited ?? t("pickers.promptDefaultBody")}
            chevron={false}
            trailing={value === null ? mark : undefined}
            onClick={() => choose(null)}
          />
          {prompts.data.map((prompt) => (
            <ListRowButton
              key={prompt.id}
              icon={<FileText size={icon.lg} />}
              title={prompt.name}
              description={prompt.origin.type === "built_in" ? t("pickers.promptBuiltin") : t("pickers.promptCustom")}
              chevron={false}
              trailing={prompt.id === value ? mark : undefined}
              onClick={() => choose(prompt.id)}
            />
          ))}
        </ListGroup>
      )}
    </Sheet>
  );
}

/** Any number of lorebooks, attached in the order they were picked. */
export function LorebookChoiceSheet({ api, open, value, onChange, onClose }: { api: ApiClient; open: boolean; value: string[]; onChange: (ids: string[]) => void; onClose: () => void }) {
  const { t } = useTranslation("characters");
  const books = useQuery(lorebooksQuery(api));
  const toggle = (id: string, on: boolean) => onChange(on ? [...value, id] : value.filter((entry) => entry !== id));
  return (
    <Sheet
      open={open}
      onClose={onClose}
      size="sm"
      title={t("pickers.lorebooks")}
      footer={<Button variant="primary" className="w-full" onClick={onClose}>{t("pickers.done")}</Button>}
    >
      {books.isPending ? (
        <LoadingState />
      ) : books.isError ? (
        <FailureState error={books.error} onRetry={() => void books.refetch()} />
      ) : books.data.length === 0 ? (
        <EmptyState size="sm" icon={<BookOpen size={icon.xl} />} title={t("extras.lorebooksEmpty")} description={t("extras.lorebooksEmptyBody")} />
      ) : (
        <ListGroup className="mb-1">
          {books.data.map((book) => (
            <ListRow
              key={book.id}
              icon={<BookOpen size={icon.lg} />}
              title={book.name}
              trailing={<Checkbox checked={value.includes(book.id)} onChange={(on) => toggle(book.id, on)} aria-label={book.name} />}
            />
          ))}
        </ListGroup>
      )}
    </Sheet>
  );
}
