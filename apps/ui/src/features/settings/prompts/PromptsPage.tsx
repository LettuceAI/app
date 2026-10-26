import { useRef, useState, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useBlocker, useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { BookOpen, Brain, Check, Copy, Download, FileText, Heart, Image, LayoutList, List, MessageSquare, MessageSquareReply, MoreHorizontal, Pencil, Plus, RotateCcw, Search, Star, Trash2, Upload } from "lucide-react";
import { Badge, Button, confirm, EmptyState, FilterBar, icon, IconButton, ListGroup, ListRow, ListRowButton, Menu, MenuContent, MenuItem, MenuLabel, MenuSeparator, MenuTrigger, Page, Sheet, Skeleton, toast, useLongPress } from "@lettuceai/crisp";
import type { FileSource, ImportPreview_Serialize, PromptSummary } from "@/api/generated/bindings";
import { ApiFailure } from "@/api/client";
import { queryKeys } from "@/api/query-keys";
import { commitImport } from "@/features/library";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { usePromptText } from "./copy";
import { newPrompt, promptListQuery, toInput, usePromptAction } from "./data";
import { CAN_BE_DEFAULT, groupOf, KIND_GROUPS, KIND_LABEL, KIND_PURPOSE, type KindGroup } from "./presentation";
import { PromptEditor } from "./PromptEditor";
import type { Kind } from "./types";

const GROUP_ICON: Record<KindGroup, ReactNode> = {
  chat: <MessageSquare size={icon.lg} />, helpers: <MessageSquareReply size={icon.lg} />, memory: <Brain size={icon.lg} />,
  lorebook: <BookOpen size={icon.lg} />, images: <Image size={icon.lg} />, companion: <Heart size={icon.lg} />,
};
function originLine(prompt: PromptSummary, t: ReturnType<typeof usePromptText>["t"]) {
  const origin = prompt.origin;
  if (origin.type === "built_in") return t(origin.edited ? "Built in, edited" : "Built in");
  if (origin.type === "derived") return t(origin.source_deleted ? "Copy of a deleted prompt" : "Copy of {{name}}", { name: origin.source_name });
  return t(origin.type === "imported" ? "Imported" : "Your prompt");
}
function PromptRow({ prompt, flat, onOpen, onMenu }: { prompt: PromptSummary; flat: boolean; onOpen: () => void; onMenu: () => void }) {
  const { t } = usePromptText();
  const press = useLongPress(onMenu);
  const kind = prompt.kind as Kind;
  const description = prompt.origin.type !== "built_in" || flat ? `${t(KIND_LABEL[kind])} · ${originLine(prompt, t)}` : `${t(KIND_PURPOSE[kind])}${prompt.origin.edited ? ` · ${t("Edited")}` : ""}`;
  return <ListRowButton icon={GROUP_ICON[groupOf(kind)]} title={prompt.name} description={description} trailing={prompt.app_default ? <Badge tone="accent" dot>{t("Default")}</Badge> : undefined} {...press.handlers} onClick={press.guardClick(onOpen)} onContextMenu={(event) => { event.preventDefault(); onMenu(); }} />;
}

export function PromptsPage() {
  const { t } = usePromptText();
  const api = useSettingsApi();
  const client = useQueryClient();
  const { phone, back } = useSettingsLayout();
  const navigate = useNavigate();
  const router = useRouter();
  const search = useSearch({ from: "/settings/$section" });
  const pushed = useLocation({ select: (location) => location.state.promptPushed });
  const prompts = useQuery(promptListQuery(api));
  const action = usePromptAction();
  const mainRef = useRef<HTMLElement>(null);
  const [query, setQuery] = useState("");
  const [group, setGroup] = useState<KindGroup | "all">("all");
  const [view, setView] = useState<"grouped" | "flat">("grouped");
  const [menuFor, setMenuFor] = useState<PromptSummary | null>(null);
  const shownMenu = useClosingValue(menuFor).shown;
  const [creating, setCreating] = useState(false);
  const [importing, setImporting] = useState<{ source: FileSource; preview: ImportPreview_Serialize; operation: string } | null>(null);
  const importShown = useClosingValue(importing).shown;
  const [picking, setPicking] = useState(false);
  useBlocker({ shouldBlockFn: ({ action: historyAction }) => {
    if (historyAction !== "BACK" || search.prompt || action.isPending) return false;
    if (importing) { setImporting(null); return true; }
    if (creating) { setCreating(false); return true; }
    if (menuFor) { setMenuFor(null); return true; }
    return false;
  }, enableBeforeUnload: false });
  const items = prompts.data ?? [];
  const q = query.trim().toLocaleLowerCase();
  const visible = items.filter((prompt) => (group === "all" || groupOf(prompt.kind as Kind) === group) && (!q || `${prompt.name} ${t(KIND_LABEL[prompt.kind as Kind])}`.toLocaleLowerCase().includes(q)));
  const yours = visible.filter((prompt) => prompt.origin.type !== "built_in");
  const builtIns = visible.filter((prompt) => prompt.origin.type === "built_in");
  const flat = [...visible.filter((prompt) => prompt.app_default), ...yours.filter((prompt) => !prompt.app_default), ...builtIns.filter((prompt) => !prompt.app_default).sort((a, b) => KIND_GROUPS.findIndex((value) => value.id === groupOf(a.kind as Kind)) - KIND_GROUPS.findIndex((value) => value.id === groupOf(b.kind as Kind)))];
  const open = (id: string, source?: string) => {
    setMenuFor(null); setCreating(false);
    void navigate({ to: "/settings/$section", params: { section: "prompts" }, search: { prompt: id, ...(source ? { promptSource: source } : {}) }, state: { promptPushed: true }, resetScroll: false });
  };
  const close = () => {
    if (pushed) router.history.back();
    else void navigate({ to: "/settings/$section", params: { section: "prompts" }, search: {}, replace: true, resetScroll: false });
  };
  const duplicate = (prompt: PromptSummary) => action.mutate(async () => {
    const original = await api.call("promptGet", { prompt_id: prompt.id });
    const draft = newPrompt(original, t("{{name}} (copy)", { name: prompt.name }));
    const copied = await api.call("promptCreate", { client_operation_id: action.operation(`copy:${prompt.id}:${prompt.revision}`), source_prompt_id: prompt.id, prompt: toInput(draft) });
    client.setQueryData([...queryKeys.prompts.all, "detail", copied.prompt.id], copied);
    toast.success(t("Made a copy of {{name}}", { name: prompt.name }));
  });
  const remove = async (prompt: PromptSummary) => {
    if (!await confirm({ title: t("Delete {{name}}?", { name: prompt.name }), message: t("Characters and chats that use it switch to the app default. This can't be undone."), confirmLabel: t("Delete prompt"), cancelLabel: t("Cancel"), destructive: true })) return false;
    await action.mutateAsync(async () => {
      await api.call("promptDelete", { client_operation_id: action.operation(`delete:${prompt.id}:${prompt.revision}`), prompt_id: prompt.id, expected_revision: prompt.revision });
      toast.success(t("Deleted {{name}}", { name: prompt.name }));
    });
    return true;
  };
  const reset = async (prompt?: PromptSummary) => {
    if (!await confirm({ title: t(prompt ? "Reset {{name}}?" : "Reset all built-in prompts?", { name: prompt?.name }), message: t(prompt ? "Its entries go back to the app's text. The name you gave it stays." : "Every built-in prompt goes back to the app's text. Your own prompts and copies stay as they are."), confirmLabel: t(prompt ? "Reset" : "Reset all"), cancelLabel: t("Cancel") })) return;
    return action.mutateAsync(async () => {
      const result = await api.call("promptBuiltinReset", prompt ? { type: "one", prompt_id: prompt.id, expected_revision: prompt.revision, client_operation_id: action.operation(`reset:${prompt.id}:${prompt.revision}`) } : { type: "all", client_operation_id: action.operation("reset-all") });
      for (const value of result.prompts) client.setQueryData([...queryKeys.prompts.all, "detail", value.prompt.id], value);
      toast.success(t("Built-in prompts reset"));
      return result;
    });
  };
  const makeDefault = (prompt: PromptSummary) => action.mutate(async () => {
    const settings = await api.call("settingsGet");
    await api.call("promptAppDefaultSet", { client_operation_id: action.operation(`default:${prompt.id}:${settings.revision}`), prompt_id: prompt.id, expected_settings_revision: settings.revision });
    toast.success(t("{{name}} is now the default", { name: prompt.name }));
  });
  const exportPrompt = (prompt: PromptSummary) => action.mutate(async () => {
    const picked = await api.call("filesPickSave", { suggested_name: `${prompt.name.replace(/[^\p{L}\p{N}_-]+/gu, "_")}.uec`, kind: "json" });
    if (!picked.target) return;
    await api.call("promptExport", { id: prompt.id, target: picked.target });
    toast.success(t("Exported {{name}}", { name: prompt.name }));
  });
  const importPrompt = async () => {
    setPicking(true);
    try {
      const picked = await api.call("filesPickOpen", { kinds: ["json"], multiple: false });
      const source = picked.sources[0];
      if (!source) return;
      const preview = await api.call("importPreview", { source });
      if (preview.content.kind !== "prompt" || preview.content.purpose === "runtime_text") throw new ApiFailure("unsupported", "not an editable prompt", null);
      setImporting({ source, preview, operation: crypto.randomUUID() });
    } catch (error) { action.mutate(() => Promise.reject(error)); }
    finally { setPicking(false); }
  };
  const importCommit = () => action.mutate(async () => {
    if (!importing) return;
    const result = await commitImport(api, importing.source, importing.preview.digest, importing.operation);
    if (result.kind !== "imported") throw new ApiFailure("unsupported", "prompt import did not return an entity", null);
    setImporting(null);
    if (result.warnings.length) toast.warning(t("Imported with notes")); else toast.success(t("Imported {{name}}", { name: importing.preview.name }));
    open(result.id);
  });
  const row = (prompt: PromptSummary, isFlat = false) => <PromptRow key={prompt.id} prompt={prompt} flat={isFlat} onOpen={() => open(prompt.id)} onMenu={() => setMenuFor(prompt)} />;
  const body = prompts.isPending ? <div role="status" aria-label={t("Loading prompts")} className="space-y-7">{[3, 4, 2].map((rows, index) => <ListGroup key={index} label={<Skeleton className="h-3 w-28" />}>{Array.from({ length: rows }, (_, id) => <ListRow key={id} chevron title={<Skeleton className="h-5 w-40" />} description={<Skeleton className="h-4 w-64 max-w-full" />} />)}</ListGroup>)}</div>
    : prompts.isError ? <FailureState error={prompts.error} onRetry={() => void prompts.refetch()} />
    : visible.length === 0 ? <EmptyState icon={<Search size={icon.xl} />} title={t("No prompts match")} description={t("Names and what each prompt is for are searched.")} />
    : view === "flat" ? <div className={`grid gap-3 ${phone ? "" : "grid-cols-2 items-start"}`}>{flat.map((prompt) => <ListGroup key={prompt.id}>{row(prompt, true)}</ListGroup>)}</div>
    : <div className="space-y-7">{yours.length ? <ListGroup label={t("Yours")}>{yours.map((prompt) => row(prompt))}</ListGroup> : !q && (group === "all" || group === "chat") ? <ListGroup label={t("Yours")}><ListRow icon={<FileText size={icon.lg} />} title={t("No prompts of your own yet")} description={t("Copy a built-in one to change it, or import a preset")} trailing={<Button size="sm" variant="secondary" onClick={() => setCreating(true)}>{t("New prompt")}</Button>} /></ListGroup> : null}<div className={`grid gap-7 ${phone ? "grid-cols-1" : "grid-cols-2 items-start gap-x-6"}`}>{KIND_GROUPS.map((value) => { const list = builtIns.filter((prompt) => groupOf(prompt.kind as Kind) === value.id); return list.length > 0 && <ListGroup key={value.id} label={t(value.label)}>{list.map((prompt) => row(prompt))}</ListGroup>; })}</div></div>;
  const menu = shownMenu;
  const current = search.prompt ? items.find((prompt) => prompt.id === search.prompt) : undefined;
  return <div className="relative h-full min-h-0">
    <div className={search.prompt ? "hidden h-full" : "h-full"}>
      <Page mainRef={mainRef} title={t("Prompts")} meta={phone || !prompts.data ? undefined : t("Prompts count", { count: items.length })} {...back} searchValue={query} onSearchChange={setQuery} searchPlaceholder={t("Search prompts")} contentClassName="space-y-5 pb-24" filters={<FilterBar aria-label={t("What the prompt is for")} value={group} onChange={setGroup} options={[{ value: "all" as const, label: t("All") }, ...KIND_GROUPS.map((value) => ({ value: value.id, label: t(value.label) }))]} />} actions={<div className="flex items-center gap-1.5">{!phone && <Button variant="secondary" size="sm" leading={<Upload size={icon.sm} />} loading={picking} onClick={() => void importPrompt()}>{t("Import")}</Button>}{!phone && <Button variant="primary" size="sm" leading={<Plus size={icon.sm} />} onClick={() => setCreating(true)}>{t("New prompt")}</Button>}<Menu><MenuTrigger><IconButton label={t("More")}><MoreHorizontal size={icon.lg} /></IconButton></MenuTrigger><MenuContent align="end">{phone && <MenuItem icon={<Upload size={icon.md} />} disabled={picking} onSelect={() => void importPrompt()}>{t("Import")}</MenuItem>}<MenuLabel>{t("View")}</MenuLabel><MenuItem icon={<LayoutList size={icon.md} />} selected={view === "grouped"} onSelect={() => setView("grouped")}>{t("Grouped by purpose")}</MenuItem><MenuItem icon={<List size={icon.md} />} selected={view === "flat"} onSelect={() => setView("flat")}>{t("One list")}</MenuItem><MenuSeparator /><MenuItem icon={<RotateCcw size={icon.md} />} disabled={action.isPending} onSelect={() => void reset().catch(() => undefined)}>{t("Reset built-in prompts")}</MenuItem></MenuContent></Menu></div>}>
        {action.isError && <FailureState error={action.error} size="sm" />}{body}
        {phone && !prompts.isPending && <div className="pointer-events-none sticky bottom-4 flex justify-end"><Button className="pointer-events-auto shadow-raised" variant="primary" leading={<Plus size={icon.md} />} onClick={() => setCreating(true)}>{t("New prompt")}</Button></div>}
      </Page>
    </div>
    {search.prompt && <div className="absolute inset-0"><PromptEditor id={search.prompt} sourceId={search.promptSource} summary={current} onBack={close} onDuplicate={duplicate} onDelete={remove} onDefault={makeDefault} onReset={reset} onExport={exportPrompt} actionError={action.error} actionPending={action.isPending} /></div>}
    <Sheet open={menuFor !== null} onClose={() => setMenuFor(null)} title={menu?.name} description={menu ? `${t(KIND_LABEL[menu.kind as Kind])} · ${originLine(menu, t)}` : undefined}>
      {menu && <ListGroup><ListRowButton icon={<Pencil size={icon.lg} />} title={t("Edit")} description={t("Entries, placement and rules")} onClick={() => open(menu.id)} />{CAN_BE_DEFAULT(menu.kind as Kind) && <ListRowButton icon={<Star size={icon.lg} />} tone="accent" title={t(menu.app_default ? "Already the default" : "Set as default")} description={t("Used by characters that have no prompt of their own")} chevron={false} disabled={menu.app_default || action.isPending} trailing={menu.app_default ? <Check size={icon.md} /> : undefined} onClick={() => { setMenuFor(null); makeDefault(menu); }} />}<ListRowButton icon={<Copy size={icon.lg} />} title={t("Duplicate")} description={t("A copy you can change freely")} chevron={false} disabled={action.isPending} onClick={() => { setMenuFor(null); duplicate(menu); }} /><ListRowButton icon={<Download size={icon.lg} />} title={t("Export")} description={t("Save as a file to share or back up")} chevron={false} onClick={() => { setMenuFor(null); exportPrompt(menu); }} />{menu.origin.type === "built_in" ? <ListRowButton icon={<RotateCcw size={icon.lg} />} title={t("Reset to the app's text")} description={t(menu.origin.edited ? "Undo your edits to this built-in prompt" : "Nothing to reset, it hasn't been edited")} chevron={false} disabled={!menu.origin.edited || action.isPending} onClick={() => { setMenuFor(null); void reset(menu).catch(() => undefined); }} /> : <ListRowButton icon={<Trash2 size={icon.lg} />} tone="danger" title={t("Delete")} description={t("Characters using it switch to the app default")} chevron={false} disabled={action.isPending} onClick={() => { setMenuFor(null); void remove(menu).catch(() => undefined); }} />}</ListGroup>}
    </Sheet>
    <Sheet open={creating} onClose={() => setCreating(false)} title={t("New prompt")} description={t("Start from scratch or from a built-in prompt")}><div className="space-y-5"><ListGroup><ListRowButton icon={<FileText size={icon.lg} />} title={t("Blank")} description={t("One empty entry, for a one-on-one chat")} onClick={() => open("new")} /></ListGroup><ListGroup label={t("Copy a built-in prompt")}>{items.filter((prompt) => prompt.origin.type === "built_in" && groupOf(prompt.kind as Kind) === "chat").map((prompt) => <ListRowButton key={prompt.id} icon={GROUP_ICON.chat} title={prompt.name} description={t(KIND_LABEL[prompt.kind as Kind])} onClick={() => open("new", prompt.id)} />)}</ListGroup></div></Sheet>
    <Sheet open={importing !== null} dismissible={!action.isPending} onClose={() => setImporting(null)} title={t("Import prompt")} description={importShown?.preview.name} footer={<div className="flex items-center gap-2"><Button variant="ghost" disabled={action.isPending} onClick={() => void importPrompt()}>{t("Choose another file")}</Button><span className="flex-1" /><Button variant="primary" loading={action.isPending} onClick={importCommit}>{t("Add prompt")}</Button></div>}>
      {importShown?.preview.content.kind === "prompt" && <div className="space-y-5"><ListGroup><ListRow title={importShown.preview.content.name} description={t("Name")} /><ListRow title={t(KIND_LABEL[importShown.preview.content.purpose as Kind])} description={t("Used for")} /><ListRow title={t("Entries count", { count: importShown.preview.content.entries.length })} description={importShown.preview.content.entries.map((entry) => entry.name).join(", ")} /></ListGroup>{importShown.preview.warnings.length > 0 && <ListGroup label={t("Notes")}>{importShown.preview.warnings.map((warning, index) => <ListRow key={index} title={t(warning.code === "field_skipped" ? "An unsupported field was left out" : warning.code === "field_converted" ? "A field was converted" : "Import note")} description={warning.field} />)}</ListGroup>}{action.isError && <FailureState error={action.error} size="sm" />}</div>}
    </Sheet>
  </div>;
}
