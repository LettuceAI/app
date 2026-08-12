import { cn, typography } from "../../../design-tokens";
import { useI18n } from "../../../../core/i18n/context";
import { LorebookAvatar } from "../../../components/LorebookAvatar";

interface LorebookPreview {
  id?: string;
  name?: string;
  avatarPath?: string;
}

interface LorebookEntryPreview {
  id: string;
  lorebookId?: string;
  title?: string;
  content?: string;
  keywords?: string[];
  alwaysActive?: boolean;
  enabled?: boolean;
  displayOrder?: number;
}

export function LorebookPreviewCard({
  lorebook,
  entries,
}: {
  lorebook: LorebookPreview | null;
  entries: LorebookEntryPreview[];
}) {
  const { t } = useI18n();
  const name = lorebook?.name?.trim() || t("characters.lorebookPreview.untitledLorebook");
  const entryCount = entries.length;
  const previewEntries = entries.slice(0, 4);

  return (
    <div className="rounded-2xl border border-fg/10 bg-fg/5 overflow-hidden">
      <div className="p-4">
        <div className="flex items-start gap-3">
          <div className="h-12 w-12 overflow-hidden rounded-xl border border-fg/10 bg-fg/5">
            <LorebookAvatar
              avatarPath={lorebook?.avatarPath}
              name={name}
              iconClassName="h-5 w-5 text-fg/60"
              fallbackClassName="bg-fg/5"
            />
          </div>
          <div className="flex-1 min-w-0">
            <h3 className={cn(typography.h2.size, typography.h2.weight, "text-fg truncate")}>
              {name}
            </h3>
            <p className="text-xs text-fg/50 mt-1">
              {t("characters.lorebookPreview.entryCount", { count: entryCount })}
            </p>
          </div>
        </div>

        <div className="mt-4">
          <p className="text-xs text-fg/40 uppercase tracking-wider mb-2">
            {t("characters.lorebookPreview.entries")}
          </p>
          {entryCount === 0 ? (
            <p className="text-sm text-fg/50">{t("characters.lorebookPreview.noEntriesYet")}</p>
          ) : (
            <div className="space-y-2">
              {previewEntries.map((entry) => {
                const title = entry.title?.trim() || t("characters.lorebookPreview.untitledEntry");
                const content =
                  entry.content?.trim() || t("characters.lorebookPreview.noContentYet");
                const keywords = entry.keywords?.filter(Boolean) ?? [];
                return (
                  <div
                    key={entry.id}
                    className={cn(
                      "rounded-lg border border-fg/10 bg-fg/4 px-3 py-2",
                      entry.enabled === false ? "opacity-60" : "",
                    )}
                  >
                    <div className="flex items-center justify-between gap-2">
                      <p className="text-sm text-fg/80 truncate">{title}</p>
                      {entry.alwaysActive && (
                        <span className="text-[10px] px-2 py-0.5 rounded-full bg-warning/15 border border-warning/30 text-warning">
                          {t("characters.lorebookPreview.alwaysActive")}
                        </span>
                      )}
                    </div>
                    <p className="text-xs text-fg/55 line-clamp-2 mt-1">{content}</p>
                    {keywords.length > 0 && (
                      <p className="text-[10px] text-fg/40 mt-1">
                        {keywords.slice(0, 3).join(", ")}
                        {keywords.length > 3 ? ` +${keywords.length - 3}` : ""}
                      </p>
                    )}
                  </div>
                );
              })}
              {entryCount > previewEntries.length && (
                <p className="text-xs text-fg/40">
                  {entryCount - previewEntries.length === 1
                    ? t("characters.lorebookPreview.moreEntry", {
                        count: entryCount - previewEntries.length,
                      })
                    : t("characters.lorebookPreview.moreEntries", {
                        count: entryCount - previewEntries.length,
                      })}
                </p>
              )}
            </div>
          )}
        </div>
      </div>
    </div>
  );
}
