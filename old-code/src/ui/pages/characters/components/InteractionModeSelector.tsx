import { useEffect, useState } from "react";
import { useNavigate, useLocation } from "react-router-dom";
import { BookOpen, Check, ChevronRight, MessageCircleHeart, Sparkles } from "lucide-react";
import type { CharacterMode } from "../../../../core/storage/schemas";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import { cn, interactive, radius, typography } from "../../../design-tokens";
import { CompanionSetupGuide } from "./CompanionSetupGuide";
import { useCompanionRequirements } from "../hooks/useCompanionRequirements";
import { buildModelRequirementsQueuePath } from "../../../modelRequirements";

const EXPLAIN_PENDING_KEY = "companion-setup-explain-pending";

interface InteractionModeSelectorProps {
  mode: CharacterMode;
  onChange: (mode: CharacterMode) => void;
  disabled?: boolean;
  onBeforeNavigateAway?: () => void;
}

const modes: Array<{
  id: CharacterMode;
  titleKey: TranslationKey;
  subtitleKey: TranslationKey;
  icon: typeof BookOpen;
}> = [
  {
    id: "roleplay",
    titleKey: "characters.interactionMode.roleplayTitle",
    subtitleKey: "characters.interactionMode.roleplaySubtitle",
    icon: BookOpen,
  },
  {
    id: "companion",
    titleKey: "characters.interactionMode.companionTitle",
    subtitleKey: "characters.interactionMode.companionSubtitle",
    icon: MessageCircleHeart,
  },
];

export function InteractionModeSelector({
  mode,
  onChange,
  disabled = false,
  onBeforeNavigateAway,
}: InteractionModeSelectorProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const location = useLocation();
  const { loading, missing, refresh } = useCompanionRequirements();
  const [setupOpen, setSetupOpen] = useState(false);

  useEffect(() => {
    if (loading) return;
    if (sessionStorage.getItem(EXPLAIN_PENDING_KEY) !== "true") return;
    sessionStorage.removeItem(EXPLAIN_PENDING_KEY);
    if (missing.length === 0) setSetupOpen(true);
  }, [loading, missing.length]);

  const handleSelect = (next: CharacterMode) => {
    onChange(next);
    if (next === "companion") {
      void (async () => {
        const result = await refresh();
        if (result.length > 0) setSetupOpen(true);
      })();
    }
  };

  const handleDownload = () => {
    sessionStorage.setItem(EXPLAIN_PENDING_KEY, "true");
    setSetupOpen(false);
    onBeforeNavigateAway?.();
    const returnTo = `${location.pathname}${location.search}`;
    navigate(buildModelRequirementsQueuePath(missing, returnTo));
  };

  return (
    <div className="space-y-2">
      <div className="flex items-center justify-between gap-2">
        <div>
          <div
            className={cn(
              typography.label.size,
              typography.label.weight,
              typography.label.tracking,
              "uppercase text-fg/70",
            )}
          >
            {t("characters.interactionMode.sectionLabel")}
          </div>
          <p className={cn(typography.bodySmall.size, "mt-1 text-fg/45")}>
            {t("characters.interactionMode.sectionHint")}
          </p>
        </div>
      </div>

      <div className="grid gap-2 sm:grid-cols-2">
        {modes.map((option) => {
          const Icon = option.icon;
          const selected = mode === option.id;
          return (
            <button
              key={option.id}
              type="button"
              disabled={disabled}
              onClick={() => handleSelect(option.id)}
              aria-pressed={selected}
              className={cn(
                "group relative overflow-hidden rounded-xl border px-4 py-3.5 text-left",
                interactive.transition.default,
                interactive.active.scale,
                disabled && "cursor-not-allowed opacity-60",
                selected
                  ? "border-accent/40 bg-accent/10 shadow-[0_0_0_1px_rgba(16,185,129,0.25),0_8px_24px_-12px_rgba(16,185,129,0.35)]"
                  : "border-fg/10 bg-fg/[0.03] hover:border-fg/20 hover:bg-fg/5",
              )}
            >
              {selected && (
                <div
                  className="pointer-events-none absolute inset-0 opacity-80"
                  style={{
                    background:
                      "radial-gradient(circle at 12% 0%, rgba(16,185,129,0.18) 0%, transparent 55%)",
                  }}
                />
              )}

              {selected && (
                <span className="pointer-events-none absolute right-3 top-3 rounded-md border border-accent/35 bg-accent/15 px-1.5 py-0.5 text-[9px] font-semibold uppercase leading-none tracking-[0.2em] text-accent/90">
                  {t("characters.interactionMode.activeBadge")}
                </span>
              )}

              <div className="relative flex items-start gap-3">
                <div
                  className={cn(
                    "flex h-9 w-9 shrink-0 items-center justify-center border",
                    radius.lg,
                    interactive.transition.default,
                    selected
                      ? "border-accent/40 bg-accent/15 text-accent shadow-lg shadow-accent/15"
                      : "border-fg/10 bg-fg/5 text-fg/45 group-hover:border-fg/20 group-hover:text-fg/70",
                  )}
                >
                  <Icon className="h-4 w-4" />
                </div>
                <div className="min-w-0 pr-12">
                  <p
                    className={cn(
                      "text-sm font-semibold transition-colors",
                      selected ? "text-fg" : "text-fg/85 group-hover:text-fg",
                    )}
                  >
                    {t(option.titleKey)}
                  </p>
                  <p
                    className={cn(
                      "mt-1 text-xs leading-relaxed transition-colors",
                      selected ? "text-fg/60" : "text-fg/45",
                    )}
                  >
                    {t(option.subtitleKey)}
                  </p>
                </div>
              </div>
            </button>
          );
        })}
      </div>

      {mode === "companion" && (
        <button
          type="button"
          onClick={() => setSetupOpen(true)}
          className={cn(
            "flex w-full items-center gap-2.5 border px-3.5 py-2.5 text-left",
            radius.lg,
            interactive.transition.fast,
            missing.length > 0
              ? "border-accent/30 bg-accent/10 hover:bg-accent/15"
              : "border-fg/10 bg-fg/[0.03] hover:bg-fg/5",
          )}
        >
          <div
            className={cn(
              "flex h-7 w-7 shrink-0 items-center justify-center border",
              radius.md,
              missing.length > 0
                ? "border-accent/30 bg-accent/15 text-accent"
                : "border-emerald-400/30 bg-emerald-500/10 text-emerald-300",
            )}
          >
            {missing.length > 0 ? (
              <Sparkles className="h-3.5 w-3.5" />
            ) : (
              <Check className="h-3.5 w-3.5" />
            )}
          </div>
          <div className="min-w-0 flex-1">
            <p className="text-sm font-medium text-fg">
              {t("characters.companionSetup.triggerTitle")}
            </p>
            <p className={cn(typography.caption.size, "text-fg/50")}>
              {loading
                ? t("characters.companionSetup.modelsChecking")
                : missing.length > 0
                  ? missing.length === 1
                    ? t("characters.companionSetup.triggerNeedsOne")
                    : t("characters.companionSetup.triggerNeedsMany", { count: missing.length })
                  : t("characters.companionSetup.triggerReady")}
            </p>
          </div>
          <ChevronRight className="h-4 w-4 shrink-0 text-fg/30" />
        </button>
      )}

      <CompanionSetupGuide
        isOpen={setupOpen}
        onClose={() => setSetupOpen(false)}
        loading={loading}
        missing={missing}
        onDownload={handleDownload}
      />
    </div>
  );
}
