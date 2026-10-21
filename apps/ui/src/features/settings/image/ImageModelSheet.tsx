import { useState } from "react";
import { Check, Cpu } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, icon, ListGroup, ListRowButton, SearchField, Sheet } from "@lettuceai/crisp";
import type { ModelView, ProviderAccountView } from "@/api/generated/bindings";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";

export type ImagePicker = "avatar" | "scene" | "writer";
export interface ImageModelChoice {
  model: ModelView;
  account: ProviderAccountView;
}

export function ImageModelSheet({ picker, open, choices, value, busy, failure, onChange, onClose, onAdd }: {
  picker: ImagePicker;
  open: boolean;
  choices: ImageModelChoice[];
  value: string | null;
  busy: boolean;
  failure: { error: unknown; retry: () => void } | null;
  onChange: (id: string | null) => void;
  onClose: () => void;
  onAdd: () => void;
}) {
  const { t } = useTranslation("settings");
  const [query, setQuery] = useState("");
  const [wasOpen, setWasOpen] = useState(open);
  if (wasOpen !== open) {
    setWasOpen(open);
    setQuery("");
  }
  const needle = query.trim().toLocaleLowerCase();
  const found = choices.filter(({ model, account }) => `${model.display_name} ${model.external_model_id} ${account.label} ${account.provider_kind}`.toLocaleLowerCase().includes(needle));
  return (
    <Sheet open={open} onClose={onClose} title={t(`image.${picker}.model`)} size="sm">
      <div className="space-y-3 pb-1">
        {failure && <FailureState error={failure.error} onRetry={failure.retry} size="sm" />}
        <SearchField value={query} onChange={setQuery} placeholder={t("image.search")} aria-label={t("image.search")} />
        <ListGroup>
          <ListRowButton
            icon={<Cpu size={icon.lg} />}
            title={t(picker === "writer" ? "image.writer.first" : "image.first")}
            description={t(picker === "writer" ? "image.writer.firstBody" : "image.firstBody")}
            chevron={false}
            aria-pressed={value === null}
            trailing={value === null ? <Check size={icon.md} /> : undefined}
            disabled={busy}
            onClick={() => onChange(null)}
          />
          {found.map(({ model, account }) => (
            <ListRowButton
              key={model.id}
              icon={<ModelMark modelId={model.external_model_id} providerKind={account.provider_kind} name={model.display_name} />}
              title={<span className="block truncate">{model.display_name}</span>}
              description={<span className="block truncate">{account.label}</span>}
              chevron={false}
              aria-pressed={value === model.id}
              trailing={value === model.id ? <Check size={icon.md} /> : undefined}
              disabled={busy}
              onClick={() => onChange(model.id)}
            />
          ))}
        </ListGroup>
        {choices.length === 0 ? (
          <EmptyState title={t(picker === "writer" ? "image.writer.empty" : "image.empty.title")} description={t(picker === "writer" ? "image.writer.emptyBody" : "image.empty.body")} action={<Button variant="secondary" onClick={onAdd}>{t("image.addModel")}</Button>} />
        ) : found.length === 0 ? <EmptyState title={t("image.noMatches")} /> : null}
      </div>
    </Sheet>
  );
}
