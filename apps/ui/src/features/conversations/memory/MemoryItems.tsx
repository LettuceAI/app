import { useState } from "react";
import { Bot, Download, Flame, Pin, Snowflake, Trash2, User } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, cn, confirm, DatePicker, Field, icon, ListGroup, ListRow, relativeTime, Segmented, Select, Sheet, Switch, Textarea } from "@lettuceai/crisp";
import type { MemoryCategory, MemoryItemView, MemoryOrigin, MemoryTemperature } from "@/api/generated/bindings";
import { dateOf, sameInstant } from "../data/memory";
import { useWindowLabel } from "./window-label";

const originIcon: Record<MemoryOrigin, typeof Bot> = { model: Bot, user: User, import: Download };

export function MemoryRow({ item, onOpen, companion = false }: { item: MemoryItemView; onOpen: () => void; companion?: boolean }) {
  const { t, i18n } = useTranslation("conversations");
  const windowLabel = useWindowLabel();
  const Origin = originIcon[item.origin];
  const cycle = windowLabel(item.cycle_label);
  return (
    <button type="button" onClick={onOpen} className="group flex w-full flex-col gap-2 px-4 py-3.5 text-left transition-colors hover:bg-fill">
      <div className="flex items-start gap-3">
        <p className="min-w-0 flex-1 line-clamp-3 text-base leading-snug text-fg">{item.text}</p>
        {item.pinned && <Pin size={icon.sm} className="mt-0.5 shrink-0 text-fg-3" aria-label={t("memory.pinnedMark")} />}
      </div>
      <div className="flex flex-wrap items-center gap-x-3 gap-y-1 text-sm text-fg-3">
        <span className="inline-flex items-center gap-1">
          <Origin size={icon.sm} />
          {t(`memory.origins.${item.origin}`)}
        </span>
        {item.category && <span>{t(`memory.categories.${item.category}`)}</span>}
        <span className={cn("inline-flex items-center gap-1", item.temperature === "cold" ? "text-info" : "text-warning")}>
          {item.temperature === "cold" ? <Snowflake size={icon.sm} /> : <Flame size={icon.sm} />}
          {t(`memory.temperature.${item.temperature}`)}
        </span>
        {!companion && item.token_count !== null && <span>{t("memory.tokens", { count: item.token_count })}</span>}
        {companion && item.observed_at !== null && <span>{relativeTime(item.observed_at, undefined, i18n.language)}</span>}
        {cycle && <span>{cycle}</span>}
      </div>
    </button>
  );
}

export interface MemoryDraft {
  text: string;
  category: MemoryCategory | null;
  observedAt: Date | null;
  temperature: MemoryTemperature;
  pinned: boolean;
}

export interface MemorySheetProps {
  open: boolean;
  item: MemoryItemView | null;
  categories: readonly MemoryCategory[];
  busy: boolean;
  onClose: () => void;
  onSave: (draft: MemoryDraft) => void;
  onDelete: () => void;
  companionName?: string;
}

export function MemorySheet({ open, item, categories, busy, onClose, onSave, onDelete, companionName }: MemorySheetProps) {
  const { t, i18n } = useTranslation("conversations");
  const windowLabel = useWindowLabel();
  const adding = item === null;
  const [text, setText] = useState(item?.text ?? "");
  const [category, setCategory] = useState<MemoryCategory | "">(item?.category ?? (companionName && categories.includes("profile") ? "profile" : ""));
  const [observedAt, setObservedAt] = useState<Date | null>(dateOf(item?.observed_at ?? null));
  const [temperature, setTemperature] = useState<MemoryTemperature>(item?.temperature ?? "hot");
  const [pinned, setPinned] = useState(item?.pinned ?? false);
  const cycle = windowLabel(item?.cycle_label ?? null);
  const description = adding
    ? undefined
    : item.origin === "model"
      ? cycle
        ? t("memory.sheet.byModelIn", { window: cycle })
        : t("memory.sheet.byModel")
      : item.origin === "import"
        ? t("memory.sheet.imported")
        : t("memory.sheet.byUser");

  const changed =
    adding ||
    text.trim() !== item.text ||
    (category || null) !== item.category ||
    !sameInstant(observedAt, dateOf(item.observed_at)) ||
    temperature !== item.temperature ||
    pinned !== item.pinned;

  const askDelete = async () => {
    const sure = await confirm({
      title: t("memory.sheet.deleteTitle"),
      message: t("memory.sheet.deleteBody"),
      confirmLabel: t("memory.sheet.delete"),
      cancelLabel: t("memory.sheet.keep"),
      destructive: true,
    });
    if (sure) onDelete();
  };

  return (
    <Sheet
      open={open}
      onClose={() => {
        if (!busy) onClose();
      }}
      placement="auto"
      size="md"
      title={adding ? t("memory.sheet.addTitle") : t("memory.sheet.editTitle")}
      {...(description ? { description } : {})}
      footer={
        <div className="flex items-center gap-2">
          {!adding && (
            <Button variant="danger" disabled={busy} leading={<Trash2 size={icon.sm} />} onClick={() => void askDelete()}>
              {t("memory.sheet.delete")}
            </Button>
          )}
          <span className="flex-1" />
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {t("memory.sheet.cancel")}
          </Button>
          <Button
            loading={busy}
            disabled={busy || text.trim().length === 0 || !changed}
            onClick={() => onSave({ text: text.trim(), category: category || null, observedAt, temperature, pinned })}
          >
            {adding ? t("memory.sheet.add") : t("memory.sheet.save")}
          </Button>
        </div>
      }
    >
      <div className="space-y-5 pb-2">
        <Field label={t(companionName ? "companion.memories.field" : "memory.sheet.text")}>
          <Textarea
            value={text}
            disabled={busy}
            rows={4}
            placeholder={companionName ? t("companion.memories.placeholder", { name: companionName }) : t("memory.sheet.textPlaceholder")}
            data-autofocus={adding || undefined}
            onChange={(event) => setText(event.target.value)}
          />
        </Field>
        <Field label={t(companionName ? "companion.memories.kind" : "memory.sheet.category")}>
          <Select
            value={category}
            disabled={busy}
            onChange={(event) => setCategory(event.target.value as MemoryCategory | "")}
            options={[
              { value: "", label: t("memory.sheet.noCategory") },
              ...(category && !categories.includes(category) ? [{ value: category, label: t(`memory.categories.${category}`), disabled: true }] : []),
              ...categories.map((id) => ({ value: id, label: t(`memory.categories.${id}`) })),
            ]}
          />
        </Field>
        {!companionName && <Field label={t("memory.sheet.when")} hint={t("memory.sheet.whenHint")}>
          <div className="flex items-center gap-2">
            <DatePicker className="min-w-0 flex-1" withTime disabled={busy} value={observedAt} placeholder={t("memory.sheet.whenPlaceholder")} locale={i18n.language} onChange={setObservedAt} />
            {observedAt && (
              <Button variant="ghost" disabled={busy} onClick={() => setObservedAt(null)}>
                {t("memory.sheet.whenClear")}
              </Button>
            )}
          </div>
        </Field>}
        {!adding && (
          <ListGroup>
            <ListRow
              title={t("memory.sheet.temperature")}
              description={companionName || temperature === "cold" ? t("memory.sheet.coldBody") : t("memory.sheet.hotBody")}
              trailing={
                <fieldset disabled={busy}>
                  <Segmented
                    size="sm"
                    value={temperature}
                    aria-label={t("memory.sheet.temperature")}
                    onChange={(next) => {
                      setTemperature(next);
                      if (next === "cold") setPinned(false);
                    }}
                    options={[
                      { value: "hot", label: t("memory.temperature.hot") },
                      { value: "cold", label: t("memory.temperature.cold") },
                    ]}
                  />
                </fieldset>
              }
            />
            <ListRow
              title={t("memory.sheet.pin")}
              description={t("memory.sheet.pinBody")}
              trailing={<Switch checked={pinned} disabled={busy} onChange={(next) => {
                setPinned(next);
                if (next) setTemperature("hot");
              }} aria-label={t("memory.sheet.pin")} />}
            />
          </ListGroup>
        )}
      </div>
    </Sheet>
  );
}
