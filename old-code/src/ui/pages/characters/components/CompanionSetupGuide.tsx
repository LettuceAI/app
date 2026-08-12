import { BookOpen, Check, Download, Heart, Loader2, MessageCircleHeart, Waypoints } from "lucide-react";
import type { ModelRequirement } from "../../../modelRequirements";
import { BottomMenu } from "../../../components/BottomMenu";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import { openDocs } from "../../../../core/utils/docs";
import { cn, interactive, radius, typography } from "../../../design-tokens";

interface CompanionSetupGuideProps {
  isOpen: boolean;
  onClose: () => void;
  loading: boolean;
  missing: ModelRequirement[];
  onDownload: () => void;
}

const CONCEPTS: Array<{
  icon: typeof Heart;
  titleKey: TranslationKey;
  bodyKey: TranslationKey;
}> = [
  {
    icon: Heart,
    titleKey: "characters.companionSetup.conceptEmotionTitle",
    bodyKey: "characters.companionSetup.conceptEmotionBody",
  },
  {
    icon: MessageCircleHeart,
    titleKey: "characters.companionSetup.conceptRelationshipTitle",
    bodyKey: "characters.companionSetup.conceptRelationshipBody",
  },
  {
    icon: Waypoints,
    titleKey: "characters.companionSetup.conceptMemoryTitle",
    bodyKey: "characters.companionSetup.conceptMemoryBody",
  },
];

function InstallPhase({
  loading,
  missing,
  onDownload,
}: {
  loading: boolean;
  missing: ModelRequirement[];
  onDownload: () => void;
}) {
  const { t } = useI18n();
  return (
    <div className="space-y-4">
      <p className={cn(typography.bodySmall.size, "text-fg/60")}>
        {t("characters.companionSetup.modelsBody")}
      </p>

      {loading ? (
        <p className={cn(typography.bodySmall.size, "inline-flex items-center gap-1.5 text-fg/45")}>
          <Loader2 className="h-3.5 w-3.5 animate-spin" />
          {t("characters.companionSetup.modelsChecking")}
        </p>
      ) : (
        <>
          <div className="space-y-1.5">
            {missing.map((req) => {
              const Icon = req.icon;
              return (
                <div
                  key={req.kind}
                  className={cn(
                    "flex items-start gap-2.5 border border-fg/10 bg-fg/[0.03] px-3 py-2.5",
                    radius.md,
                  )}
                >
                  <Icon className="mt-0.5 h-4 w-4 shrink-0 text-fg/55" />
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center justify-between gap-2">
                      <span className="text-sm font-medium text-fg">{req.title}</span>
                      <span className="shrink-0 text-[10px] tabular-nums text-fg/40">
                        {req.approxSize}
                      </span>
                    </div>
                    <p className="mt-0.5 text-xs leading-snug text-fg/50">{req.subtitle}</p>
                  </div>
                </div>
              );
            })}
          </div>

          <button
            type="button"
            onClick={onDownload}
            className={cn(
              "inline-flex w-full items-center justify-center gap-1.5 border border-accent/40 bg-accent/20 px-3 py-3 text-sm font-semibold text-accent",
              radius.md,
              interactive.transition.fast,
              interactive.active.scale,
              "hover:border-accent/55 hover:bg-accent/30",
            )}
          >
            <Download className="h-4 w-4" />
            {missing.length === 1
              ? t("characters.companionSetup.downloadOne")
              : t("characters.companionSetup.downloadMany", { count: missing.length })}
          </button>
        </>
      )}
    </div>
  );
}

function ExplainPhase() {
  const { t } = useI18n();
  return (
    <div className="space-y-5">
      <div
        className={cn(
          "flex items-center gap-2.5 border border-emerald-400/25 bg-emerald-500/10 px-3 py-2.5",
          radius.md,
        )}
      >
        <Check className="h-4 w-4 shrink-0 text-emerald-300" />
        <p className="text-sm font-medium text-emerald-200">
          {t("characters.companionSetup.modelsDoneTitle")}
        </p>
      </div>

      <p className={cn(typography.bodySmall.size, "text-fg/60")}>
        {t("characters.companionSetup.explainIntro")}
      </p>

      <div className="space-y-1.5">
        <div className="flex flex-wrap items-center gap-2">
          <p className="text-sm font-semibold text-fg">
            {t("characters.companionSetup.soulTitle")}
          </p>
          <span className="rounded-md border border-fg/15 bg-fg/5 px-1.5 py-0.5 text-[9px] font-semibold uppercase leading-none tracking-[0.18em] text-fg/45">
            {t("characters.companionSetup.stepOptional")}
          </span>
        </div>
        <p className={cn(typography.bodySmall.size, "text-fg/55")}>
          {t("characters.companionSetup.soulBody")}
        </p>
      </div>

      <div className="space-y-3">
        <p className="text-sm font-semibold text-fg">
          {t("characters.companionSetup.conceptsTitle")}
        </p>
        <div className="space-y-2.5">
          {CONCEPTS.map((concept) => {
            const Icon = concept.icon;
            return (
              <div key={concept.titleKey} className="flex items-start gap-2.5">
                <div
                  className={cn(
                    "flex h-7 w-7 shrink-0 items-center justify-center border border-fg/10 bg-fg/5 text-fg/55",
                    radius.md,
                  )}
                >
                  <Icon className="h-3.5 w-3.5" />
                </div>
                <div className="min-w-0">
                  <p className="text-xs font-semibold text-fg/85">{t(concept.titleKey)}</p>
                  <p className="mt-0.5 text-[11px] leading-snug text-fg/50">{t(concept.bodyKey)}</p>
                </div>
              </div>
            );
          })}
        </div>
      </div>

      <div className={cn("border border-fg/10 bg-fg/[0.03] px-3 py-2.5", radius.md)}>
        <p className="text-xs font-semibold text-fg/80">
          {t("characters.companionSetup.whereTitle")}
        </p>
        <p className="mt-1 text-[11px] leading-relaxed text-fg/50">
          {t("characters.companionSetup.whereBody")}
        </p>
      </div>

      <button
        type="button"
        onClick={() => void openDocs("companionMode")}
        className="inline-flex items-center gap-1.5 text-xs font-medium text-fg/40 transition active:scale-95 hover:text-fg/70"
      >
        <BookOpen className="h-3.5 w-3.5" />
        {t("common.buttons.learnMore")}
      </button>
    </div>
  );
}

export function CompanionSetupGuide({
  isOpen,
  onClose,
  loading,
  missing,
  onDownload,
}: CompanionSetupGuideProps) {
  const { t } = useI18n();
  const modelsReady = !loading && missing.length === 0;

  return (
    <BottomMenu
      isOpen={isOpen}
      onClose={onClose}
      title={
        modelsReady
          ? t("characters.companionSetup.headingReady")
          : t("characters.companionSetup.heading")
      }
    >
      {modelsReady ? (
        <ExplainPhase />
      ) : (
        <InstallPhase loading={loading} missing={missing} onDownload={onDownload} />
      )}
    </BottomMenu>
  );
}
