import { useCallback, useEffect, useState } from "react";
import { useNavigate } from "react-router-dom";
import {
  Brain,
  ChevronRight,
  Cpu,
  Download,
  Heart,
  Info,
  Loader2,
  RefreshCw,
  Tag,
  Trash2,
  Waypoints,
} from "lucide-react";
import { getEmbeddingModelInfo } from "../../../core/storage/repo";
import { storageBridge } from "../../../core/storage/files";
import { useI18n } from "../../../core/i18n/context";
import { cn, colors, interactive, spacing, typography } from "../../design-tokens";

type CompanionKind = "emotion" | "ner" | "router";

type ModelStatus = {
  emotion: boolean;
  ner: boolean;
  router: boolean;
  bundleComplete: boolean;
  embeddingInstalled: boolean;
  embeddingVersion: string | null;
};

const DEFAULT_STATUS: ModelStatus = {
  emotion: false,
  ner: false,
  router: false,
  bundleComplete: false,
  embeddingInstalled: false,
  embeddingVersion: null,
};

const COMPANION_RETURN = "/settings/advanced/companions";

function SectionHeader({
  icon: Icon,
  title,
  trailing,
}: {
  icon: React.ComponentType<{ size?: number }>;
  title: string;
  trailing?: React.ReactNode;
}) {
  return (
    <div className="mb-2.5 flex items-center justify-between gap-2 px-1">
      <div className="flex items-center gap-2">
        <span className="text-fg/30">
          <Icon size={12} />
        </span>
        <h2
          className={cn(
            typography.overline.size,
            typography.overline.weight,
            typography.overline.tracking,
            typography.overline.transform,
            "text-fg/40",
          )}
        >
          {title}
        </h2>
      </div>
      {trailing}
    </div>
  );
}

function ModelRow({
  icon: Icon,
  title,
  description,
  installed,
  busy,
  onInstall,
  onDelete,
}: {
  icon: React.ComponentType<{ className?: string }>;
  title: string;
  description: string;
  installed: boolean;
  busy: boolean;
  onInstall: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();
  return (
    <div
      className={cn(
        "rounded-xl border px-4 py-3 transition-colors",
        installed ? "border-fg/10 bg-fg/5" : "border-warning/20 bg-warning/5",
      )}
    >
      <div className="flex items-start gap-3">
        <div
          className={cn(
            "flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border",
            installed
              ? "border-accent/30 bg-accent/10 text-accent"
              : "border-warning/30 bg-warning/10 text-warning/80",
          )}
        >
          <Icon className="h-4 w-4" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-start justify-between gap-2">
            <p className="text-sm font-medium text-fg">{title}</p>
            <span
              className={cn(
                "shrink-0 rounded-md border px-1.5 py-0.5 text-[9px] font-semibold uppercase leading-none tracking-[0.2em]",
                installed
                  ? "border-accent/40 bg-accent/15 text-accent/80"
                  : "border-warning/40 bg-warning/15 text-warning/80",
              )}
            >
              {installed ? t("companion.modelStatus.ready") : t("companion.modelStatus.missing")}
            </span>
          </div>
          <p className="mt-1 text-[11px] leading-relaxed text-fg/50">{description}</p>

          <div className="mt-3 flex items-center gap-2">
            {installed ? (
              <button
                type="button"
                onClick={onDelete}
                disabled={busy}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-lg border border-danger/25 bg-danger/10 px-2.5 py-1 text-[11px] font-medium text-danger/80",
                  "transition hover:bg-danger/15 hover:border-danger/35",
                  busy && "pointer-events-none opacity-60",
                )}
              >
                {busy ? (
                  <Loader2 className="h-3 w-3 animate-spin" />
                ) : (
                  <Trash2 className="h-3 w-3" />
                )}
                {busy ? t("companion.modelStatus.removing") : t("companion.modelStatus.uninstall")}
              </button>
            ) : (
              <button
                type="button"
                onClick={onInstall}
                disabled={busy}
                className={cn(
                  "inline-flex items-center gap-1.5 rounded-lg border border-info/30 bg-info/10 px-2.5 py-1 text-[11px] font-medium text-info",
                  "transition hover:bg-info/15 hover:border-info/40",
                  busy && "pointer-events-none opacity-60",
                )}
              >
                <Download className="h-3 w-3" />
                {t("common.buttons.install")}
              </button>
            )}
          </div>
        </div>
      </div>
    </div>
  );
}

export function CompanionsPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const [status, setStatus] = useState<ModelStatus>(DEFAULT_STATUS);
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [busyKind, setBusyKind] = useState<CompanionKind | null>(null);
  const [actionError, setActionError] = useState<string | null>(null);

  const refresh = useCallback(async () => {
    const info = await getEmbeddingModelInfo();
    setStatus({
      emotion: info.companionEmotionInstalled ?? false,
      ner: info.companionNerInstalled ?? false,
      router: info.companionRouterInstalled ?? false,
      bundleComplete: info.installBundleComplete ?? info.installed,
      embeddingInstalled: info.installed,
      embeddingVersion:
        info.selectedSourceVersion ?? info.sourceVersion ?? info.version ?? null,
    });
  }, []);

  useEffect(() => {
    let cancelled = false;
    (async () => {
      try {
        await refresh();
      } catch (err) {
        console.error("Failed to load companion model status:", err);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [refresh]);

  const handleRefresh = async () => {
    if (refreshing) return;
    setRefreshing(true);
    try {
      await refresh();
    } catch (err) {
      console.error("Failed to refresh companion model status:", err);
    } finally {
      setRefreshing(false);
    }
  };

  const handleInstall = (kind: CompanionKind) => {
    navigate(
      `/settings/companion-download?kind=${kind}&returnTo=${encodeURIComponent(COMPANION_RETURN)}`,
    );
  };

  const handleDelete = async (kind: CompanionKind) => {
    if (busyKind) return;
    setBusyKind(kind);
    setActionError(null);
    try {
      await storageBridge.deleteCompanionModel(kind);
      await refresh();
    } catch (err) {
      setActionError(err instanceof Error ? err.message : String(err));
    } finally {
      setBusyKind(null);
    }
  };

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-6 w-6 animate-spin rounded-full border-2 border-fg/20 border-t-fg/60" />
      </div>
    );
  }

  const installedCount =
    Number(status.emotion) + Number(status.ner) + Number(status.router);
  const progressPct = (installedCount / 3) * 100;

  return (
    <div className="flex h-full flex-col">
      <main className="flex-1 overflow-y-auto px-3 pt-3 pb-6 lg:px-8">
        <div className={cn("mx-auto w-full max-w-3xl", spacing.section)}>
          {/* Status hero */}
          <section
            className={cn(
              "relative overflow-hidden rounded-2xl border px-5 py-4",
              status.bundleComplete
                ? "border-accent/25 bg-accent/8"
                : "border-warning/25 bg-warning/8",
            )}
          >
            <div
              className="pointer-events-none absolute inset-0 opacity-70"
              style={{
                background: status.bundleComplete
                  ? "radial-gradient(circle at 15% 15%, rgba(16,185,129,0.10) 0%, transparent 55%)"
                  : "radial-gradient(circle at 15% 15%, rgba(251,191,36,0.10) 0%, transparent 55%)",
              }}
            />
            <div className="relative flex items-start gap-3.5">
              <div
                className={cn(
                  "flex h-11 w-11 shrink-0 items-center justify-center rounded-xl border shadow-lg",
                  status.bundleComplete
                    ? "border-accent/40 bg-accent/15 text-accent shadow-accent/20"
                    : "border-warning/40 bg-warning/15 text-warning shadow-warning/20",
                )}
              >
                <Heart className="h-5 w-5" />
              </div>
              <div className="min-w-0 flex-1">
                <p className="text-sm font-semibold text-fg">
                  {status.bundleComplete
                    ? t("companion.runtime.readyTitle")
                    : t("companion.runtime.incompleteTitle")}
                </p>
                <p className="mt-1 text-[11px] leading-relaxed text-fg/55">
                  {status.bundleComplete
                    ? t("companion.runtime.readyDescription")
                    : t("companion.runtime.incompleteDescription", { count: installedCount })}
                </p>

                <div className="mt-3 h-1 w-full overflow-hidden rounded-full bg-fg/10">
                  <div
                    className={cn(
                      "h-full rounded-full transition-all duration-500",
                      status.bundleComplete ? "bg-accent" : "bg-warning",
                    )}
                    style={{ width: `${progressPct}%` }}
                  />
                </div>
              </div>
              <button
                type="button"
                onClick={handleRefresh}
                className={cn(
                  "shrink-0 rounded-lg border border-fg/10 bg-fg/5 p-1.5 text-fg/55",
                  "transition hover:border-fg/20 hover:bg-fg/10 hover:text-fg/80",
                  refreshing && "pointer-events-none opacity-60",
                )}
                aria-label={t("companion.runtime.refreshAria")}
              >
                <RefreshCw className={cn("h-3.5 w-3.5", refreshing && "animate-spin")} />
              </button>
            </div>
          </section>

          {actionError && (
            <div className="rounded-lg border border-danger/20 bg-danger/10 px-3 py-2 text-[11px] text-danger/80">
              {actionError}
            </div>
          )}

          {/* Memory backend */}
          <section>
            <SectionHeader icon={Brain} title={t("companion.sections.memoryBackend")} />
            <button
              type="button"
              onClick={() => navigate("/settings/advanced/memory")}
              className={cn(
                "group w-full text-left",
                "relative overflow-hidden rounded-xl border border-fg/10 bg-fg/5 px-4 py-3.5",
                "transition-all duration-300 hover:border-fg/20",
                interactive.active.scale,
                interactive.focus.ring,
              )}
            >
              <div className="flex items-start gap-3">
                <div
                  className={cn(
                    "flex h-9 w-9 shrink-0 items-center justify-center rounded-xl border",
                    "border-info/30 bg-info/10 text-info/80",
                  )}
                >
                  <Cpu className="h-4 w-4" />
                </div>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center justify-between gap-2">
                    <span className="text-sm font-medium text-fg">
                      {t("companion.sections.dynamicMemoryTitle")}
                    </span>
                    <ChevronRight className="h-4 w-4 shrink-0 text-fg/25 transition-colors group-hover:text-fg/50" />
                  </div>
                  <p className="mt-1 text-[11px] leading-relaxed text-fg/50">
                    {t("companion.sections.dynamicMemoryDescription")}
                  </p>
                </div>
              </div>
            </button>
          </section>

          {/* Local analysis models */}
          <section>
            <SectionHeader
              icon={Cpu}
              title={t("companion.sections.localAnalysisModels")}
              trailing={
                <span
                  className={cn(
                    "rounded-md border px-1.5 py-0.5 text-[10px] font-medium tabular-nums",
                    installedCount === 3
                      ? "border-accent/30 bg-accent/10 text-accent/80"
                      : "border-fg/10 bg-fg/5 text-fg/45",
                  )}
                >
                  {installedCount}/3
                </span>
              }
            />

            <div className="space-y-2.5">
              <ModelRow
                icon={Heart}
                title={t("companion.models.emotionTitle")}
                description={t("companion.models.emotionSubtitle")}
                installed={status.emotion}
                busy={busyKind === "emotion"}
                onInstall={() => handleInstall("emotion")}
                onDelete={() => handleDelete("emotion")}
              />
              <ModelRow
                icon={Tag}
                title={t("companion.models.nerTitle")}
                description={t("companion.models.nerSubtitle")}
                installed={status.ner}
                busy={busyKind === "ner"}
                onInstall={() => handleInstall("ner")}
                onDelete={() => handleDelete("ner")}
              />
              <ModelRow
                icon={Waypoints}
                title={t("companion.models.routerTitle")}
                description={t("companion.models.routerSubtitle")}
                installed={status.router}
                busy={busyKind === "router"}
                onInstall={() => handleInstall("router")}
                onDelete={() => handleDelete("router")}
              />
            </div>
          </section>

          {/* About companion mode */}
          <section>
            <SectionHeader icon={Heart} title={t("companion.sections.aboutCompanionMode")} />
            <div
              className={cn(
                "rounded-xl border px-4 py-3.5",
                colors.glass.subtle,
                "flex items-start gap-3",
              )}
            >
              <Info className="mt-0.5 h-4 w-4 shrink-0 text-fg/30" />
              <div className="space-y-2 text-[11px] leading-relaxed text-fg/55">
                <p>{t("companion.about.paragraph1")}</p>
                <p>{t("companion.about.paragraph2")}</p>
              </div>
            </div>
          </section>
        </div>
      </main>
    </div>
  );
}
