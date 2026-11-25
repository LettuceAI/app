import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { SettingsDynamicMemorySettings } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

const backupKey = "lettuce.developer.chapter-settings-backup";

interface Backup {
  dynamic_memory: SettingsDynamicMemorySettings;
  group_dynamic_memory: SettingsDynamicMemorySettings | null;
}

const testValues = {
  enabled: true,
  run_mode: "auto",
  summary_message_interval: 6,
  chapter_summaries_enabled: true,
  summary_token_budget: 800,
  summary_chapter_size: 2,
} satisfies Partial<SettingsDynamicMemorySettings>;

function readBackup(): Backup | null {
  try {
    const raw = localStorage.getItem(backupKey);
    return raw ? (JSON.parse(raw) as Backup) : null;
  } catch {
    return null;
  }
}

export function hasChapterSettingsBackup(): boolean {
  return readBackup() !== null;
}

export async function applyChapterTestSettings(api: ApiClient, queryClient: QueryClient): Promise<void> {
  let settings = await api.call("settingsGet");
  if (!readBackup()) {
    const backup: Backup = { dynamic_memory: settings.global.dynamic_memory, group_dynamic_memory: settings.global.group_dynamic_memory };
    localStorage.setItem(backupKey, JSON.stringify(backup));
  }
  try {
    settings = await api.call("settingsUpdate", {
      patch: { section: "dynamic_memory", value: { ...settings.global.dynamic_memory, ...testValues } },
      expected_revision: settings.revision,
    });
    const group = settings.global.group_dynamic_memory;
    if (group) {
      await api.call("settingsUpdate", {
        patch: { section: "group_dynamic_memory", value: { ...group, ...testValues } },
        expected_revision: settings.revision,
      });
    }
  } finally {
    await queryClient.invalidateQueries({ queryKey: queryKeys.settings.all });
  }
}

export async function restoreChapterSettings(api: ApiClient, queryClient: QueryClient): Promise<void> {
  const backup = readBackup();
  if (!backup) return;
  try {
    let settings = await api.call("settingsGet");
    settings = await api.call("settingsUpdate", {
      patch: { section: "dynamic_memory", value: backup.dynamic_memory },
      expected_revision: settings.revision,
    });
    await api.call("settingsUpdate", {
      patch: { section: "group_dynamic_memory", value: backup.group_dynamic_memory },
      expected_revision: settings.revision,
    });
    localStorage.removeItem(backupKey);
  } finally {
    await queryClient.invalidateQueries({ queryKey: queryKeys.settings.all });
  }
}
