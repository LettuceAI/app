import { useMemo } from "react";
import { History } from "lucide-react";
import type { CompanionTimeOverride, Session } from "../../../../core/storage/schemas";
import { useI18n } from "../../../../core/i18n/context";
import type { TranslationKey } from "../../../../core/i18n/context";
import { cn, interactive, radius } from "../../../design-tokens";
import { DateTimePicker } from "../../../components/DateTimePicker";
import { useCompanionTimeOverrideEditor } from "../utils/companionTimeOverride";

const MODE_OPTIONS = [
  { mode: "off", label: "chats.timeOverride.modeLive" },
  { mode: "frozen", label: "chats.timeOverride.modeFrozen" },
  { mode: "ticking", label: "chats.timeOverride.modeTicking" },
] as const satisfies ReadonlyArray<{ mode: string; label: TranslationKey }>;

interface CompanionTimeOverrideCardProps {
  session: Session | null;
  onApply: (override: CompanionTimeOverride | null) => void | Promise<void>;
  disabled?: boolean;
}

export function CompanionTimeOverrideCard({
  session,
  onApply,
  disabled,
}: CompanionTimeOverrideCardProps) {
  const { t } = useI18n();
  const override = session?.companionState?.preferences?.timeOverride;
  const canEdit = !disabled && !!session;
  const {
    activeMode,
    selectedMode,
    editing,
    beginEdit,
    selectLive,
    apply,
    cancel,
    draftMs,
    setDraftMs,
    shownMs,
    nowMs,
    isOverridden,
  } = useCompanionTimeOverrideEditor(override, onApply, canEdit);

  const formatter = useMemo(
    () =>
      new Intl.DateTimeFormat(undefined, {
        weekday: "short",
        month: "short",
        day: "numeric",
        hour: "2-digit",
        minute: "2-digit",
      }),
    [],
  );

  return (
    <div
      className={cn(
        "flex flex-col gap-3 rounded-xl border px-4 py-3",
        !session
          ? "border-white/5 bg-[#0c0d13]/50 opacity-50"
          : "border-white/10 bg-[#0c0d13]/85",
      )}
    >
      <div className="flex items-start gap-3">
        <div
          className={cn(
            "mt-0.5 flex h-8 w-8 shrink-0 items-center justify-center border border-fg/15 bg-fg/10 text-fg/75",
            radius.full,
          )}
        >
          <History className="h-4 w-4" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center justify-between gap-2">
            <p className="text-sm font-semibold text-white">{t("chats.timeOverride.title")}</p>
            <span
              className={cn(
                "rounded-full px-1.5 py-0.5 text-[10px] font-semibold uppercase tracking-wide",
                isOverridden ? "bg-accent/15 text-accent" : "text-white/35",
              )}
            >
              {isOverridden
                ? activeMode === "frozen"
                  ? t("chats.timeOverride.badgeFrozen")
                  : t("chats.timeOverride.badgeCustom")
                : t("chats.timeOverride.badgeLive")}
            </span>
          </div>
          <p className="mt-1 text-xs text-white/50">
            {t("chats.timeOverride.description")}
          </p>
          <p className="mt-1.5 text-xs tabular-nums text-white/70">
            {formatter.format(shownMs)}
            {isOverridden && (
              <span className="text-white/35">
                {" "}
                {t("chats.timeOverride.realClock", { time: formatter.format(nowMs) })}
              </span>
            )}
          </p>
        </div>
      </div>

      <div className="flex gap-1.5">
        {MODE_OPTIONS.map((opt) => (
          <button
            key={opt.mode}
            type="button"
            disabled={!canEdit}
            onClick={() => (opt.mode === "off" ? selectLive() : beginEdit(opt.mode))}
            className={cn(
              "flex-1 border px-2 py-1.5 text-xs font-medium",
              radius.md,
              interactive.transition.default,
              (editing ? selectedMode : activeMode) === opt.mode
                ? "border-accent/40 bg-accent/15 text-accent"
                : "border-white/10 bg-[#0c0d13]/85 text-white/60 hover:border-white/20 hover:text-white/80",
              !canEdit && "cursor-not-allowed opacity-50",
            )}
          >
            {t(opt.label)}
          </button>
        ))}
      </div>

      {canEdit && !editing && isOverridden && (
        <button
          type="button"
          onClick={() => beginEdit(activeMode === "frozen" ? "frozen" : "ticking")}
          className={cn(
            "self-start text-xs font-medium text-accent hover:brightness-110",
            interactive.transition.default,
          )}
        >
          {t("chats.timeOverride.changeTime")}
        </button>
      )}

      {editing && selectedMode !== "off" && (
        <div className="flex flex-col gap-2">
          <DateTimePicker valueMs={draftMs} onChange={setDraftMs} />
          <div className="flex items-center gap-2">
            <button
              type="button"
              onClick={() => setDraftMs(Date.now())}
              className={cn(
                "border border-white/10 bg-[#0c0d13]/85 px-3 py-2 text-xs font-medium text-white/60 hover:border-white/20 hover:text-white/80",
                radius.lg,
                interactive.transition.default,
              )}
            >
              {t("chats.timeOverride.now")}
            </button>
            <button
              type="button"
              onClick={cancel}
              className={cn(
                "ml-auto border border-white/10 bg-[#0c0d13]/85 px-3 py-2 text-xs font-medium text-white/60 hover:border-white/20 hover:text-white/80",
                radius.lg,
                interactive.transition.default,
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              type="button"
              onClick={apply}
              className={cn(
                "bg-accent px-3 py-2 text-xs font-semibold text-black",
                radius.lg,
                interactive.transition.default,
                interactive.active.scale,
                "hover:brightness-110",
              )}
            >
              {selectedMode === "frozen"
                ? t("chats.timeOverride.freeze")
                : t("chats.timeOverride.set")}
            </button>
          </div>
        </div>
      )}
    </div>
  );
}
