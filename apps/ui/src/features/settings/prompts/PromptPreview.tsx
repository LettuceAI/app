import { useEffect, useMemo, useState } from "react";
import { useInfiniteQuery, useQuery, keepPreviousData } from "@tanstack/react-query";
import { EyeOff, Search, UserRound } from "lucide-react";
import { Avatar, Badge, Button, Collapsible, EmptyState, icon, ListGroup, ListRow, ListRowButton, SearchField, Sheet, Skeleton, Text } from "@lettuceai/crisp";
import { useBlocker } from "@tanstack/react-router";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { useSettingsApi } from "../data";
import { usePromptText } from "./copy";
import { toInput, usePromptDraft } from "./data";
import { ROLE_LABEL } from "./presentation";
import type { Entry, PreviewSelection, Prompt } from "./types";

export function PromptPreview({ phone, prompt, dirty, onOpenEntry, onPickerChange }: { phone: boolean; prompt: Prompt; dirty: boolean; onOpenEntry: (entry: Entry) => void; onPickerChange: (open: boolean) => void }) {
  const { t } = usePromptText();
  const api = useSettingsApi();
  const [character, setCharacter] = useState<PreviewSelection | null | undefined>(undefined);
  const [persona, setPersona] = useState<PreviewSelection | null | undefined>(undefined);
  const [picker, setPicker] = useState<"character" | "persona" | null>(null);
  const [query, setQuery] = useState("");
  const characters = useInfiniteQuery({ queryKey: [...queryKeys.characters.all, "prompt-preview", picker === "character" ? query : ""], initialPageParam: null as string | null, queryFn: ({ pageParam }) => api.call("charactersList", { cursor: pageParam, limit: 50, query: picker === "character" ? query || null : null, search_description: true, search_nickname: true }), getNextPageParam: (last) => last.next_cursor ?? null });
  const personas = useInfiniteQuery({ queryKey: [...queryKeys.personas.all, "prompt-preview", picker === "persona" ? query : ""], initialPageParam: null as string | null, queryFn: ({ pageParam }) => api.call("personasList", { cursor: pageParam, limit: 50, query: picker === "persona" ? query || null : null, sort: null, search_description: true, search_nickname: true }), getNextPageParam: (last) => last.next_cursor ?? null });
  const characterOptions = characters.data?.pages.flatMap((page) => page.items.map((item): PreviewSelection => ({ id: item.id, name: item.name, description: item.description ?? "", image: item.avatar?.preview_url ?? item.avatar?.url ?? null }))) ?? [];
  const personaOptions = personas.data?.pages.flatMap((page) => page.items.map((item): PreviewSelection => ({ id: item.id, name: item.title, description: item.description, image: item.avatar?.preview_url ?? item.avatar?.url ?? null }))) ?? [];
  if (character === undefined && characters.data && picker !== "character") setCharacter(characterOptions[0] ?? null);
  useEffect(() => { onPickerChange(picker !== null); return () => onPickerChange(false); }, [picker, onPickerChange]);
  useBlocker({ shouldBlockFn: ({ action }) => { if (action !== "BACK" || picker === null) return false; setPicker(null); return true; }, enableBeforeUnload: false });
  const chosenCharacter = character ?? null;
  const defaultPersona = personas.data?.pages[0]?.default_persona.persona_id;
  const defaultDetail = useQuery({ queryKey: [...queryKeys.personas.all, "detail", defaultPersona], queryFn: () => api.call("personaGet", { persona_id: defaultPersona! }), enabled: persona === undefined && !!defaultPersona });
  if (persona === undefined && personas.data && picker !== "persona") {
    if (!defaultPersona) setPersona(null);
    else if (defaultDetail.data) {
      const value = defaultDetail.data;
      const avatarId = value.draft.media.find((link) => link.slot === "avatar")?.asset_id;
      const avatar = value.assets.find((asset) => asset.asset_id === avatarId);
      setPersona({ id: value.id, name: value.draft.title, description: value.draft.description, image: avatar?.preview_url ?? avatar?.url ?? null });
    }
  }
  const chosenPersona = persona ?? null;
  const input = useMemo(() => toInput(prompt, true), [prompt]);
  const debounced = usePromptDraft(input);
  const selectionReady = character !== undefined && persona !== undefined;
  const canPreview = !!debounced.name && debounced.entries.some((entry) => !!entry.content.trim() || entry.image_slot !== null);
  const preview = useQuery({ queryKey: [...queryKeys.prompts.all, "preview", prompt.id, debounced, chosenCharacter?.id, chosenPersona?.id], queryFn: () => api.call("promptPreview", { prompt_id: prompt.id, draft: debounced, conversation_id: null, character_id: chosenCharacter?.id ?? null, persona_id: chosenPersona?.id ?? null }), enabled: selectionReady && canPreview, placeholderData: keepPreviousData, retry: false });
  const shown = useClosingValue(preview.data ?? null).shown;
  const before = shown?.entries.filter((entry) => entry.position === "relative") ?? [];
  const inside = shown?.entries.filter((entry) => entry.position !== "relative").toSorted((a, b) => b.depth - a.depth) ?? [];
  const skipped = shown?.skipped_entry_ids.flatMap((id) => { const entry = prompt.entries.find((value) => value.id === id); return entry ? [entry] : []; }) ?? [];
  const pick = (kind: "character" | "persona") => { setQuery(""); setPicker(kind); };
  const controls = <div className="space-y-4"><ListGroup label={t("Render with")}>
    {characters.isPending ? <ListRow title={<Skeleton className="h-5 w-32" />} description={<Skeleton className="h-4 w-20" />} /> : <ListRowButton icon={chosenCharacter ? <Avatar name={chosenCharacter.name} src={chosenCharacter.image ?? null} size="sm" /> : <UserRound size={icon.lg} />} title={chosenCharacter?.name ?? t("No character")} description={t("Character")} onClick={() => pick("character")} />}
    {personas.isPending ? <ListRow title={<Skeleton className="h-5 w-32" />} description={<Skeleton className="h-4 w-20" />} /> : <ListRowButton icon={chosenPersona ? <Avatar name={chosenPersona.name} src={chosenPersona.image ?? null} size="sm" /> : <UserRound size={icon.lg} />} title={chosenPersona?.name ?? t("No persona")} description={t("Persona")} onClick={() => pick("persona")} />}
  </ListGroup><Text variant="hint" className="px-1">{t("Memories, summary and lorebook use sample text.")}{dirty ? ` ${t("Shows your unsaved changes.")}` : ""}</Text>{characters.isError && <FailureState error={characters.error} onRetry={() => void characters.refetch()} />}{persona === undefined && defaultDetail.isError && <FailureState error={defaultDetail.error} onRetry={() => void defaultDetail.refetch()} />}{personas.isError && <FailureState error={personas.error} onRetry={() => void personas.refetch()} />}</div>;
  const block = (id: string, title: string, meta: string, body: string) => <button key={id} type="button" disabled={id === "condensed"} onClick={() => { const entry = prompt.entries.find((value) => value.id === id); if (entry) onOpenEntry(entry); }} className="block w-full rounded-xl border border-line bg-surface-1 p-3 text-left transition-colors hover:bg-fill-2"><div className="flex items-baseline justify-between gap-3"><p className="truncate text-sm font-medium text-fg">{title}</p><span className="shrink-0 text-2xs text-fg-3">{meta}</span></div><p className="mt-1.5 whitespace-pre-line text-sm leading-relaxed text-fg-2">{body || t("Empty")}</p></button>;
  const order = <div className="space-y-2" aria-busy={preview.isFetching || input !== debounced}>
    {preview.isError && <FailureState error={preview.error} onRetry={() => void preview.refetch()} />}
    {!canPreview ? <EmptyState size="sm" title={t("Write an entry to preview it")} description={t("Give the prompt a name and some content first")} /> : !shown ? <div role="status" aria-label={t("Rendering preview")} className="space-y-3">{[0, 1, 2].map((id) => <Skeleton key={id} shape="block" className="h-28 w-full" />)}</div> : <>
      {prompt.condense && before.length > 1 ? block("condensed", t("System message"), t("{{count}} entries joined", { count: before.length }), before.map((entry) => entry.text.trim()).filter(Boolean).join("\n\n")) : before.map((entry) => block(entry.entry_id, entry.name || t("Untitled entry"), t(ROLE_LABEL[entry.role]), entry.text))}
      <div className="flex items-center gap-3 px-1 py-1.5"><span className="h-px flex-1 bg-line" /><span className="text-2xs text-fg-3">{t("The chat")}</span><span className="h-px flex-1 bg-line" /></div>
      {inside.map((entry) => block(entry.entry_id, entry.name || t("Untitled entry"), entry.depth === 0 ? t("After the newest") : t("{{count}} from the newest", { count: entry.depth }), entry.text))}
    </>}
    {(preview.isFetching || input !== debounced) && shown && <Text variant="hint">{t("Updating preview")}</Text>}
  </div>;
  const left = skipped.length > 0 && <Collapsible defaultOpen={!phone} title={t("Left out this time ({{count}})", { count: skipped.length })} description={t("Turned off, or their rules didn't match")}><div className="divide-y divide-line">{skipped.map((entry) => <button key={entry.id} type="button" onClick={() => onOpenEntry(entry)} className="flex w-full items-start gap-3 px-4 py-3 text-left transition-colors hover:bg-fill-2"><EyeOff size={icon.md} className="mt-0.5 shrink-0 text-fg-4" /><span className="min-w-0"><span className="block truncate text-sm text-fg">{entry.name || t("Untitled entry")}</span><span className="block text-sm text-fg-3">{t(!entry.enabled && !entry.required ? "Turned off" : "Its rules or placement didn't match this preview")}</span></span></button>)}</div></Collapsible>;
  const options = picker === "persona" ? personaOptions : characterOptions;
  const result = picker === "persona" ? personas : characters;
  return <>
    {phone ? <div className="space-y-5">{controls}{order}{left}</div> : <div className="grid grid-cols-[minmax(300px,380px)_minmax(0,1fr)] items-start gap-8"><div className="min-w-0 space-y-5">{controls}{left}</div><div className="min-w-0">{order}</div></div>}
    <Sheet open={picker !== null} onClose={() => setPicker(null)} title={t(picker === "persona" ? "Preview with persona" : "Preview with character")}><div className="space-y-4"><SearchField value={query} onChange={setQuery} placeholder={t("Search")} />{result.isError && <FailureState error={result.error} onRetry={() => void result.refetch()} />}<ListGroup><ListRowButton icon={<UserRound size={icon.lg} />} title={t(picker === "persona" ? "No persona" : "No character")} description={t("Its entries render empty")} chevron={false} onClick={() => { if (picker === "persona") setPersona(null); else setCharacter(null); setPicker(null); }} />{options.map((option) => <ListRowButton key={option.id} icon={<Avatar name={option.name} src={option.image ?? null} size="sm" />} title={option.name} description={option.description} chevron={false} trailing={(picker === "persona" ? chosenPersona?.id : chosenCharacter?.id) === option.id ? <Badge tone="accent" dot>{t("Chosen")}</Badge> : undefined} onClick={() => { if (picker === "persona") setPersona(option); else setCharacter(option); setPicker(null); }} />)}</ListGroup>{!result.isPending && !result.isError && options.length === 0 && <EmptyState size="sm" icon={<Search size={icon.lg} />} title={t("No results")} />}{result.isPending && <Skeleton className="h-20 w-full" />}{result.hasNextPage && <Button variant="ghost" loading={result.isFetchingNextPage} onClick={() => void result.fetchNextPage()}>{t("Load more")}</Button>}</div></Sheet>
  </>;
}
