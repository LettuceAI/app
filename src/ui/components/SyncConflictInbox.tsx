import { useCallback, useEffect, useMemo, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AlertTriangle,
  Check,
  ChevronDown,
  GitCompareArrows,
  Loader2,
} from "lucide-react";

import { useI18n, type TranslationKey } from "../../core/i18n/context";
import { cn, interactive, typography } from "../design-tokens";

type ConflictField = {
  key: string;
  current: unknown;
  other: unknown;
};

type SyncConflict = {
  conflictId: string;
  entityType: string;
  entityName: string;
  operation: string;
  detectedAt: number;
  currentChangeId: string;
  currentDeviceId: string;
  currentTimestamp: number;
  otherChangeId: string;
  otherDeviceId: string;
  otherTimestamp: number;
  fields: ConflictField[];
};

const ENTITY_LABELS: Record<string, TranslationKey> = {
  characters: "characters.conflicts.entities.character",
  personas: "characters.conflicts.entities.persona",
  lorebooks: "characters.conflicts.entities.lorebook",
  lorebook_entries: "characters.conflicts.entities.lorebookEntry",
  prompt_templates: "characters.conflicts.entities.promptTemplate",
};

const FIELD_LABELS: Record<string, TranslationKey> = {
  name: "characters.conflicts.fields.name",
  nickname: "characters.conflicts.fields.nickname",
  title: "characters.conflicts.fields.title",
  description: "characters.conflicts.fields.description",
  definition: "characters.conflicts.fields.definition",
  scenario: "characters.conflicts.fields.scenario",
  creator_notes: "characters.conflicts.fields.creatorNotes",
  tags: "characters.conflicts.fields.tags",
  system_prompt: "characters.conflicts.fields.systemPrompt",
  keyword_detection_mode: "characters.conflicts.fields.keywordDetection",
  content: "characters.conflicts.fields.content",
  keywords: "characters.conflicts.fields.keywords",
  enabled: "characters.conflicts.fields.enabled",
  always_active: "characters.conflicts.fields.alwaysActive",
  priority: "characters.conflicts.fields.priority",
  entries: "characters.conflicts.fields.entries",
};

interface SyncConflictInboxProps {
  refreshKey: string;
  disabled?: boolean;
}

function formatValue(
  key: string,
  value: unknown,
  emptyLabel: string,
  enabledLabel: string,
  disabledLabel: string,
): string {
  if (value === null || value === undefined || value === "") return emptyLabel;
  if (
    (key === "enabled" || key === "always_active") &&
    (value === 0 || value === 1)
  ) {
    return value === 1 ? enabledLabel : disabledLabel;
  }
  if (typeof value === "boolean") return value ? "✓" : "—";
  if (typeof value === "number") return value.toLocaleString();
  if (typeof value !== "string") return JSON.stringify(value, null, 2) ?? emptyLabel;
  const trimmed = value.trim();
  if (
    (trimmed.startsWith("[") && trimmed.endsWith("]")) ||
    (trimmed.startsWith("{") && trimmed.endsWith("}"))
  ) {
    try {
      const parsed = JSON.parse(trimmed);
      if (Array.isArray(parsed) && parsed.every((item) => typeof item === "string")) {
        return parsed.length > 0 ? parsed.join(", ") : emptyLabel;
      }
      return JSON.stringify(parsed, null, 2);
    } catch {
      return value;
    }
  }
  return value;
}

function shortDeviceId(deviceId: string): string {
  return deviceId.length > 10 ? `${deviceId.slice(0, 8)}…` : deviceId;
}

export function SyncConflictInbox({
  refreshKey,
  disabled = false,
}: SyncConflictInboxProps) {
  const { t, locale } = useI18n();
  const [conflicts, setConflicts] = useState<SyncConflict[]>([]);
  const [loading, setLoading] = useState(true);
  const [resolving, setResolving] = useState<string | null>(null);
  const [error, setError] = useState<string | null>(null);

  const dateFormatter = useMemo(
    () =>
      new Intl.DateTimeFormat(locale, {
        dateStyle: "medium",
        timeStyle: "short",
      }),
    [locale],
  );

  const load = useCallback(async () => {
    try {
      const next = await invoke<SyncConflict[]>("list_sync_conflicts");
      setConflicts(next);
      setError(null);
    } catch (loadError) {
      console.error("Failed to load sync conflicts", loadError);
      setError(String(loadError));
    } finally {
      setLoading(false);
    }
  }, []);

  useEffect(() => {
    void load();
  }, [load, refreshKey]);

  useEffect(() => {
    let unlisten: (() => void) | null = null;
    void listen("sync-conflicts-changed", () => void load()).then((stop) => {
      unlisten = stop;
    });
    return () => {
      unlisten?.();
    };
  }, [load]);

  const resolve = async (conflictId: string, choice: "current" | "other") => {
    setResolving(conflictId);
    setError(null);
    try {
      await invoke("resolve_sync_conflict", { conflictId, choice });
      setConflicts((current) =>
        current.filter((conflict) => conflict.conflictId !== conflictId),
      );
    } catch (resolveError) {
      console.error("Failed to resolve sync conflict", resolveError);
      setError(String(resolveError));
    } finally {
      setResolving(null);
    }
  };

  if (loading || conflicts.length === 0) {
    return error ? (
      <div className="rounded-xl border border-danger/20 bg-danger/8 px-4 py-3 text-xs text-danger/80">
        {t("characters.conflicts.loadError")}
      </div>
    ) : null;
  }

  return (
    <div>
      <div className="mb-2.5 flex items-center justify-between gap-3 px-1">
        <div className="flex items-center gap-2">
          <GitCompareArrows className="h-3 w-3 text-warning/75" />
          <h2
            className={cn(
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "text-fg/45",
            )}
          >
            {t("characters.conflicts.title")}
          </h2>
        </div>
        <span className="rounded-full border border-warning/20 bg-warning/10 px-2 py-0.5 font-mono text-[10px] font-semibold tabular-nums text-warning/85">
          {conflicts.length}
        </span>
      </div>

      <div className="overflow-hidden rounded-xl border border-warning/20 bg-warning/[0.035]">
        <div className="flex items-start gap-3 border-b border-warning/10 px-4 py-3">
          <AlertTriangle className="mt-0.5 h-4 w-4 shrink-0 text-warning/80" />
          <div>
            <p className="text-[12px] font-medium text-fg/85">
              {t("characters.conflicts.reviewTitle")}
            </p>
            <p className="mt-0.5 text-[11px] leading-relaxed text-fg/45">
              {disabled
                ? t("characters.conflicts.waitForSync")
                : t("characters.conflicts.reviewDescription")}
            </p>
          </div>
        </div>

        <div className="divide-y divide-fg/8">
          {conflicts.map((conflict) => {
            const entityLabel = t(
              ENTITY_LABELS[conflict.entityType] ?? "characters.conflicts.entities.item",
            );
            const isResolving = resolving === conflict.conflictId;

            return (
              <details key={conflict.conflictId} className="group">
                <summary className="flex cursor-pointer list-none items-center gap-3 px-4 py-3.5">
                  <div className="flex h-8 w-8 shrink-0 items-center justify-center rounded-lg border border-warning/20 bg-warning/8 text-warning/80">
                    <GitCompareArrows className="h-4 w-4" />
                  </div>
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[12px] font-semibold text-fg/85">
                      {conflict.entityName || t("common.labels.untitled")}
                    </p>
                    <p className="mt-0.5 truncate text-[10px] text-fg/40">
                      {entityLabel} ·{" "}
                      {t("characters.conflicts.fieldsDiffer", {
                        count: conflict.fields.length,
                      })}
                    </p>
                  </div>
                  <ChevronDown className="h-4 w-4 shrink-0 text-fg/30 transition-transform group-open:rotate-180" />
                </summary>

                <div className="border-t border-fg/8 px-4 py-4">
                  <div className="mb-3 grid grid-cols-2 gap-2">
                    <div className="rounded-lg border border-accent/15 bg-accent/[0.04] px-3 py-2">
                      <p className="text-[9px] font-semibold uppercase tracking-wider text-accent/70">
                        {t("characters.conflicts.currentVersion")}
                      </p>
                      <p className="mt-1 font-mono text-[9px] text-fg/35">
                        {t("characters.conflicts.device", {
                          id: shortDeviceId(conflict.currentDeviceId),
                        })}
                      </p>
                    </div>
                    <div className="rounded-lg border border-info/15 bg-info/[0.04] px-3 py-2">
                      <p className="text-[9px] font-semibold uppercase tracking-wider text-info/70">
                        {t("characters.conflicts.otherVersion")}
                      </p>
                      <p className="mt-1 font-mono text-[9px] text-fg/35">
                        {t("characters.conflicts.device", {
                          id: shortDeviceId(conflict.otherDeviceId),
                        })}
                      </p>
                    </div>
                  </div>

                  <div className="space-y-2">
                    {conflict.fields.map((field) => (
                      <div
                        key={field.key}
                        className="overflow-hidden rounded-lg border border-fg/8"
                      >
                        <p className="border-b border-fg/8 bg-fg/[0.025] px-3 py-1.5 text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                          {t(
                            FIELD_LABELS[field.key] ??
                              "characters.conflicts.fields.changedValue",
                          )}
                        </p>
                        <div className="grid grid-cols-2 divide-x divide-fg/8">
                          <pre className="max-h-36 overflow-auto whitespace-pre-wrap break-words px-3 py-2.5 font-sans text-[10px] leading-relaxed text-fg/65">
                            {formatValue(
                              field.key,
                              field.current,
                              t("characters.conflicts.emptyValue"),
                              t("common.labels.enabled"),
                              t("common.labels.disabled"),
                            )}
                          </pre>
                          <pre className="max-h-36 overflow-auto whitespace-pre-wrap break-words px-3 py-2.5 font-sans text-[10px] leading-relaxed text-fg/65">
                            {formatValue(
                              field.key,
                              field.other,
                              t("characters.conflicts.emptyValue"),
                              t("common.labels.enabled"),
                              t("common.labels.disabled"),
                            )}
                          </pre>
                        </div>
                      </div>
                    ))}
                  </div>

                  <p className="mt-3 text-[9px] text-fg/30">
                    {t("characters.conflicts.detected", {
                      date: dateFormatter.format(new Date(conflict.detectedAt)),
                    })}
                  </p>

                  {error && (
                    <p className="mt-3 rounded-lg border border-danger/15 bg-danger/8 px-3 py-2 text-[10px] text-danger/80">
                      {t("characters.conflicts.resolveError")}
                    </p>
                  )}

                  <div className="mt-3 grid grid-cols-2 gap-2">
                    <button
                      type="button"
                      disabled={disabled || resolving !== null}
                      onClick={() => void resolve(conflict.conflictId, "current")}
                      className={cn(
                        "flex items-center justify-center gap-2 rounded-lg border border-accent/30 bg-accent/10 px-3 py-2.5 text-[11px] font-semibold text-accent/90",
                        interactive.transition.fast,
                        interactive.active.scale,
                        "hover:border-accent/50 hover:bg-accent/15 disabled:cursor-not-allowed disabled:opacity-40",
                      )}
                    >
                      {isResolving ? (
                        <Loader2 className="h-3.5 w-3.5 animate-spin" />
                      ) : (
                        <Check className="h-3.5 w-3.5" />
                      )}
                      {t("characters.conflicts.keepCurrent")}
                    </button>
                    <button
                      type="button"
                      disabled={disabled || resolving !== null}
                      onClick={() => void resolve(conflict.conflictId, "other")}
                      className={cn(
                        "rounded-lg border border-fg/12 bg-fg/5 px-3 py-2.5 text-[11px] font-medium text-fg/70",
                        interactive.transition.fast,
                        interactive.active.scale,
                        "hover:border-info/30 hover:bg-info/8 hover:text-info disabled:cursor-not-allowed disabled:opacity-40",
                      )}
                    >
                      {t("characters.conflicts.useOther")}
                    </button>
                  </div>
                </div>
              </details>
            );
          })}
        </div>
      </div>
    </div>
  );
}
