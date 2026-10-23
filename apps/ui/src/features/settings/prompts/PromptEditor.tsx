import { useMemo, useRef, useState, type ReactNode } from "react";
import { AnimatePresence, motion, useIsPresent } from "framer-motion";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useNavigate } from "@tanstack/react-router";
import { Copy, Download, Layers, MoreHorizontal, Plus, RotateCcw, Star, Trash2 } from "lucide-react";
import { Alert, Badge, Button, confirm, Field, icon, IconButton, Input, ListGroup, ListRow, ListRowButton, Menu, MenuContent, MenuItem, MenuSeparator, MenuTrigger, motion as m, Page, ReorderableList, Sheet, Skeleton, Switch, Tab, TabList, Tabs, Text, toast } from "@lettuceai/crisp";
import type { PromptSummary, PromptView } from "@/api/generated/bindings";
import { ApiFailure, isApiFailure } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { usePromptText } from "./copy";
import { fromView, newPrompt, placeholdersQuery, promptQuery, toInput, usePromptAction, usePromptDraft } from "./data";
import { CAN_BE_DEFAULT, KIND_GROUPS, KIND_LABEL, POSITION, ROLE_LABEL, SLOT_LABEL } from "./presentation";
import { contradictions, describeRules } from "./rules";
import { PromptEntrySheet } from "./PromptEntrySheet";
import { PromptPreview } from "./PromptPreview";
import type { Entry, Prompt } from "./types";

type Actions = {
  onBack: () => void; onDuplicate: (prompt: PromptSummary) => void; onDelete: (prompt: PromptSummary) => Promise<boolean>;
  onDefault: (prompt: PromptSummary) => void; onReset: (prompt?: PromptSummary) => Promise<unknown>; onExport: (prompt: PromptSummary) => void;
  actionError: unknown; actionPending: boolean;
};
function placementLine(entry: Entry, t: ReturnType<typeof usePromptText>["t"]) {
  if (entry.position === "relative") return t(POSITION.relative.label);
  const from = entry.depth === 0 ? t("after the newest message") : t("{{count}} from the newest", { count: entry.depth });
  if (entry.position === "in_chat") return t("Inside the chat, {{from}}", { from });
  if (entry.position === "conditional") return t("From message {{count}}, {{from}}", { count: entry.minMessages ?? 1, from });
  return t("Every {{count}} messages, {{from}}", { count: entry.everyTurns ?? 3, from });
}
export function PromptEditor({ id, sourceId, summary, ...actions }: { id: string; sourceId?: string | undefined; summary?: PromptSummary | undefined } & Actions) {
  const { t } = usePromptText();
  const api = useSettingsApi();
  const loaded = useQuery(promptQuery(api, id === "new" ? sourceId ?? "new" : id));
  const initial = useMemo(() => id === "new" ? newPrompt(loaded.data, loaded.data ? t("{{name}} (copy)", { name: loaded.data.prompt.name }) : "") : loaded.data && fromView(loaded.data), [id, loaded.data, t]);
  if (loaded.isError) return <Page title={t("Prompt")} onBack={actions.onBack} backLabel={t("Prompts")}><FailureState error={loaded.error} onRetry={() => void loaded.refetch()} /></Page>;
  if (!initial || (id === "new" && sourceId && !loaded.data)) return <Page title={t("Prompt")} onBack={actions.onBack} backLabel={t("Prompts")}><div role="status" aria-label={t("Loading prompt")} className="space-y-6"><Skeleton className="h-10 w-64" />{[0, 1, 2].map((value) => <Skeleton key={value} shape="block" className="h-28 w-full" />)}</div></Page>;
  if (initial.kind === ("runtime_text" as string)) return <Page title={t("Prompt")} onBack={actions.onBack} backLabel={t("Prompts")}><Alert tone="info" title={t("This prompt belongs to the app runtime")} /></Page>;
  return <PromptEditorForm initial={initial} stored={loaded.data} summary={summary} {...actions} />;
}

function PromptEditorForm({ initial, stored, summary, onBack, onDuplicate, onDelete, onDefault, onReset, onExport, actionError, actionPending }: { initial: Prompt; stored?: PromptView | undefined; summary?: PromptSummary | undefined } & Actions) {
  const { t } = usePromptText();
  const api = useSettingsApi();
  const client = useQueryClient();
  const navigate = useNavigate();
  const { phone } = useSettingsLayout();
  const [saved, setSaved] = useState(initial);
  const [draft, setDraft] = useState(initial);
  const [tab, setTab] = useState<"entries" | "preview">("entries");
  const [editing, setEditing] = useState<Entry | "new" | null>(null);
  const [previewPicker, setPreviewPicker] = useState(false);
  const [kindPicker, setKindPicker] = useState(false);
  const [saveFailure, setSaveFailure] = useState<unknown>(null);
  const [undo, setUndo] = useState<{ entry: Entry; index: number } | null>(null);
  const scrollRef = useRef<HTMLElement>(null);
  const leaving = useRef(false);
  const action = usePromptAction();
  const registry = useQuery(placeholdersQuery(api, draft.kind));
  const isNew = saved.revision === null;
  const dirty = isNew || JSON.stringify(draft) !== JSON.stringify(saved);
  const builtIn = draft.origin.type === "built_in";
  const current = summary ?? stored?.prompt;
  const isDefault = current?.app_default ?? false;
  if (stored && stored.prompt.id === saved.id && !dirty && (stored.prompt.revision !== saved.revision || JSON.stringify(stored.prompt.origin) !== JSON.stringify(saved.origin))) {
    const next = fromView(stored); setSaved(next); setDraft(next);
  }
  useBlocker({ shouldBlockFn: async ({ action: historyAction }) => {
    if (leaving.current) return false;
    if (editing !== null || (historyAction === "BACK" && previewPicker)) return false;
    if (historyAction === "BACK" && kindPicker) { setKindPicker(false); return true; }
    if (!dirty) return false;
    return !await confirm({ title: t("Discard your changes?"), message: t("Your unsaved prompt changes will be lost."), confirmLabel: t("Discard"), cancelLabel: t("Keep editing"), destructive: true });
  }, enableBeforeUnload: dirty });
  const set = <K extends keyof Prompt>(key: K, value: Prompt[K]) => setDraft((valueBefore) => ({ ...valueBefore, [key]: value }));
  const setEntries = (entries: Entry[]) => set("entries", entries);
  const toggle = (id: string, on: boolean) => setEntries(draft.entries.map((entry) => entry.id === id ? { ...entry, enabled: on } : entry));
  const input = useMemo(() => toInput(draft), [draft]);
  const settled = usePromptDraft(input);
  const validation = useQuery({ queryKey: [...queryKeys.prompts.all, "validation", settled], queryFn: () => api.call("promptValidate", { prompt: settled }), enabled: !!settled.name && settled.entries.some((entry) => entry.content.trim() || entry.image_slot), retry: false });
  const missing = validation.data?.missing_placeholders ?? [];
  const failure = action.error ?? saveFailure ?? actionError;
  const missingFromFailure = isApiFailure(failure) && failure.details?.type === "prompt_missing_placeholders" ? failure.details.placeholders : [];
  const broken = draft.entries.filter((entry) => contradictions(entry.rules));
  const chars = draft.entries.reduce((count, entry) => count + entry.content.length, 0);
  const canSave = !!draft.name.trim() && draft.entries.every((entry) => entry.name.trim()) && draft.entries.some((entry) => entry.content.trim() || entry.imageSlot) && missing.length === 0 && broken.length === 0 && registry.data !== undefined && !registry.isError && validation.data !== undefined && !validation.isError && settled === input;
  const blocker = !draft.name.trim() ? t("Give it a name") : draft.entries.some((entry) => !entry.name.trim()) ? t("Name every entry") : missing.length ? t("Add the missing placeholders") : broken.length ? t("Fix the rules that can't both be true") : input !== settled || validation.isFetching ? t("Checking prompt") : !canSave ? t("Write at least one entry") : null;
  const save = () => {
    if (!canSave || action.isPending) return;
    setSaveFailure(null);
    const signature = JSON.stringify({ id: draft.id, revision: saved.revision, source: draft.sourceId, input });
    action.mutate(async () => {
      const checked = await api.call("promptValidate", { prompt: input });
      if (checked.missing_placeholders.length) throw new ApiFailure("invalid_input", "required prompt placeholders missing", { type: "prompt_missing_placeholders", placeholders: checked.missing_placeholders });
      const result = isNew ? await api.call("promptCreate", { client_operation_id: action.operation(signature), source_prompt_id: draft.sourceId, prompt: input }) : await api.call("promptUpdate", { client_operation_id: action.operation(signature), prompt_id: saved.id, expected_revision: saved.revision!, prompt: input });
      const next = fromView(result);
      client.setQueryData([...queryKeys.prompts.all, "detail", result.prompt.id], result);
      setSaved(next); setDraft(next); setUndo(null);
      toast.success(t(isNew ? "Prompt created" : "Changes saved"));
      if (isNew) {
        leaving.current = true;
        await navigate({ to: "/settings/$section", params: { section: "prompts" }, search: { prompt: result.prompt.id }, replace: true, resetScroll: false });
        leaving.current = false;
      }
    });
  };
  const reset = async () => {
    if (!current) return;
    try {
      const result = await onReset(current);
      if (!result) return;
      const view = await api.call("promptGet", { prompt_id: current.id });
      const next = fromView(view); setSaved(next); setDraft(next); setEditing(null); setUndo(null);
    } catch (error) { setSaveFailure(error); }
  };
  const remove = async () => {
    if (!current) return;
    try {
      if (await onDelete(current)) { leaving.current = true; onBack(); }
    } catch (error) { setSaveFailure(error); }
  };
  const entryRow = (entry: Entry, handle: ReactNode) => <div className={`group flex items-start gap-2 rounded-2xl border border-line bg-surface-1 p-3 transition-colors hover:bg-fill-2 @xl:p-4 ${!entry.enabled && !entry.required ? "opacity-60" : ""}`}>
    {!phone && <span className="mt-0.5">{handle}</span>}<button type="button" onClick={() => setEditing(entry)} className="min-w-0 flex-1 text-left"><div className="flex items-center gap-2"><p className="truncate text-base font-medium text-fg">{entry.name || t("Untitled entry")}</p>{entry.required && <Badge>{t("Required")}</Badge>}</div><p className="mt-0.5 truncate text-sm text-fg-3">{t("{{role}} · {{placement}}", { role: t(ROLE_LABEL[entry.role]), placement: placementLine(entry, t) })}{entry.imageSlot ? ` · ${t(SLOT_LABEL[entry.imageSlot])}` : ""}</p><p className="mt-1.5 line-clamp-2 whitespace-pre-line font-mono text-xs leading-relaxed text-fg-2">{entry.content || t("Empty")}</p>{(entry.rules.include.length > 0 || entry.rules.exclude.length > 0) && <p className="mt-1.5 line-clamp-2 text-sm text-fg-3">{describeRules(entry.rules, t)}</p>}</button>{phone && <span className="mt-0.5">{handle}</span>}{!entry.required && <Switch checked={entry.enabled} onChange={(on) => toggle(entry.id, on)} aria-label={t("{{name}} included", { name: entry.name || t("Untitled entry") })} className="mt-0.5" />}
  </div>;
  const header = <div className="space-y-6">
    {builtIn && <Alert tone="info" title={t("Built-in prompt")} description={t(saved.origin.type === "built_in" && saved.origin.edited ? "You've edited it. Reset brings back the app's text and keeps your name for it." : "Your edits are kept across updates. Reset brings back the app's text.")} action={saved.origin.type === "built_in" && saved.origin.edited ? <Button size="sm" variant="ghost" leading={<RotateCcw size={icon.sm} />} disabled={actionPending || action.isPending} onClick={() => void reset()}>{t("Reset")}</Button> : undefined} />}
    {draft.origin.type === "derived" && draft.origin.source_deleted && <Alert tone="info" title={t("Copied from a prompt that's gone")} description={t("It keeps working. It just has nothing to compare against.")} />}
    <Field label={t("Name")} required><Input value={draft.name} onChange={(event) => set("name", event.target.value)} placeholder={t("Slow burn")} /></Field>
    <ListGroup><ListRowButton icon={<Layers size={icon.lg} />} title={t("Used for")} description={builtIn ? t("{{purpose}}. Built-in prompts keep their purpose.", { purpose: t(KIND_LABEL[draft.kind]) }) : t(KIND_LABEL[draft.kind])} disabled={builtIn} chevron={!builtIn} onClick={() => setKindPicker(true)} /><ListRow title={t("Send as one message")} description={t("Joins the entries before the chat into a single system message")} trailing={<Switch checked={draft.condense} onChange={(value) => set("condense", value)} aria-label={t("Send as one message")} />} />{CAN_BE_DEFAULT(draft.kind) && !isNew && current && <ListRow title={t("App default")} description={t(isDefault ? "Characters without a prompt of their own use this one" : "Characters without a prompt of their own use another one")} trailing={isDefault ? <Badge tone="accent" dot>{t("Default")}</Badge> : <Button size="sm" variant="ghost" disabled={actionPending} onClick={() => onDefault(current)}>{t("Make default")}</Button>} />}</ListGroup>
    {registry.isError && <FailureState error={registry.error} onRetry={() => void registry.refetch()} />}
    {validation.isError && <FailureState error={validation.error} onRetry={() => void validation.refetch()} />}
    {missingFromFailure.length > 0 && <Alert tone="danger" title={t("Add the missing placeholders")} description={missingFromFailure.join(", ")} />}
    {missing.length > 0 && <Alert tone="danger" title={t("Missing placeholders count", { count: missing.length })} description={<span>{t("{{purpose}} prompts need these in an included entry:", { purpose: t(KIND_LABEL[draft.kind]) })} <span className="font-mono">{missing.join(", ")}</span></span>} />}
    {broken.length > 0 && <Alert tone="danger" title={t("Some rules can't both be true")} description={broken.map((entry) => entry.name || t("Untitled entry")).join(", ")} />}
    {(action.isError || !!saveFailure || !!actionError) && <FailureState error={action.error ?? saveFailure ?? actionError} size="sm" />}
  </div>;
  const entries = <section className="space-y-3"><div className="flex items-center justify-between gap-3 px-1"><Text variant="label">{t("Entries count", { count: draft.entries.length })}</Text><Button size="sm" variant="ghost" leading={<Plus size={icon.sm} />} onClick={() => setEditing("new")}>{t("Add entry")}</Button></div><ReorderableList aria-label={t("Entries")} items={draft.entries} onReorder={setEntries} keyOf={(entry) => entry.id} className="grid gap-3">{(entry, handle) => entryRow(entry, handle)}</ReorderableList><Text variant="hint" className="px-1">{t("Drag to reorder. Entries before the chat go in this order; the ones inside it go at their place in the messages.")} {t("Character count", { count: chars })}</Text>{undo && <Alert tone="info" title={t("Removed {{name}}", { name: undo.entry.name || t("the entry") })} action={<Button variant="ghost" size="sm" onClick={() => { setDraft((value) => ({ ...value, entries: [...value.entries.slice(0, undo.index), undo.entry, ...value.entries.slice(undo.index)] })); setUndo(null); }}>{t("Undo")}</Button>} />}</section>;
  const actions = <Menu><MenuTrigger><IconButton label={t("More actions")}><MoreHorizontal size={icon.lg} /></IconButton></MenuTrigger><MenuContent align="end">{CAN_BE_DEFAULT(draft.kind) && !isNew && current && <MenuItem icon={<Star size={icon.md} />} disabled={isDefault || actionPending} onSelect={() => onDefault(current)}>{t(isDefault ? "Already the default" : "Set as default")}</MenuItem>}<MenuItem icon={<Copy size={icon.md} />} disabled={isNew || actionPending} onSelect={() => { if (current) onDuplicate(current); }}>{t("Duplicate")}</MenuItem><MenuItem icon={<Download size={icon.md} />} disabled={isNew} onSelect={() => { if (current) onExport(current); }}>{t("Export")}</MenuItem>{!builtIn && !isNew && <><MenuSeparator /><MenuItem icon={<Trash2 size={icon.md} />} danger disabled={actionPending} onSelect={() => void remove()}>{t("Delete")}</MenuItem></>}</MenuContent></Menu>;
  const tabs = <Tabs value={tab} onChange={(value) => { setTab(value as typeof tab); scrollRef.current?.scrollTo({ top: 0 }); }}><TabList aria-label={t("Prompt sections")}><Tab value="entries" count={draft.entries.length}>{t("Entries")}</Tab><Tab value="preview">{t("Preview")}</Tab></TabList></Tabs>;
  return <div className="relative flex h-full min-h-0 flex-col"><Page mainRef={scrollRef} title={draft.name || t(isNew ? "New prompt" : "Prompt")} meta={phone ? undefined : t(KIND_LABEL[draft.kind])} onBack={onBack} backLabel={t("Prompts")} actions={actions} filters={tabs} contentClassName="pb-28 pt-4">{tab === "preview" && !!failure && <FailureState error={failure} size="sm" />}<AnimatePresence mode="wait" initial={false}><motion.div key={tab} initial={{ opacity: 0, y: 6 }} animate={{ opacity: 1, y: 0 }} exit={{ opacity: 0, y: -6 }} transition={m.quick}>{tab === "preview" ? <PromptPreview phone={phone} prompt={draft} dirty={dirty && !isNew} onOpenEntry={setEditing} onPickerChange={setPreviewPicker} /> : phone ? <div className="space-y-6">{header}{entries}</div> : <div className="grid grid-cols-[minmax(300px,380px)_minmax(0,1fr)] items-start gap-8"><div className="min-w-0">{header}</div><div className="min-w-0">{entries}</div></div>}</motion.div></AnimatePresence></Page>
    <AnimatePresence initial={false}>{dirty && <PromptSaveBar phone={phone}><div className="flex items-center gap-2">{!phone && <span className="flex-1 truncate text-sm text-fg-3">{blocker ?? t(isNew ? "Not saved yet" : "Unsaved changes")}</span>}<Button variant="ghost" className={phone ? "flex-1" : undefined} disabled={action.isPending} onClick={() => { action.reset(); setSaveFailure(null); if (isNew) { leaving.current = true; onBack(); } else { setDraft(saved); setUndo(null); } }}>{t("Discard")}</Button><Button variant="primary" className={phone ? "flex-1" : undefined} loading={action.isPending} disabled={!canSave || actionPending} onClick={save}>{t(isNew ? "Create prompt" : "Save")}</Button></div></PromptSaveBar>}</AnimatePresence>
    <PromptEntrySheet entry={editing} phone={phone} promptDirty={dirty} registry={registry.data} onClose={() => setEditing(null)} onDone={(entry) => { setEntries(draft.entries.some((value) => value.id === entry.id) ? draft.entries.map((value) => value.id === entry.id ? entry : value) : [...draft.entries, entry]); setEditing(null); }} onDelete={(entry) => { setUndo({ entry, index: draft.entries.findIndex((value) => value.id === entry.id) }); setEntries(draft.entries.filter((value) => value.id !== entry.id)); setEditing(null); }} />
    <Sheet open={kindPicker} onClose={() => setKindPicker(false)} title={t("Used for")} description={t("Decides which placeholders the prompt can use")}><div className="space-y-5">{KIND_GROUPS.map((group) => <ListGroup key={group.id} label={t(group.label)}>{group.kinds.map((kind) => <ListRowButton key={kind} title={t(KIND_LABEL[kind])} chevron={false} trailing={kind === draft.kind ? <Badge tone="accent" dot>{t("Chosen")}</Badge> : undefined} onClick={() => { set("kind", kind); setKindPicker(false); }} />)}</ListGroup>)}</div></Sheet>
  </div>;
}

function PromptSaveBar({ phone, children }: { phone: boolean; children: ReactNode }) {
  const present = useIsPresent();
  return <motion.div inert={!present} aria-hidden={!present} initial={{ y: 24, opacity: 0 }} animate={{ y: 0, opacity: 1 }} exit={{ y: 24, opacity: 0 }} transition={m.quick} className={`absolute inset-x-0 bottom-0 z-10 border-t border-line backdrop-blur ${phone ? "px-4 py-3" : "px-8 py-3"}`}>{children}</motion.div>;
}
