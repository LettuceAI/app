import { memo, type ReactNode } from "react";
import { Pin } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Badge, cn, icon, InlineMarkdown, Switch } from "@lettuceai/crisp";
import type { LorebookEntryView } from "@/api/generated/bindings";

export const EntryRow = memo(function EntryRow({
  entry,
  handle,
  onOpen,
  onToggle,
}: {
  entry: LorebookEntryView;
  handle: ReactNode;
  onOpen: (entry: LorebookEntryView) => void;
  onToggle: (entry: LorebookEntryView, enabled: boolean) => void;
}) {
  const { t } = useTranslation("lorebook-editor");
  const { t: preview } = useTranslation("lorebook-preview");
  const title = entry.title || preview("untitled");
  return (
    <div
      className={cn(
        "group flex items-start gap-2 rounded-2xl border border-line bg-surface-1 p-3 transition-colors hover:bg-fill @xl:p-4",
        !entry.enabled && "opacity-60",
      )}
    >
      {handle}
      <button
        type="button"
        onClick={() => onOpen(entry)}
        className="min-w-0 flex-1 rounded-lg text-left focus-visible:outline focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-accent"
      >
        <div className="flex items-center gap-2">
          <p className="truncate text-base font-medium text-fg">{title}</p>
          {entry.always_active && (
            <Badge tone="accent" className="shrink-0">
              <Pin size={icon.xs} />
              {t("alwaysBadge")}
            </Badge>
          )}
        </div>
        <p className="mt-1 line-clamp-2 wrap-anywhere text-sm leading-snug text-fg-2">
          <InlineMarkdown>{entry.content}</InlineMarkdown>
        </p>
        <div className="mt-2 flex flex-wrap gap-1">
          {entry.always_active ? (
            <span className="text-xs text-fg-3">{t("inEveryPrompt")}</span>
          ) : entry.keywords.length === 0 ? (
            <span className="text-xs text-fg-3">{t("noKeywords")}</span>
          ) : (
            entry.keywords.map((keyword, index) => (
              <span
                key={`${index}:${keyword}`}
                className="max-w-full truncate rounded-md bg-fill-2 px-1.5 py-0.5 font-mono text-xs text-fg-2"
              >
                {keyword}
              </span>
            ))
          )}
        </div>
      </button>
      <Switch
        checked={entry.enabled}
        onChange={(enabled) => onToggle(entry, enabled)}
        aria-label={t("enabledFor", { title })}
        className="mt-0.5"
      />
    </div>
  );
});
