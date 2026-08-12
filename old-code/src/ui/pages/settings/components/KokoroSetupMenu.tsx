import { useEffect, useMemo, useState } from "react";
import { AlertTriangle, Check, Cpu, Download, Loader2, Sparkles } from "lucide-react";

import { BottomMenu } from "../../../components/BottomMenu";
import { cn } from "../../../design-tokens";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import { useDownloadQueue } from "../../../../core/downloads/DownloadQueueContext";
import {
  kokoroDefaultAssetRoot,
  kokoroInstallModel,
  kokoroInstallVoices,
  kokoroSupportedVariants,
  upsertAudioProvider,
  type AudioProvider,
  type KokoroModelVariant,
  type KokoroSupportedVariant,
} from "../../../../core/storage/audioProviders";

export const STARTER_PACK_VOICE_IDS = ["af_heart", "am_adam", "bf_emma", "bm_george"];

const VARIANT_COPY = {
  int8: {
    descriptionKey: "voices.extra.kokoroSetup.variantInt8Description",
    tagKey: "voices.extra.kokoroSetup.variantInt8Tag",
  },
  fp16: {
    descriptionKey: "voices.extra.kokoroSetup.variantFp16Description",
    tagKey: "voices.extra.kokoroSetup.variantFp16Tag",
  },
  fp32: {
    descriptionKey: "voices.extra.kokoroSetup.variantFp32Description",
    tagKey: "voices.extra.kokoroSetup.variantFp32Tag",
  },
} as const satisfies Record<
  KokoroModelVariant,
  { descriptionKey: TranslationKey; tagKey: TranslationKey }
>;

export function KokoroSetupMenu({
  provider,
  onClose,
  onStarted,
}: {
  provider: AudioProvider | null;
  onClose: () => void;
  onStarted?: () => void;
}) {
  const { t } = useI18n();
  const { queue } = useDownloadQueue();
  const [variants, setVariants] = useState<KokoroSupportedVariant[]>([]);
  const [picked, setPicked] = useState<KokoroModelVariant | undefined>(undefined);
  const [includeStarter, setIncludeStarter] = useState(true);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    if (!provider) return;
    let active = true;
    void kokoroSupportedVariants()
      .then((list) => {
        if (!active) return;
        setVariants(list);
        setPicked(
          provider.kokoroVariant ?? list.find((v) => v.id === "fp16")?.id ?? list[0]?.id,
        );
      })
      .catch((e) => active && setError(e instanceof Error ? e.message : String(e)));
    return () => {
      active = false;
    };
  }, [provider]);

  const assetRoot = provider?.assetRoot?.trim() ?? "";
  const downloading = useMemo(
    () =>
      queue.some(
        (item) =>
          item.queueKind === "kokoro" &&
          item.assetRoot === assetRoot &&
          (item.status === "downloading" || item.status === "queued"),
      ),
    [queue, assetRoot],
  );

  const handleDownload = async () => {
    if (!provider || !picked || busy) return;
    setBusy(true);
    setError(null);
    try {
      let next = provider;
      let root = next.assetRoot?.trim() ?? "";
      if (!root) {
        root = await kokoroDefaultAssetRoot();
        next = { ...next, assetRoot: root };
      }
      if (next.kokoroVariant !== picked) {
        next = { ...next, kokoroVariant: picked };
      }
      if (next !== provider) {
        await upsertAudioProvider(next);
      }
      await kokoroInstallModel(root, picked);
      if (includeStarter) {
        await kokoroInstallVoices(root, STARTER_PACK_VOICE_IDS);
      }
      onStarted?.();
      onClose();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <BottomMenu
      isOpen={!!provider}
      onClose={() => {
        if (!busy) onClose();
      }}
      title={t("voices.extra.kokoroSetup.title")}
    >
      <div className="space-y-3">
        <p className="text-[12px] leading-relaxed text-fg/55">
          {t("voices.extra.kokoroSetup.intro")}
        </p>

        <div className="space-y-1.5">
          {variants.map((v) => {
            const copy = VARIANT_COPY[v.id];
            const isSelected = picked === v.id;
            return (
              <button
                key={v.id}
                onClick={() => setPicked(v.id)}
                disabled={busy || downloading}
                className={cn(
                  "group relative w-full overflow-hidden rounded-xl border px-3.5 py-3 text-left transition disabled:opacity-60",
                  isSelected
                    ? "border-accent/35 bg-accent/8"
                    : "border-fg/10 bg-fg/3 hover:border-fg/20 hover:bg-fg/5",
                )}
              >
                <div className="flex items-center gap-3">
                  <span
                    className={cn(
                      "flex h-9 w-9 shrink-0 items-center justify-center rounded-lg",
                      isSelected ? "bg-accent/15 text-accent" : "bg-fg/8 text-fg/55",
                    )}
                  >
                    <Cpu size={15} />
                  </span>
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-2">
                      <span className="truncate text-[13px] font-medium text-fg">{v.label}</span>
                      {copy && (
                        <span
                          className={cn(
                            "shrink-0 rounded-full border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider",
                            v.id === "fp16"
                              ? "border-accent/30 bg-accent/10 text-accent/85"
                              : "border-fg/15 bg-fg/5 text-fg/55",
                          )}
                        >
                          {t(copy.tagKey)}
                        </span>
                      )}
                    </div>
                    {copy && (
                      <p className="mt-0.5 truncate text-[11.5px] text-fg/45">
                        {t(copy.descriptionKey)}
                      </p>
                    )}
                  </div>
                  <span className="shrink-0 text-[12px] font-semibold tabular-nums text-fg/75">
                    {v.sizeMb} {t("common.units.mb")}
                  </span>
                  <span
                    className={cn(
                      "flex h-5 w-5 shrink-0 items-center justify-center rounded-full border transition",
                      isSelected
                        ? "border-accent/50 bg-accent/15 text-accent"
                        : "border-fg/15 text-transparent",
                    )}
                  >
                    <Check size={11} strokeWidth={3} />
                  </span>
                </div>
              </button>
            );
          })}
        </div>

        <button
          onClick={() => setIncludeStarter((prev) => !prev)}
          disabled={busy || downloading}
          className={cn(
            "flex w-full items-center gap-3 rounded-xl border px-3.5 py-3 text-left transition disabled:opacity-60",
            includeStarter
              ? "border-accent/35 bg-accent/8"
              : "border-fg/10 bg-fg/3 hover:border-fg/20 hover:bg-fg/5",
          )}
        >
          <span
            className={cn(
              "flex h-9 w-9 shrink-0 items-center justify-center rounded-lg",
              includeStarter ? "bg-accent/15 text-accent" : "bg-fg/8 text-fg/55",
            )}
          >
            <Sparkles size={15} />
          </span>
          <div className="min-w-0 flex-1">
            <p className="text-[13px] font-medium text-fg">
              {t("voices.extra.kokoroSetup.starterPackTitle")}
            </p>
            <p className="mt-0.5 text-[11.5px] text-fg/45">
              {t("voices.extra.kokoroSetup.starterPackDescription")}
            </p>
          </div>
          <span
            className={cn(
              "flex h-5 w-5 shrink-0 items-center justify-center rounded-md border transition",
              includeStarter
                ? "border-accent/50 bg-accent/15 text-accent"
                : "border-fg/15 text-transparent",
            )}
          >
            <Check size={11} strokeWidth={3} />
          </span>
        </button>

        {error && (
          <div className="flex items-start gap-2 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-danger/85" />
            <p className="flex-1 text-[12px] text-danger/85">{error}</p>
          </div>
        )}

        <button
          onClick={() => void handleDownload()}
          disabled={!picked || busy || downloading}
          className="flex w-full items-center justify-center gap-2 rounded-xl bg-accent px-4 py-2.5 text-[13px] font-semibold text-bg transition hover:brightness-110 disabled:opacity-50"
        >
          {busy || downloading ? (
            <Loader2 className="h-4 w-4 animate-spin" />
          ) : (
            <Download className="h-4 w-4" />
          )}
          {downloading
            ? t("voices.extra.kokoroSetup.downloading")
            : t("voices.extra.kokoroSetup.download")}
        </button>
      </div>
    </BottomMenu>
  );
}
