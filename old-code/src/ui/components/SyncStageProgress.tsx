import { Check, Database } from "lucide-react";

import { useI18n, type TranslationKey } from "../../core/i18n/context";
import { cn } from "../design-tokens";

const STAGES = [
  { id: "verify", label: "sync.progress.stages.verify" },
  { id: "compare", label: "sync.progress.stages.compare" },
  { id: "data", label: "sync.progress.stages.data" },
  { id: "files", label: "sync.progress.stages.files" },
] as const satisfies ReadonlyArray<{ id: string; label: TranslationKey }>;

const PHASES: Record<string, { stage: number; label: TranslationKey }> = {
  "Verifying devices": { stage: 0, label: "sync.progress.phases.verifyingDevices" },
  "Comparing changes": { stage: 1, label: "sync.progress.phases.comparingChanges" },
  "Exchanging changes": { stage: 2, label: "sync.progress.phases.exchangingChanges" },
  "Applying changes": { stage: 2, label: "sync.progress.phases.applyingChanges" },
  "Separating chat branches": {
    stage: 2,
    label: "sync.progress.phases.separatingChatBranches",
  },
  "Waiting for local database": {
    stage: 2,
    label: "sync.progress.phases.waitingForDatabase",
  },
  "Comparing files": { stage: 3, label: "sync.progress.phases.comparingFiles" },
  "Exchanging files": { stage: 3, label: "sync.progress.phases.exchangingFiles" },
};

interface SyncStageProgressProps {
  phase: string;
  surface?: "default" | "onboarding";
  databaseWaitAttempt?: number | null;
  databaseWaitTotal?: number | null;
  databaseWaitMs?: number | null;
}

export function SyncStageProgress({
  phase,
  surface = "default",
  databaseWaitAttempt,
  databaseWaitTotal,
  databaseWaitMs,
}: SyncStageProgressProps) {
  const { t } = useI18n();
  const resolved = PHASES[phase];
  const currentStage = resolved?.stage ?? 0;
  const phaseLabel = resolved ? t(resolved.label) : phase;
  const onboarding = surface === "onboarding";
  const waitingForDatabase = phase === "Waiting for local database";
  const retryDelay =
    typeof databaseWaitMs === "number"
      ? databaseWaitMs < 1000
        ? `${databaseWaitMs} ms`
        : `${databaseWaitMs / 1000} s`
      : null;

  return (
    <div
      className={cn(
        "border-t px-4 py-3.5",
        onboarding ? "border-white/10" : "border-fg/8",
      )}
    >
      {waitingForDatabase && (
        <div
          className={cn(
            "mb-3 flex items-start gap-2.5 rounded-lg border px-3 py-2.5",
            onboarding
              ? "border-amber-300/20 bg-amber-300/10"
              : "border-warning/20 bg-warning/[0.07]",
          )}
        >
          <Database
            className={cn(
              "mt-0.5 h-3.5 w-3.5 shrink-0 animate-pulse",
              onboarding ? "text-amber-200/80" : "text-warning/80",
            )}
          />
          <div className="min-w-0">
            <p
              className={cn(
                "text-[11px] font-semibold",
                onboarding ? "text-amber-100/90" : "text-warning/90",
              )}
            >
              {t("sync.progress.databaseWait.title")}
            </p>
            <p
              className={cn(
                "mt-0.5 text-[10px] leading-relaxed",
                onboarding ? "text-amber-100/55" : "text-fg/50",
              )}
            >
              {t("sync.progress.databaseWait.description")}
            </p>
            {typeof databaseWaitAttempt === "number" &&
              typeof databaseWaitTotal === "number" &&
              retryDelay && (
                <p
                  className={cn(
                    "mt-1 font-mono text-[9px] tabular-nums",
                    onboarding ? "text-amber-200/60" : "text-warning/65",
                  )}
                >
                  {t("sync.progress.databaseWait.retry", {
                    current: databaseWaitAttempt,
                    total: databaseWaitTotal,
                    delay: retryDelay,
                  })}
                </p>
              )}
          </div>
        </div>
      )}

      <div className="mb-3 min-w-0">
        <p
          className={cn(
            "text-[9px] font-semibold uppercase tracking-[0.16em]",
            onboarding ? "text-white/40" : "text-fg/35",
          )}
        >
          {t("sync.progress.stage", {
            current: currentStage + 1,
            total: STAGES.length,
          })}
        </p>
        <p
          className={cn(
            "mt-0.5 truncate text-[11px] font-medium",
            onboarding ? "text-white/75" : "text-fg/70",
          )}
        >
          {phaseLabel}
        </p>
      </div>

      <div className="relative grid grid-cols-4">
        <div
          className={cn(
            "absolute left-[12.5%] right-[12.5%] top-[7px] h-px",
            onboarding ? "bg-white/15" : "bg-fg/12",
          )}
        />
        <div
          className={cn(
            "absolute left-[12.5%] top-[7px] h-px bg-accent/75 transition-[width] duration-500 ease-out",
            onboarding && "bg-emerald-400/80",
          )}
          style={{ width: `${(currentStage / (STAGES.length - 1)) * 75}%` }}
        />

        {STAGES.map((stage, index) => {
          const completed = index < currentStage;
          const active = index === currentStage;

          return (
            <div
              key={stage.id}
              aria-current={active ? "step" : undefined}
              className="relative flex min-w-0 flex-col items-center"
            >
              <span
                className={cn(
                  "relative z-10 flex h-[15px] w-[15px] items-center justify-center rounded-full border",
                  completed &&
                    (onboarding
                      ? "border-emerald-300/70 bg-emerald-400 text-black"
                      : "border-accent/70 bg-accent text-bg"),
                  active &&
                    (onboarding
                      ? "border-emerald-300 bg-emerald-400/20 shadow-[0_0_0_3px_rgba(52,211,153,0.12)]"
                      : "border-accent bg-accent/20 shadow-[0_0_0_3px_hsl(var(--accent)/0.1)]"),
                  !completed &&
                    !active &&
                    (onboarding
                      ? "border-white/20 bg-black/80"
                      : "border-fg/15 bg-bg"),
                )}
              >
                {completed ? (
                  <Check className="h-2.5 w-2.5 stroke-[3]" />
                ) : active ? (
                  <span
                    className={cn(
                      "h-1.5 w-1.5 animate-pulse rounded-full",
                      onboarding ? "bg-emerald-300" : "bg-accent",
                    )}
                  />
                ) : null}
              </span>
              <span
                className={cn(
                  "mt-1.5 truncate px-1 text-[9px] font-medium",
                  completed || active
                    ? onboarding
                      ? "text-white/75"
                      : "text-fg/70"
                    : onboarding
                      ? "text-white/30"
                      : "text-fg/30",
                )}
              >
                {t(stage.label)}
              </span>
            </div>
          );
        })}
      </div>
    </div>
  );
}
