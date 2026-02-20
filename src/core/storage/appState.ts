import { invoke } from "@tauri-apps/api/core";
import { readSettings, setAppState } from "./repo";
import { createDefaultAppState, type AppState, type ChatsViewMode, type CustomColors, type PureModeLevel } from "./schemas";

type Theme = AppState["theme"];

function cloneAppState(state?: AppState): AppState {
  const source = state ?? createDefaultAppState();
  return {
    onboarding: { ...source.onboarding },
    theme: source.theme,
    tooltips: { ...source.tooltips },
    pureModeEnabled: source.pureModeEnabled,
    pureModeLevel: source.pureModeLevel ?? "standard",
    autoDownloadCharacterCardAvatars: source.autoDownloadCharacterCardAvatars ?? true,
    analyticsEnabled: source.analyticsEnabled ?? true,
    appActiveUsageMs: source.appActiveUsageMs ?? 0,
    appActiveUsageByDayMs: { ...(source.appActiveUsageByDayMs ?? {}) },
    appActiveUsageStartedAtMs: source.appActiveUsageStartedAtMs,
    appActiveUsageLastUpdatedAtMs: source.appActiveUsageLastUpdatedAtMs,
    customColors: source.customColors ? { ...source.customColors } : undefined,
    chatsViewMode: source.chatsViewMode ?? "hero",
  };
}

async function saveAppState(nextState: AppState): Promise<AppState> {
  const updated = cloneAppState(nextState);
  await setAppState(updated);
  return cloneAppState(updated);
}

async function withAppState(mutator: (draft: AppState) => void): Promise<AppState> {
  const settings = await readSettings();
  const draft = cloneAppState(settings.appState);
  mutator(draft);
  await setAppState(draft);
  return cloneAppState(draft);
}

export async function getAppState(): Promise<AppState> {
  const settings = await readSettings();
  return cloneAppState(settings.appState);
}

export async function resetAppState(): Promise<AppState> {
  return saveAppState(createDefaultAppState());
}

export async function isOnboardingCompleted(): Promise<boolean> {
  const state = await getAppState();
  return state.onboarding.completed;
}

export async function setOnboardingCompleted(completed: boolean = true): Promise<void> {
  await withAppState((state) => {
    state.onboarding.completed = completed;
    if (completed) {
      state.onboarding.skipped = false;
    }
  });
}

export async function isOnboardingSkipped(): Promise<boolean> {
  const state = await getAppState();
  return state.onboarding.skipped;
}

export async function setOnboardingSkipped(skipped: boolean = true): Promise<void> {
  await withAppState((state) => {
    state.onboarding.skipped = skipped;
    if (skipped) {
      state.onboarding.completed = true;
    }
  });
}

export async function isProviderSetupCompleted(): Promise<boolean> {
  const state = await getAppState();
  return state.onboarding.providerSetupCompleted;
}

export async function setProviderSetupCompleted(completed: boolean = true): Promise<void> {
  await withAppState((state) => {
    state.onboarding.providerSetupCompleted = completed;
    if (!completed) {
      state.onboarding.completed = false;
      state.onboarding.skipped = false;
    }
  });
}

export async function isModelSetupCompleted(): Promise<boolean> {
  const state = await getAppState();
  return state.onboarding.modelSetupCompleted;
}

export async function setModelSetupCompleted(completed: boolean = true): Promise<void> {
  await withAppState((state) => {
    state.onboarding.modelSetupCompleted = completed;
    if (!completed) {
      state.onboarding.completed = false;
      state.onboarding.skipped = false;
    }
  });
}

export async function getTheme(): Promise<Theme> {
  const state = await getAppState();
  return state.theme;
}

export async function setTheme(theme: Theme): Promise<void> {
  await withAppState((state) => {
    state.theme = theme;
  });
}

export async function setPureModeLevel(level: PureModeLevel): Promise<void> {
  await withAppState((state) => {
    state.pureModeLevel = level;
    state.pureModeEnabled = level !== "off";
  });
  await invoke("set_content_filter_level", { level }).catch(() => {
    // Filter state sync is best-effort; Rust side will re-read on next startup
  });
}

export async function setPureModeEnabled(enabled: boolean): Promise<void> {
  await setPureModeLevel(enabled ? "standard" : "off");
}

export async function setAnalyticsEnabled(enabled: boolean): Promise<void> {
  await withAppState((state) => {
    state.analyticsEnabled = enabled;
  });
}

export async function setAutoDownloadCharacterCardAvatars(enabled: boolean): Promise<void> {
  await withAppState((state) => {
    state.autoDownloadCharacterCardAvatars = enabled;
  });
}

export async function hasSeenTooltip(id: string): Promise<boolean> {
  const state = await getAppState();
  return Boolean(state.tooltips[id]);
}

export async function setTooltipSeen(id: string, seen: boolean = true): Promise<void> {
  await withAppState((state) => {
    if (seen) {
      state.tooltips[id] = true;
    } else {
      delete state.tooltips[id];
    }
  });
}

export async function clearTooltipState(): Promise<void> {
  await withAppState((state) => {
    state.tooltips = {};
  });
}

export async function getAppStateSummary(): Promise<{
  onboarding: AppState["onboarding"];
  theme: Theme;
  tooltipCount: number;
}> {
  const state = await getAppState();
  return {
    onboarding: { ...state.onboarding },
    theme: state.theme,
    tooltipCount: Object.keys(state.tooltips).length,
  };
}

export async function getCustomColors(): Promise<CustomColors | undefined> {
  const state = await getAppState();
  return state.customColors;
}

export async function setCustomColors(colors: CustomColors): Promise<void> {
  await withAppState((state) => {
    state.customColors = colors;
  });
}

export async function getChatsViewMode(): Promise<ChatsViewMode> {
  const state = await getAppState();
  return state.chatsViewMode ?? "hero";
}

export async function setChatsViewMode(mode: ChatsViewMode): Promise<void> {
  await withAppState((state) => {
    state.chatsViewMode = mode;
  });
}
