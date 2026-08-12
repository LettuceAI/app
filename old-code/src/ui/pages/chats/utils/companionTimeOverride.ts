import { useEffect, useState } from "react";
import type { CompanionTimeOverride, Session } from "../../../../core/storage/schemas";

export type OverrideMode = CompanionTimeOverride["mode"];

export function effectiveOverrideMs(
  override: CompanionTimeOverride | undefined | null,
  nowMs: number,
): number {
  if (!override || override.mode === "off") return nowMs;
  if (override.mode === "frozen") return override.anchorMs;
  return override.anchorMs + (nowMs - override.setAtMs);
}

export function sessionEffectiveNowMs(session: Session | null | undefined, nowMs: number): number {
  if (!session?.companionState?.preferences?.timeAwarenessEnabled) return nowMs;
  return effectiveOverrideMs(session.companionState.preferences.timeOverride, nowMs);
}

function floorToMinute(ms: number): number {
  return ms - (ms % 60000);
}

export function useCompanionTimeOverrideEditor(
  override: CompanionTimeOverride | undefined | null,
  onApply: (next: CompanionTimeOverride | null) => void | Promise<void>,
  canEdit: boolean,
) {
  const activeMode: OverrideMode = override?.mode ?? "off";
  const [nowMs, setNowMs] = useState(() => Date.now());
  const [editing, setEditing] = useState(false);
  const [selectedMode, setSelectedMode] = useState<OverrideMode>(activeMode);
  const [draftMs, setDraftMs] = useState(() => floorToMinute(Date.now()));

  useEffect(() => {
    const id = window.setInterval(() => setNowMs(Date.now()), 1000);
    return () => window.clearInterval(id);
  }, []);

  // If the persisted override changes from elsewhere, drop any open edit session.
  useEffect(() => {
    setSelectedMode(activeMode);
    setEditing(false);
  }, [activeMode]);

  const shownMs = effectiveOverrideMs(override, nowMs);

  const beginEdit = (mode: Exclude<OverrideMode, "off">) => {
    if (!canEdit) return;
    setSelectedMode(mode);
    setDraftMs(floorToMinute(effectiveOverrideMs(override, Date.now())));
    setEditing(true);
  };

  const selectLive = () => {
    if (!canEdit) return;
    setSelectedMode("off");
    setEditing(false);
    void onApply(null);
  };

  const apply = () => {
    if (!canEdit || selectedMode === "off") return;
    setEditing(false);
    void onApply({ mode: selectedMode, anchorMs: draftMs, setAtMs: Date.now() });
  };

  const cancel = () => {
    setEditing(false);
    setSelectedMode(activeMode);
  };

  return {
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
    isOverridden: activeMode !== "off",
  };
}
