import { useCallback, useEffect, useMemo, useState } from "react";
import { CalendarClock, Eye, EyeOff, Pencil, Plus, Trash2 } from "lucide-react";

import { BottomMenu, MenuSection } from "../../../components/BottomMenu";
import { Switch } from "../../../components/Switch";
import { toast } from "../../../components/toast";
import { cn, radius, spacing, typography, interactive } from "../../../design-tokens";
import { useI18n, type TranslationKey } from "../../../../core/i18n/context";
import {
  deleteCompanionScheduledNote,
  listCompanionScheduledNotes,
  previewActiveCompanionScheduledNotes,
  saveCompanionScheduledNote,
} from "../../../../core/storage/repo";
import type {
  CompanionScheduledNote,
  CompanionScheduledNoteRecurrence,
} from "../../../../core/storage/schemas";

type DraftState = {
  id: string;
  label: string;
  content: string;
  availableAt: string;
  expiresAt: string;
  recurrence: CompanionScheduledNoteRecurrence;
  recurrenceWindowHours: string;
  enabled: boolean;
  createdAt: number;
};

const RECURRENCE_OPTIONS = [
  { value: "none", labelKey: "characters.scheduledNotes.recurrenceOnce" },
  { value: "daily", labelKey: "characters.scheduledNotes.recurrenceDaily" },
  { value: "weekly", labelKey: "characters.scheduledNotes.recurrenceWeekly" },
  { value: "monthly", labelKey: "characters.scheduledNotes.recurrenceMonthly" },
  { value: "yearly", labelKey: "characters.scheduledNotes.recurrenceYearly" },
] satisfies Array<{ value: CompanionScheduledNoteRecurrence; labelKey: TranslationKey }>;

const RECURRENCE_UNIT_KEY = {
  daily: "characters.scheduledNotes.unitDay",
  weekly: "characters.scheduledNotes.unitWeek",
  monthly: "characters.scheduledNotes.unitMonth",
  yearly: "characters.scheduledNotes.unitYear",
} satisfies Record<Exclude<CompanionScheduledNoteRecurrence, "none">, TranslationKey>;

function pad(value: number): string {
  return String(value).padStart(2, "0");
}

function formatDateTimeLocal(valueMs: number): string {
  const date = new Date(valueMs);
  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}T${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function dateTimeLocalToMs(value: string): number {
  const parsed = new Date(value);
  return Number.isNaN(parsed.getTime()) ? Date.now() : parsed.getTime();
}

function formatPreviewDate(valueMs: number, withTime = true): string {
  return new Intl.DateTimeFormat(undefined, {
    dateStyle: "medium",
    timeStyle: withTime ? "short" : undefined,
  }).format(new Date(valueMs));
}

function formatTime(valueMs: number): string {
  return new Intl.DateTimeFormat(undefined, { timeStyle: "short" }).format(new Date(valueMs));
}

// Split a datetime-local string ("YYYY-MM-DDTHH:MM") into date and time parts.
function splitDateTime(value: string): { date: string; time: string } {
  if (!value) return { date: "", time: "" };
  const [date = "", time = ""] = value.split("T");
  return { date, time: time.slice(0, 5) };
}

function combineDateTime(date: string, time: string): string {
  if (!date && !time) return "";
  const safeDate = date || formatDateTimeLocal(Date.now()).split("T")[0]!;
  const safeTime = time || "09:00";
  return `${safeDate}T${safeTime}`;
}

type TFn = (key: TranslationKey, params?: Record<string, string | number>) => string;

function describeSchedule(draft: DraftState, t: TFn): string {
  const startsMs = dateTimeLocalToMs(draft.availableAt);
  const endsMs = draft.expiresAt ? dateTimeLocalToMs(draft.expiresAt) : null;
  const hours = Number(draft.recurrenceWindowHours);
  const validHours = Number.isFinite(hours) && hours > 0;

  if (draft.recurrence === "none") {
    const start = formatPreviewDate(startsMs);
    if (endsMs && endsMs > startsMs) {
      return t("characters.scheduledNotes.describeOnceUntil", {
        start,
        end: formatPreviewDate(endsMs),
      });
    }
    return t("characters.scheduledNotes.describeOnceOnward", { start });
  }

  const timeOfDay = formatTime(startsMs);
  const window = validHours
    ? t("characters.scheduledNotes.windowHours", { hours })
    : t("characters.scheduledNotes.windowUntilNext", { unit: t(RECURRENCE_UNIT_KEY[draft.recurrence]) });
  const startDate = formatPreviewDate(startsMs, false);
  const cap =
    endsMs && endsMs > startsMs
      ? t("characters.scheduledNotes.describeStopsAfter", { date: formatPreviewDate(endsMs, false) })
      : "";

  if (draft.recurrence === "daily") {
    return t("characters.scheduledNotes.describeDaily", { time: timeOfDay, window, start: startDate, cap });
  }
  if (draft.recurrence === "weekly") {
    const weekday = new Intl.DateTimeFormat(undefined, { weekday: "long" }).format(
      new Date(startsMs),
    );
    return t("characters.scheduledNotes.describeWeekly", {
      weekday,
      time: timeOfDay,
      window,
      start: startDate,
      cap,
    });
  }
  // monthly / yearly
  const recurrenceLabel = t(
    RECURRENCE_OPTIONS.find((o) => o.value === draft.recurrence)?.labelKey ??
      "characters.scheduledNotes.recurrenceMonthly",
  ).toLowerCase();
  return t("characters.scheduledNotes.describeMonthlyYearly", {
    recurrence: recurrenceLabel,
    date: formatPreviewDate(startsMs),
    window,
    cap,
  });
}

function notePreview(content: string, t: TFn): string {
  const trimmed = content.trim();
  if (!trimmed) return t("characters.scheduledNotes.emptyContent");
  return trimmed.length > 140 ? `${trimmed.slice(0, 140)}...` : trimmed;
}

function defaultDraft(): DraftState {
  const now = Date.now();
  return {
    id: globalThis.crypto?.randomUUID?.() ?? `${now}`,
    label: "",
    content: "",
    availableAt: formatDateTimeLocal(now),
    expiresAt: "",
    recurrence: "none",
    recurrenceWindowHours: "24",
    enabled: true,
    createdAt: now,
  };
}

function noteToDraft(note: CompanionScheduledNote): DraftState {
  return {
    id: note.id,
    label: note.label,
    content: note.content,
    availableAt: formatDateTimeLocal(note.availableAt),
    expiresAt: note.expiresAt ? formatDateTimeLocal(note.expiresAt) : "",
    recurrence: note.recurrence,
    recurrenceWindowHours: note.recurrenceWindowMs
      ? String(Math.round(note.recurrenceWindowMs / (60 * 60 * 1000)))
      : "",
    enabled: note.enabled,
    createdAt: note.createdAt,
  };
}

function draftToNote(characterId: string, draft: DraftState): CompanionScheduledNote {
  const availableAt = dateTimeLocalToMs(draft.availableAt);
  const expiresAt = draft.expiresAt ? dateTimeLocalToMs(draft.expiresAt) : null;
  const windowHours = Number(draft.recurrenceWindowHours);
  return {
    id: draft.id,
    characterId,
    label: draft.label.trim(),
    content: draft.content.trim(),
    availableAt,
    expiresAt,
    recurrence: draft.recurrence,
    recurrenceWindowMs:
      draft.recurrence !== "none" && Number.isFinite(windowHours) && windowHours > 0
        ? Math.round(windowHours * 60 * 60 * 1000)
        : null,
    enabled: draft.enabled,
    createdAt: draft.createdAt || Date.now(),
    updatedAt: Date.now(),
  };
}

type Status = "active" | "scheduled" | "expired" | "inactive";

function statusFor(note: CompanionScheduledNote, isActive: boolean, previewMs: number): Status {
  if (note.expiresAt != null && previewMs >= note.expiresAt) return "expired";
  if (previewMs < note.availableAt) return "scheduled";
  return isActive ? "active" : "inactive";
}

const STATUS_LABEL_KEY = {
  active: "characters.scheduledNotes.statusActive",
  scheduled: "characters.scheduledNotes.statusScheduled",
  expired: "characters.scheduledNotes.statusExpired",
  inactive: "characters.scheduledNotes.statusInactive",
} satisfies Record<Status, TranslationKey>;

const STATUS_PILL: Record<Status, string> = {
  active: "border-accent/40 bg-accent/15 text-accent",
  scheduled: "border-info/40 bg-info/15 text-info",
  expired: "border-fg/15 bg-fg/8 text-fg/55",
  inactive: "border-fg/15 bg-fg/8 text-fg/55",
};

const inputClass = cn(
  "min-w-0 w-full border bg-surface-el/40 px-3 py-2 text-fg outline-none placeholder:text-fg/35",
  typography.bodySmall.size,
  "border-fg/10",
  radius.md,
  interactive.transition.fast,
  "focus:border-fg/25 focus:bg-surface-el/60",
);

const labelClass = cn(
  typography.label.size,
  typography.label.weight,
  typography.label.tracking,
  "uppercase text-fg/55",
);

const accentButton = cn(
  "inline-flex items-center justify-center gap-1.5 border border-accent/30 bg-accent/15 px-3 py-2 font-semibold text-accent",
  typography.bodySmall.size,
  radius.md,
  interactive.transition.fast,
  interactive.active.scale,
  "hover:border-accent/45 hover:bg-accent/25 disabled:cursor-not-allowed disabled:opacity-50",
);

const ghostButton = cn(
  "inline-flex items-center justify-center gap-1.5 border border-fg/10 bg-fg/5 px-3 py-2 font-medium text-fg/70",
  typography.bodySmall.size,
  radius.md,
  interactive.transition.fast,
  "hover:border-fg/20 hover:text-fg disabled:opacity-50",
);

interface Props {
  characterId: string;
}

export function CompanionScheduledNotesEditor({ characterId }: Props) {
  const { t } = useI18n();
  const [notes, setNotes] = useState<CompanionScheduledNote[]>([]);
  const [activeIds, setActiveIds] = useState<Set<string>>(new Set());
  const [loading, setLoading] = useState(true);
  const [saving, setSaving] = useState(false);
  const [previewAsOf, setPreviewAsOf] = useState(() => formatDateTimeLocal(Date.now()));
  const [showPreviewControl, setShowPreviewControl] = useState(false);
  const [draft, setDraft] = useState<DraftState | null>(null);

  const previewMs = useMemo(() => dateTimeLocalToMs(previewAsOf), [previewAsOf]);
  const isPreviewingNow = useMemo(() => {
    return Math.abs(previewMs - Date.now()) < 60_000;
  }, [previewMs]);

  const refreshActive = useCallback(
    async (asOfMs: number) => {
      try {
        const active = await previewActiveCompanionScheduledNotes(characterId, asOfMs);
        setActiveIds(new Set(active.map((note) => note.id)));
      } catch (error) {
        console.error("Failed to preview active scheduled notes:", error);
      }
    },
    [characterId],
  );

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      const [listed, active] = await Promise.all([
        listCompanionScheduledNotes(characterId),
        previewActiveCompanionScheduledNotes(characterId, dateTimeLocalToMs(previewAsOf)),
      ]);
      setNotes(listed);
      setActiveIds(new Set(active.map((note) => note.id)));
    } catch (error) {
      console.error("Failed to load companion scheduled notes:", error);
      toast.error(t("characters.scheduledNotes.loadError"));
    } finally {
      setLoading(false);
    }
    // refresh on first mount only; previewAsOf change handled separately
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [characterId]);

  useEffect(() => {
    void reload();
  }, [reload]);

  useEffect(() => {
    if (notes.length === 0) return;
    void refreshActive(previewMs);
  }, [notes.length, previewMs, refreshActive]);

  const handleSave = useCallback(async () => {
    if (!draft) return;
    try {
      setSaving(true);
      const saved = await saveCompanionScheduledNote(draftToNote(characterId, draft));
      setNotes((current) => {
        const next = current.filter((n) => n.id !== saved.id);
        next.push(saved);
        next.sort((a, b) => a.availableAt - b.availableAt || a.id.localeCompare(b.id));
        return next;
      });
      await refreshActive(previewMs);
      setDraft(null);
      toast.success(t("characters.scheduledNotes.saveSuccess"));
    } catch (error) {
      console.error("Failed to save companion scheduled note:", error);
      toast.error(
        error instanceof Error ? error.message : t("characters.scheduledNotes.saveError"),
      );
    } finally {
      setSaving(false);
    }
  }, [characterId, draft, previewMs, refreshActive]);

  const toggleEnabled = useCallback(
    async (note: CompanionScheduledNote, enabled: boolean) => {
      try {
        const saved = await saveCompanionScheduledNote({ ...note, enabled });
        setNotes((current) => current.map((n) => (n.id === saved.id ? saved : n)));
        await refreshActive(previewMs);
      } catch (error) {
        console.error("Failed to toggle scheduled note:", error);
        toast.error(t("characters.scheduledNotes.updateError"));
      }
    },
    [previewMs, refreshActive],
  );

  const handleDelete = useCallback(async (id: string) => {
    try {
      await deleteCompanionScheduledNote(id);
      setNotes((current) => current.filter((n) => n.id !== id));
      setActiveIds((current) => {
        const next = new Set(current);
        next.delete(id);
        return next;
      });
      toast.success(t("characters.scheduledNotes.deleteSuccess"));
    } catch (error) {
      console.error("Failed to delete companion scheduled note:", error);
      toast.error(t("characters.scheduledNotes.deleteError"));
    }
  }, []);

  const draftIsNew = draft ? !notes.some((n) => n.id === draft.id) : false;

  const formSheet = (
    <BottomMenu
      isOpen={draft != null}
      onClose={() => {
        if (!saving) setDraft(null);
      }}
      title={
        draftIsNew
          ? t("characters.scheduledNotes.newNoteTitle")
          : t("characters.scheduledNotes.editNoteTitle")
      }
    >
      <MenuSection>
        {draft ? (
          <div className={spacing.group}>
            <div className={spacing.field}>
              <label className={labelClass}>
                {t("characters.scheduledNotes.labelOptional")}
              </label>
              <input
                type="text"
                value={draft.label}
                onChange={(e) => setDraft((d) => (d ? { ...d, label: e.target.value } : d))}
                placeholder={t("characters.scheduledNotes.labelPlaceholder")}
                className={inputClass}
              />
            </div>

            <div className={spacing.field}>
              <label className={labelClass}>{t("characters.scheduledNotes.contentLabel")}</label>
              <textarea
                value={draft.content}
                onChange={(e) => setDraft((d) => (d ? { ...d, content: e.target.value } : d))}
                rows={4}
                placeholder={t("characters.scheduledNotes.contentPlaceholder")}
                className={cn(inputClass, "resize-none leading-relaxed")}
              />
              <p className={cn(typography.caption.size, "text-fg/45")}>
                {t("characters.scheduledNotes.contentHelp")}
              </p>
            </div>

            <div className={spacing.field}>
              <label className={labelClass}>{t("characters.scheduledNotes.repeatsLabel")}</label>
              <select
                value={draft.recurrence}
                onChange={(e) =>
                  setDraft((d) =>
                    d
                      ? { ...d, recurrence: e.target.value as CompanionScheduledNoteRecurrence }
                      : d,
                  )
                }
                className={inputClass}
              >
                {RECURRENCE_OPTIONS.map((option) => (
                  <option key={option.value} value={option.value}>
                    {t(option.labelKey)}
                  </option>
                ))}
              </select>
            </div>

            {(() => {
              const start = splitDateTime(draft.availableAt);
              const end = splitDateTime(draft.expiresAt);
              const setStart = (date: string, time: string) =>
                setDraft((d) =>
                  d ? { ...d, availableAt: combineDateTime(date, time) } : d,
                );
              const setEndDate = (date: string) =>
                setDraft((d) =>
                  d
                    ? {
                        ...d,
                        expiresAt: date ? combineDateTime(date, end.time || "23:59") : "",
                      }
                    : d,
                );
              const setEnd = (date: string, time: string) =>
                setDraft((d) =>
                  d
                    ? {
                        ...d,
                        expiresAt: date || time ? combineDateTime(date, time) : "",
                      }
                    : d,
                );

              if (draft.recurrence === "none") {
                return (
                  <div className="grid items-end gap-3 sm:grid-cols-2">
                    <div className={spacing.field}>
                      <label className={labelClass}>{t("characters.scheduledNotes.showOn")}</label>
                      <div className="grid grid-cols-[minmax(0,1fr)_7rem] gap-2">
                        <input
                          type="date"
                          value={start.date}
                          onChange={(e) => setStart(e.target.value, start.time)}
                          className={inputClass}
                        />
                        <input
                          type="time"
                          value={start.time}
                          onChange={(e) => setStart(start.date, e.target.value)}
                          className={inputClass}
                        />
                      </div>
                    </div>
                    <div className={spacing.field}>
                      <label className={labelClass}>{t("characters.scheduledNotes.hideOn")}</label>
                      <div className="grid grid-cols-[minmax(0,1fr)_7rem] gap-2">
                        <input
                          type="date"
                          value={end.date}
                          onChange={(e) => setEnd(e.target.value, end.time)}
                          className={inputClass}
                        />
                        <input
                          type="time"
                          value={end.time}
                          onChange={(e) => setEnd(end.date, e.target.value)}
                          disabled={!end.date}
                          className={cn(
                            inputClass,
                            "disabled:cursor-not-allowed disabled:opacity-40",
                          )}
                        />
                      </div>
                    </div>
                  </div>
                );
              }

              if (draft.recurrence === "daily" || draft.recurrence === "weekly") {
                return (
                  <div className="grid items-end gap-3 sm:grid-cols-4">
                    <div className={spacing.field}>
                      <label className={labelClass}>{t("characters.scheduledNotes.timeOfDay")}</label>
                      <input
                        type="time"
                        value={start.time}
                        onChange={(e) => setStart(start.date, e.target.value)}
                        className={inputClass}
                      />
                    </div>
                    <div className={spacing.field}>
                      <label className={labelClass}>{t("characters.scheduledNotes.startsOn")}</label>
                      <input
                        type="date"
                        value={start.date}
                        onChange={(e) => setStart(e.target.value, start.time)}
                        className={inputClass}
                      />
                    </div>
                    <div className={spacing.field}>
                      <label className={labelClass}>{t("characters.scheduledNotes.stopsOn")}</label>
                      <input
                        type="date"
                        value={end.date}
                        onChange={(e) => setEndDate(e.target.value)}
                        className={inputClass}
                      />
                    </div>
                    <div className={spacing.field}>
                      <label className={labelClass}>{t("characters.scheduledNotes.visibleForHours")}</label>
                      <input
                        type="number"
                        min="1"
                        step="1"
                        value={draft.recurrenceWindowHours}
                        onChange={(e) =>
                          setDraft((d) =>
                            d ? { ...d, recurrenceWindowHours: e.target.value } : d,
                          )
                        }
                        placeholder="24"
                        className={inputClass}
                      />
                    </div>
                  </div>
                );
              }

              // monthly / yearly
              return (
                <div className="grid items-end gap-3 sm:grid-cols-3">
                  <div className={spacing.field}>
                    <label className={labelClass}>{t("characters.scheduledNotes.firstOccurrence")}</label>
                    <div className="grid grid-cols-[minmax(0,1fr)_7rem] gap-2">
                      <input
                        type="date"
                        value={start.date}
                        onChange={(e) => setStart(e.target.value, start.time)}
                        className={inputClass}
                      />
                      <input
                        type="time"
                        value={start.time}
                        onChange={(e) => setStart(start.date, e.target.value)}
                        className={inputClass}
                      />
                    </div>
                  </div>
                  <div className={spacing.field}>
                    <label className={labelClass}>{t("characters.scheduledNotes.stopsAfter")}</label>
                    <input
                      type="date"
                      value={end.date}
                      onChange={(e) => setEndDate(e.target.value)}
                      className={inputClass}
                    />
                  </div>
                  <div className={spacing.field}>
                    <label className={labelClass}>{t("characters.scheduledNotes.visibleForHours")}</label>
                    <input
                      type="number"
                      min="1"
                      step="1"
                      value={draft.recurrenceWindowHours}
                      onChange={(e) =>
                        setDraft((d) =>
                          d ? { ...d, recurrenceWindowHours: e.target.value } : d,
                        )
                      }
                      placeholder="24"
                      className={inputClass}
                    />
                  </div>
                </div>
              );
            })()}

            <div
              className={cn(
                "border border-fg/10 bg-fg/5 px-3 py-2",
                radius.md,
              )}
            >
              <p className={cn(typography.caption.size, "text-fg/65 leading-relaxed")}>
                {describeSchedule(draft, t)}
              </p>
            </div>

            <label
              className={cn(
                "flex items-center justify-between border border-fg/10 bg-fg/5 px-3 py-3",
                radius.md,
              )}
            >
              <span className="min-w-0">
                <span className={cn(typography.bodySmall.size, "block font-semibold text-fg")}>
                  {t("characters.scheduledNotes.enabled")}
                </span>
                <span className={cn(typography.caption.size, "mt-1 block text-fg/50")}>
                  {t("characters.scheduledNotes.enabledHelp")}
                </span>
              </span>
              <Switch
                checked={draft.enabled}
                onChange={(checked) => setDraft((d) => (d ? { ...d, enabled: checked } : d))}
              />
            </label>

            <div className="flex items-center justify-end gap-2 pt-1">
              <button
                type="button"
                onClick={() => setDraft(null)}
                disabled={saving}
                className={ghostButton}
              >
                {t("common.buttons.cancel")}
              </button>
              <button
                type="button"
                onClick={() => void handleSave()}
                disabled={saving || !draft.content.trim()}
                className={accentButton}
              >
                {saving
                  ? t("common.buttons.saving")
                  : draftIsNew
                    ? t("characters.scheduledNotes.addNote")
                    : t("common.buttons.save")}
              </button>
            </div>
          </div>
        ) : null}
      </MenuSection>
    </BottomMenu>
  );

  // ----- Empty state -----
  if (!loading && notes.length === 0) {
    return (
      <>
        <div
          className={cn(
            "flex flex-col items-center gap-3 border border-dashed border-fg/15 px-6 py-10 text-center",
            radius.md,
          )}
        >
          <div
            className={cn(
              "flex h-10 w-10 items-center justify-center border border-fg/10 bg-fg/5 text-fg/55",
              radius.full,
            )}
          >
            <CalendarClock className="h-5 w-5" />
          </div>
          <div className="max-w-sm">
            <p className={cn(typography.body.size, "font-semibold text-fg")}>
              {t("characters.scheduledNotes.emptyTitle")}
            </p>
            <p className={cn(typography.bodySmall.size, "mt-1 text-fg/55")}>
              {t("characters.scheduledNotes.emptyDescription")}
            </p>
          </div>
          <button
            type="button"
            onClick={() => setDraft(defaultDraft())}
            className={cn(accentButton, "mt-1")}
          >
            <Plus className="h-3.5 w-3.5" />
            {t("characters.scheduledNotes.addANote")}
          </button>
        </div>
        {formSheet}
      </>
    );
  }

  // ----- List view -----
  return (
    <>
    <div className={cn(spacing.group, "pb-20")}>
      <div className="flex items-center justify-between gap-2">
        <button
          type="button"
          onClick={() => setShowPreviewControl((v) => !v)}
          className={cn(
            "inline-flex items-center gap-1.5 border border-fg/10 bg-fg/5 px-2.5 py-1.5 font-medium text-fg/70",
            typography.caption.size,
            radius.md,
            interactive.transition.fast,
            "hover:border-fg/20 hover:text-fg",
            showPreviewControl && "border-fg/25 text-fg",
          )}
          title={
            isPreviewingNow
              ? t("characters.scheduledNotes.previewingNowTooltip")
              : t("characters.scheduledNotes.previewingFutureTooltip")
          }
        >
          {showPreviewControl ? <EyeOff className="h-3 w-3" /> : <Eye className="h-3 w-3" />}
          {isPreviewingNow
            ? t("characters.scheduledNotes.activeNow")
            : t("characters.scheduledNotes.asOf", { date: formatPreviewDate(previewMs, false) })}
        </button>
        <button
          type="button"
          onClick={() => setDraft(defaultDraft())}
          className={accentButton}
        >
          <Plus className="h-3.5 w-3.5" />
          {t("characters.scheduledNotes.addNote")}
        </button>
      </div>

      {showPreviewControl ? (
        <div className={cn("border border-fg/10 bg-fg/5 p-3", radius.md)}>
          {(() => {
            const preview = splitDateTime(previewAsOf);
            const setPreview = (date: string, time: string) =>
              setPreviewAsOf(combineDateTime(date, time));
            return (
              <div className="grid grid-cols-[minmax(0,1fr)_7rem_auto] gap-2">
                <input
                  type="date"
                  value={preview.date}
                  onChange={(e) => setPreview(e.target.value, preview.time)}
                  className={inputClass}
                />
                <input
                  type="time"
                  value={preview.time}
                  onChange={(e) => setPreview(preview.date, e.target.value)}
                  className={inputClass}
                />
                <button
                  type="button"
                  onClick={() => setPreviewAsOf(formatDateTimeLocal(Date.now()))}
                  className={ghostButton}
                >
                  {t("characters.scheduledNotes.now")}
                </button>
              </div>
            );
          })()}
          <p className={cn(typography.caption.size, "mt-2 text-fg/50")}>
            {t("characters.scheduledNotes.previewHelp")}
          </p>
        </div>
      ) : null}

      <div className={spacing.item}>
        {loading ? (
          <div
            className={cn(
              "border border-dashed border-fg/10 px-4 py-6 text-center text-fg/55",
              typography.bodySmall.size,
              radius.md,
            )}
          >
            {t("characters.scheduledNotes.loading")}
          </div>
        ) : (
          notes.map((note) => {
            const status = statusFor(note, activeIds.has(note.id), previewMs);
            return (
              <div
                key={note.id}
                className={cn(
                  "border border-fg/10 bg-surface-el/40 p-4 transition-colors duration-200 hover:border-fg/20",
                  radius.md,
                )}
              >
                <div className="flex items-start justify-between gap-3">
                  <div className="min-w-0 flex-1">
                    <div className="flex min-h-[1.5rem] items-center gap-2 overflow-hidden">
                      <p
                        className={cn(
                          typography.body.size,
                          "min-w-0 truncate font-semibold text-fg",
                        )}
                      >
                        {note.label.trim() || t("characters.scheduledNotes.untitledNote")}
                      </p>
                      <span
                        className={cn(
                          "shrink-0 border px-2 py-0.5 uppercase tracking-[0.14em]",
                          typography.overline.size,
                          radius.full,
                          STATUS_PILL[status],
                          !note.enabled && "opacity-50",
                        )}
                      >
                        {!note.enabled
                          ? t("characters.scheduledNotes.statusOff")
                          : t(STATUS_LABEL_KEY[status])}
                      </span>
                    </div>
                    <p className={cn(typography.bodySmall.size, "mt-2 text-fg/75")}>
                      {notePreview(note.content, t)}
                    </p>
                    <div
                      className={cn(
                        "mt-3 flex flex-wrap items-center gap-x-3 gap-y-1.5 text-fg/50",
                        typography.caption.size,
                      )}
                    >
                      <span className="inline-flex items-center gap-1">
                        <CalendarClock className="h-3 w-3" />
                        {t("characters.scheduledNotes.cardStarts", {
                          date: formatPreviewDate(note.availableAt),
                        })}
                      </span>
                      <span>
                        {(() => {
                          const labelKey = RECURRENCE_OPTIONS.find(
                            (o) => o.value === note.recurrence,
                          )?.labelKey;
                          return labelKey ? t(labelKey) : null;
                        })()}
                      </span>
                      {note.recurrence !== "none" && note.recurrenceWindowMs ? (
                        <span>
                          {t("characters.scheduledNotes.cardWindow", {
                            hours: Math.round(note.recurrenceWindowMs / (60 * 60 * 1000)),
                          })}
                        </span>
                      ) : null}
                      {note.expiresAt ? (
                        <span>
                          {t("characters.scheduledNotes.cardEnds", {
                            date: formatPreviewDate(note.expiresAt),
                          })}
                        </span>
                      ) : null}
                    </div>
                  </div>
                  <div className="flex shrink-0 items-center gap-2">
                    <Switch
                      checked={note.enabled}
                      onChange={(checked) => void toggleEnabled(note, checked)}
                    />
                    <button
                      type="button"
                      onClick={() => setDraft(noteToDraft(note))}
                      className={cn(
                        "border border-fg/10 p-2 text-fg/65",
                        radius.md,
                        interactive.transition.fast,
                        "hover:border-fg/25 hover:bg-fg/5 hover:text-fg",
                      )}
                      aria-label={t("characters.scheduledNotes.editNoteAria")}
                    >
                      <Pencil className="h-3.5 w-3.5" />
                    </button>
                    <button
                      type="button"
                      onClick={() => void handleDelete(note.id)}
                      className={cn(
                        "border border-danger/25 p-2 text-danger",
                        radius.md,
                        interactive.transition.fast,
                        "hover:bg-danger/10",
                      )}
                      aria-label={t("characters.scheduledNotes.deleteNoteAria")}
                    >
                      <Trash2 className="h-3.5 w-3.5" />
                    </button>
                  </div>
                </div>
              </div>
            );
          })
        )}
      </div>
    </div>
    {formSheet}
    </>
  );
}
