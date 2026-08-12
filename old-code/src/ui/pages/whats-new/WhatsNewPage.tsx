import { useEffect, useMemo, useState } from "react";
import { createPortal } from "react-dom";
import { invoke } from "@tauri-apps/api/core";
import { AnimatePresence, motion } from "framer-motion";
import { ArrowRight, Sparkles } from "lucide-react";

import { cn } from "../../design-tokens";
import { setLastSeenAppVersion } from "../../../core/storage/appState";
import { changelog, type ChangelogEntry } from "../settings/ChangelogPage";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";

type ChangeType = ChangelogEntry["changes"][number]["type"];

const TYPE_LABEL = {
  feature: "updates.whatsNew.types.added",
  improvement: "updates.whatsNew.types.improved",
  bugfix: "updates.whatsNew.types.fixed",
  breaking: "updates.whatsNew.types.breaking",
} satisfies Record<ChangeType, TranslationKey>;

const TYPE_ORDER: ChangeType[] = ["feature", "improvement", "bugfix", "breaking"];

const TYPE_DOT: Record<ChangeType, string> = {
  feature: "bg-accent",
  improvement: "bg-info",
  bugfix: "bg-warning",
  breaking: "bg-danger",
};

export function WhatsNewDrawer({
  isOpen,
  onClose,
}: {
  isOpen: boolean;
  onClose: () => void;
}) {
  const { t } = useI18n();
  const [currentVersion, setCurrentVersion] = useState<string | null>(null);

  useEffect(() => {
    if (!isOpen) return;
    let cancelled = false;
    invoke<string>("get_app_version")
      .then((v) => {
        if (!cancelled) setCurrentVersion(v);
      })
      .catch(() => {
        if (!cancelled) setCurrentVersion(null);
      });
    return () => {
      cancelled = true;
    };
  }, [isOpen]);

  const entry: ChangelogEntry | undefined = changelog[0];

  const grouped = useMemo(() => {
    if (!entry) return [] as { type: ChangeType; items: string[] }[];
    const buckets = new Map<ChangeType, string[]>();
    for (const change of entry.changes) {
      const list = buckets.get(change.type) ?? [];
      list.push(change.description);
      buckets.set(change.type, list);
    }
    return TYPE_ORDER.filter((type) => buckets.has(type)).map((type) => ({
      type,
      items: buckets.get(type) ?? [],
    }));
  }, [entry]);

  const formattedDate = useMemo(() => {
    if (!entry) return "";
    const parsed = new Date(entry.date);
    if (Number.isNaN(parsed.getTime())) return entry.date;
    return parsed.toLocaleDateString(undefined, {
      year: "numeric",
      month: "long",
      day: "numeric",
    });
  }, [entry]);

  const handleClose = async () => {
    if (currentVersion) {
      try {
        await setLastSeenAppVersion(currentVersion);
      } catch { }
    }
    onClose();
  };

  useEffect(() => {
    if (!isOpen) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === "Escape") {
        event.preventDefault();
        void handleClose();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [isOpen, currentVersion]);

  if (typeof document === "undefined") return null;

  return createPortal(
    <AnimatePresence>
      {isOpen && (
        <>
          <motion.div
            className="fixed inset-0 z-[100] bg-black/45 backdrop-blur-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            onClick={() => void handleClose()}
            aria-hidden
          />

          <motion.aside
            role="dialog"
            aria-modal="true"
            aria-labelledby="whats-new-title"
            className={cn(
              "fixed bottom-0 left-0 top-[var(--titlebar-h,0px)] z-[110] flex flex-col",
              "w-full bg-surface text-fg shadow-2xl",
              "border-r border-fg/10",
              "lg:w-[480px] xl:w-[560px]",
            )}
            initial={{ x: "-100%" }}
            animate={{ x: 0 }}
            exit={{ x: "-100%" }}
            transition={{ type: "spring", damping: 32, stiffness: 320, mass: 0.8 }}
          >
            <header className="shrink-0 border-b border-fg/8 px-5 pb-4 pt-[env(safe-area-inset-top)] sm:px-6">
              <div className="flex items-center gap-3">
                <div className="flex h-8 w-8 items-center justify-center rounded-lg border border-accent/30 bg-accent/15">
                  <Sparkles className="h-4 w-4 text-accent" />
                </div>
                <div className="min-w-0 flex-1">
                  <p className="text-[10px] font-semibold uppercase tracking-[0.28em] text-accent/80">
                    {t("updates.whatsNew.welcomeBack")}
                  </p>
                  <h2
                    id="whats-new-title"
                    className="text-base font-semibold tracking-tight text-fg"
                  >
                    {t("updates.whatsNew.title")}
                  </h2>
                </div>
              </div>
              {entry ? (
                <p className="mt-3 flex flex-wrap items-baseline gap-x-2 gap-y-0.5 text-[11px] text-fg/45">
                  <span className="font-mono text-fg/70">v{entry.version}</span>
                  <span className="text-fg/25">·</span>
                  <span>{formattedDate}</span>
                </p>
              ) : null}
            </header>

            <div className="min-h-0 flex-1 overflow-y-auto px-5 py-5 sm:px-6">
              {entry ? (
                <div className="space-y-6">
                  {entry.title || entry.description ? (
                    <div className="space-y-2">
                      {entry.title ? (
                        <h3 className="text-lg font-semibold tracking-tight text-fg">
                          {entry.title}
                        </h3>
                      ) : null}
                      {entry.description ? (
                        <p className="text-[13px] leading-relaxed text-fg/65">
                          {entry.description}
                        </p>
                      ) : null}
                    </div>
                  ) : null}
                  {grouped.map(({ type, items }) => (
                    <section key={type}>
                      <div className="mb-2 flex items-center gap-2">
                        <span className={cn("h-1.5 w-1.5 rounded-full", TYPE_DOT[type])} />
                        <p className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/55">
                          {t(TYPE_LABEL[type])}
                        </p>
                        <span className="font-mono text-[10px] tabular-nums text-fg/30">
                          {items.length.toString().padStart(2, "0")}
                        </span>
                      </div>
                      <ul className="space-y-1.5">
                        {items.map((description, idx) => (
                          <li
                            key={idx}
                            className="flex gap-2.5 text-[13.5px] leading-relaxed text-fg/82"
                          >
                            <span aria-hidden className="mt-2 h-px w-2.5 shrink-0 bg-fg/25" />
                            <span>{description}</span>
                          </li>
                        ))}
                      </ul>
                    </section>
                  ))}
                </div>
              ) : (
                <p className="text-sm text-fg/55">{t("updates.whatsNew.noNotes")}</p>
              )}
            </div>

            <footer className="shrink-0 border-t border-fg/8 bg-fg/3 px-5 pb-[env(safe-area-inset-bottom)] pt-4 sm:px-6">
              <button
                type="button"
                onClick={() => void handleClose()}
                className={cn(
                  "inline-flex w-full items-center justify-center gap-2 rounded-lg",
                  "border border-accent/40 bg-accent/20 px-4 py-2.5",
                  "text-sm font-semibold text-accent/90 transition",
                  "hover:border-accent/60 hover:bg-accent/30 active:scale-[0.99]",
                  "focus:outline-none focus-visible:ring-2 focus-visible:ring-accent/40",
                )}
              >
                {t("common.buttons.continue")}
                <ArrowRight className="h-4 w-4" />
              </button>
            </footer>
          </motion.aside>
        </>
      )}
    </AnimatePresence>,
    document.body,
  );
}

export const WHATS_NEW_OPEN_EVENT = "whatsnew:open";
