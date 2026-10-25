import { AssetAudio } from "@/shared/ui/AssetAudio";
import { useCallback, useRef, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { AudioLines, BookOpen, Check, Download, Mic, MoreHorizontal, Pencil, Plus, Upload, Wand2 } from "lucide-react";
import { Alert, Badge, Button, Collapsible, EmptyState, icon, IconButton, ListGroup, ListRow, ListRowButton, Menu, MenuContent, MenuItem, MenuTrigger, Page, Input, Field, SearchField, Segmented, SelectMenu, Sheet, Skeleton, Tab, TabList, Tabs, Text, toast } from "@lettuceai/crisp";
import { appStatusQuery } from "@/api/app-status-query";
import type { AssetRef, AsrCorrectionView, AsrIgnoredSuggestionView, JobView, TranscribeOptions } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useDownloads } from "@/entities/download";
import { terminalJobStates } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { microphoneOptions, allScopes, asSuggestion, languageName, libraryFilter, runOptions, useSpeechAction, useSpeechData } from "./data";
import { whisperLanguages } from "./languages";
import { CorrectionRow } from "./CorrectionRow";
import { CorrectionSheet, ExampleSheet, WordSheet } from "./LibrarySheets";
import { ModelPicker } from "./ModelPicker";
import { ModelTab } from "./ModelTab";
import { SpeechDownload } from "./SpeechDownload";

type SpeechTab = "model" | "vocabulary" | "corrections" | "examples";
type Panel = "models" | "microphone" | "word" | "correction" | "example";

export function SpeechPage() {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const { t: settingsText } = useTranslation("settings");
  const api = useSettingsApi();
  const { phone, back } = useSettingsLayout();
  const navigate = useNavigate();
  const router = useRouter();
  const search = useSearch({ from: "/settings/$section" });
  const pushed = useLocation({ select: (location) => location.state.speechPanelPushed });
  const tab = search.speechTab ?? "model";
  const panel = search.speechPanel;
  const itemId = search.speechItem ?? "new";
  const mainRef = useRef<HTMLElement>(null);
  const status = useQuery(appStatusQuery(api));
  const [scope, setScope] = useState("all");
  const [customScope, setCustomScope] = useState(false);
  const [scopeDraft, setScopeDraft] = useState("");
  const data = useSpeechData(api, scope);
  const devices = useQuery({ queryKey: [...queryKeys.speech.all, "microphones"], queryFn: () => api.call("microphoneDevices") });
  const action = useSpeechAction();
  const downloads = useDownloads();
  const [query, setQuery] = useState("");
  const [language, setLanguage] = useState("all");
  const [correctionView, setCorrectionView] = useState<"approved" | "suggested" | "ignored">("approved");
  const [testExample, setTestExample] = useState<{ audio: AssetRef; whisperOutput: string | null } | null>(null);
  const [gpu, setGpu] = useState(true);
  const [keepLoaded, setKeepLoaded] = useState(true);
  const [accepted, setAccepted] = useState<Record<string, { model: string; job: JobView | null }>>({});
  const requests = useRef(new Set<string>());
  const [requesting, setRequesting] = useState<ReadonlySet<string>>(new Set());
  const models = data.models.data?.models ?? [];
  const gpuSupported = data.models.data?.gpu_supported === true;
  const active = models.find((model) => model.dictation) ?? null;
  const storedMic = status.data?.ui_state.speechMicrophoneId;
  const deviceId = typeof storedMic === "string" ? storedMic : null;
  const options: TranscribeOptions = { language: null, scopes: ["conversation", "global"], initial_prompt: null, translate: false, detect_language: false, threads: null, run: runOptions(gpuSupported && gpu), keep_model_loaded: keepLoaded };
  const close = () => {
    if (pushed) router.history.back();
    else void navigate({ to: "/settings/$section", params: { section: "speech" }, search: { speechTab: tab }, replace: true, resetScroll: false });
  };
  const open = (next: Panel, id?: string, recording?: { audio: AssetRef; whisperOutput: string | null }) => { if (next === "example") setTestExample(recording ?? null); void navigate({ to: "/settings/$section", params: { section: "speech" }, search: { speechTab: tab, speechPanel: next, ...(id ? { speechItem: id } : {}) }, state: { speechPanelPushed: true }, resetScroll: false }); };
  const changeTab = (next: string) => { setQuery(""); void navigate({ to: "/settings/$section", params: { section: "speech" }, search: { speechTab: next as SpeechTab }, replace: true, resetScroll: false }); mainRef.current?.scrollTo({ top: 0 }); };
  const chooseDevice = (id: string | null) => action.mutate(() => api.call("appUiStateUpdate", { patch: { speechMicrophoneId: id } }), { onSuccess: () => { if (panel === "microphone") close(); } });
  const tracked = new Map<string, { model: string; job: JobView | null }>();
  for (const job of downloads.downloads) if (job.subject_detail?.type === "whisper_model_install") tracked.set(job.id, { model: job.subject_detail.model_id, job });
  for (const [id, value] of Object.entries(accepted)) tracked.set(id, value);
  const pending = new Set([...requesting, ...[...tracked.values()].filter((value) => !value.job || !terminalJobStates.has(value.job.state)).map((value) => value.model)]);
  const updateDownload = useCallback((job: JobView) => setAccepted((previous) => {
    const known = previous[job.id];
    if (!known || known.job === job) return previous;
    return { ...previous, [job.id]: { ...known, job } };
  }), []);
  const getModel = (id: string) => {
    if (pending.has(id) || requests.current.has(id) || action.isPending) return;
    requests.current.add(id);
    setRequesting((previous) => new Set(previous).add(id));
    action.mutate(async () => { const result = await api.call("whisperDownload", { model_id: id }); setAccepted((previous) => ({ ...previous, [result.job_id]: { model: id, job: null } })); }, { onSettled: () => { requests.current.delete(id); setRequesting((previous) => { const next = new Set(previous); next.delete(id); return next; }); } });
  };
  const transfer = (kind: "import" | "export") => action.mutate(async () => {
    if (kind === "import") {
      const source = (await api.call("filesPickOpen", { kinds: ["json"], multiple: false })).sources[0];
      if (!source) return;
      const result = await api.call("asrLearningImport", { client_operation_id: crypto.randomUUID(), source });
      toast.success(t("imported", { words: result.vocabulary_count, corrections: result.correction_count, examples: result.voice_example_count }));
    } else {
      const { target } = await api.call("filesPickSave", { suggested_name: "speech-learning.json", kind: "json" });
      if (!target) return;
      await api.call("asrLearningExport", { target, filter: { ...libraryFilter, scopes: scope === "all" ? allScopes : [scope], language: language === "all" ? null : language } }); toast.success(t("exported"));
    }
  });
  const needed = tab === "model" ? [data.models, status] : tab === "vocabulary" ? [data.words] : tab === "corrections" ? [data.corrections, data.ignored] : [data.examples];
  const failure = needed.find((value) => value.isError);
  const loading = needed.some((value) => value.isPending);
  const words = data.words.data ?? [];
  const corrections = data.corrections.data ?? [];
  const ignored = data.ignored.data ?? [];
  const examples = data.examples.data ?? [];
  const approved = corrections.filter((value) => value.user_approved);
  const suggested = corrections.filter((value) => !value.user_approved);
  const matches = (text: string, lang: string | null, where: string) => (!query.trim() || text.toLocaleLowerCase().includes(query.trim().toLocaleLowerCase())) && (language === "all" || lang === language || lang === null) && (scope === "all" || scope === where);
  const scopeName = (value: string) => allScopes.includes(value) ? t(value as "global") : value;
  const describe = (lang: string | null, where: string) => `${scopeName(where)} · ${lang ? languageName(lang) : t("anyLanguage")}`;
  const groups = new Map<string, typeof words>();
  for (const word of [...words].sort((a, b) => b.priority - a.priority)) if (matches(`${word.term} ${word.category ?? ""}`, word.language, word.scope)) { const key = word.category || t("ungrouped"); groups.set(key, [...(groups.get(key) ?? []), word]); }
  const previewFilter = { language: language === "all" ? null : language, scopes: scope === "all" ? allScopes : [scope], user_approved_only: null };
  const preview = useQuery({ queryKey: [...queryKeys.speech.all, "preview", previewFilter], queryFn: () => api.call("asrPromptPreview", previewFilter), enabled: tab === "vocabulary" });
  const toolbar = (kind: "word" | "correction" | "example", label: "addWord" | "addCorrection" | "addExample", placeholder: "searchWords" | "searchCorrections" | "searchExamples") => (
    <div className={phone ? "grid grid-cols-2 gap-2" : "flex flex-wrap gap-2"}>
      <div className={phone ? "col-span-2" : "min-w-48 flex-1"}><SearchField value={query} onChange={setQuery} placeholder={t(placeholder)} /></div>
      <Field label={<span className="sr-only">{t("language")}</span>} className={phone ? "w-full space-y-0" : "w-44 space-y-0"}>
        <SelectMenu options={[{ value: "all", label: t("allLanguages") }, ...whisperLanguages.map((value) => ({ value: value.code, label: value.name }))]} value={language} onChange={(value) => setLanguage(value ?? "all")} />
      </Field>
      <Field label={<span className="sr-only">{t("where")}</span>} className={phone ? "w-full space-y-0" : "w-36 space-y-0"}>
        <SelectMenu options={[{ value: "all", label: t("allScopes") }, ...allScopes.map((value) => ({ value, label: scopeName(value) })), { value: "__custom__", label: t("customScope") }]} value={customScope ? "__custom__" : scope} onChange={(value) => { setCustomScope(value === "__custom__"); if (value && value !== "__custom__") setScope(value); }} />
      </Field>
      {customScope && <Field label={<span className="sr-only">{t("scopeName")}</span>} className={phone ? "col-span-2 space-y-0" : "w-40 space-y-0"}>
        <Input value={scopeDraft} placeholder={t("scopeName")} onChange={(event) => setScopeDraft(event.target.value)} onBlur={() => { if (scopeDraft.trim()) setScope(scopeDraft.trim().toLowerCase()); }} onKeyDown={(event) => { if (event.key === "Enter" && scopeDraft.trim()) setScope(scopeDraft.trim().toLowerCase()); }} />
      </Field>}
      <Button variant="tonal" className={phone ? "col-span-2" : undefined} block={phone} leading={<Plus size={icon.sm} />} onClick={() => open(kind)}>{t(label)}</Button>
    </div>
  );
  const approve = (value: AsrCorrectionView | AsrIgnoredSuggestionView) => action.mutate(() => api.call("asrSuggestionApprove", { client_operation_id: crypto.randomUUID(), suggestion: asSuggestion(value) }));
  const ignore = (value: AsrCorrectionView) => action.mutate(() => api.call("asrSuggestionIgnore", { client_operation_id: crypto.randomUUID(), suggestion: asSuggestion(value) }));
  const list = (correctionView === "approved" ? approved : correctionView === "suggested" ? suggested : ignored).filter((value) => matches(`${value.wrong} ${value.correct}`, value.language, value.scope));
  const emptyMatches = <EmptyState icon={<BookOpen size={icon.xl} />} title={t("nothingMatches")} description={t("nothingMatchesBody")} />;
  const vocabulary = <div className="space-y-5"><Text variant="hint" className="px-1">{t("wordHint")}</Text>{toolbar("word", "addWord", "searchWords")}{words.length === 0 ? <EmptyState icon={<BookOpen size={icon.xl} />} title={t("noWords")} description={t("noWordsBody")} action={<Button variant="tonal" leading={<Plus size={icon.sm} />} onClick={() => open("word")}>{t("addWord")}</Button>} /> : !groups.size ? emptyMatches : <div className={phone ? "grid gap-6" : "grid grid-cols-2 items-start gap-6"}>{[...groups.entries()].map(([category, values]) => <ListGroup key={category} label={category}>{values.map((word) => <ListRowButton key={word.id} title={word.term} description={describe(word.language, word.scope)} trailing={<Badge>{word.priority}</Badge>} onClick={() => open("word", word.id)} />)}</ListGroup>)}</div>}{words.length > 0 && <Collapsible title={t("preview")} description={t("previewBody")}>{preview.isError ? <FailureState error={preview.error} onRetry={() => void preview.refetch()} size="sm" /> : preview.isPending ? <Skeleton className="h-12 w-full" /> : <p className="whitespace-pre-wrap font-mono text-sm leading-relaxed text-fg-2">{preview.data || t("emptyPreview")}</p>}</Collapsible>}</div>;
  const correctionsTab = <div className="space-y-5"><Segmented block value={correctionView} onChange={setCorrectionView} options={[{ value: "approved", label: `${t("approved")} (${approved.length})` }, { value: "suggested", label: `${t("suggested")} (${suggested.length})` }, { value: "ignored", label: `${t("ignored")} (${ignored.length})` }]} /><Text variant="hint" className="px-1">{t(`${correctionView}Hint`)}</Text>{toolbar("correction", "addCorrection", "searchCorrections")}{list.length > 0 ? <ListGroup>{list.map((value) => <CorrectionRow key={value.id} value={value} view={correctionView} phone={phone} busy={action.isPending} description={`${t(correctionView === "approved" ? "used" : "seen", { count: "use_count" in value && correctionView === "approved" ? value.use_count : "seen_count" in value ? value.seen_count : value.ignored_count })} · ${describe(value.language, value.scope)}`} onEdit={(id) => open("correction", id)} onApprove={approve} onIgnore={ignore} />)}</ListGroup> : <EmptyState icon={<Wand2 size={icon.xl} />} title={t(correctionView === "approved" ? "noCorrections" : correctionView === "suggested" ? "noSuggestions" : "nothingIgnored")} {...(correctionView !== "ignored" ? { description: t(correctionView === "approved" ? "noCorrectionsBody" : "noSuggestionsBody") } : {})} />}</div>;
  const shownExamples = examples.filter((value) => matches(`${value.expected_text} ${value.whisper_output ?? ""}`, value.language, value.scope));
  const examplesTab = <div className="space-y-5"><Text variant="hint" className="px-1">{t("examplesHint")}</Text>{toolbar("example", "addExample", "searchExamples")}{shownExamples.length ? <div className={phone ? "grid gap-3" : "grid grid-cols-2 items-start gap-3"}>{shownExamples.map((value) => { const same = value.whisper_output?.trim() === value.expected_text.trim(); return <AssetAudio key={value.id} className="min-w-0" src={value.audio.url} title={value.expected_text} description={value.whisper_output ? same ? t("getsRight") : t("heardText", { text: value.whisper_output }) : t("notTranscribed")} {...(value.whisper_output ? { badge: { label: t(same ? "matches" : "differs"), tone: same ? "accent" as const : "warning" as const } } : {})} actions={<IconButton label={t("edit")} onClick={() => open("example", value.id)}><Pencil size={icon.md} /></IconButton>} />; })}</div> : examples.length ? emptyMatches : <EmptyState icon={<AudioLines size={icon.xl} />} title={t("noExamples")} description={t("noExamplesBody")} action={<Button variant="tonal" onClick={() => open("example")}>{t("addExample")}</Button>} />}</div>;
  const selectedWord = words.find((value) => value.id === itemId);
  const selectedCorrection = corrections.find((value) => value.id === itemId);
  const selectedExample = examples.find((value) => value.id === itemId);
  const microphones = microphoneOptions(devices.data, deviceId, t("defaultMic"), t("micUnavailable"), (name) => t("micAmbiguous", { name }));
  return <>
    <Page mainRef={mainRef} title={settingsText("sections.speech.title")} meta={phone ? undefined : t("meta")} {...back} filters={<Tabs value={tab} onChange={changeTab}><TabList aria-label={t("sections")}><Tab value="model">{t("model")}</Tab><Tab value="vocabulary" {...(!phone ? { count: words.length } : {})}>{t("vocabulary")}</Tab><Tab value="corrections" {...(suggested.length ? { count: suggested.length } : {})}>{t("corrections")}</Tab><Tab value="examples" {...(!phone ? { count: examples.length } : {})}>{t("examples")}</Tab></TabList></Tabs>} actions={<Menu><MenuTrigger><IconButton label={t("more")}><MoreHorizontal size={icon.lg} /></IconButton></MenuTrigger><MenuContent align="end"><MenuItem icon={<Upload size={icon.md} />} disabled={action.isPending} onSelect={() => transfer("import")}>{t("import")}</MenuItem><MenuItem icon={<Download size={icon.md} />} disabled={action.isPending} onSelect={() => transfer("export")}>{t("export")}</MenuItem></MenuContent></Menu>} contentClassName={tab === "model" ? "space-y-5" : "max-w-4xl space-y-5"}>
      {action.isError && <FailureState error={action.error} />}{devices.isError && panel !== "microphone" && <FailureState error={devices.error} onRetry={() => void devices.refetch()} size="sm" />}{failure && <FailureState error={failure.error} onRetry={() => void failure.refetch()} />}{downloads.error !== null && <FailureState error={downloads.error} onRetry={downloads.retry} size="sm" />}
      {loading ? <div className="grid gap-7 @4xl:grid-cols-2" role="status" aria-label={t("loading")}>{[1, 4].map((count, index) => <ListGroup key={index} label={<Skeleton className="h-4 w-24" />}>{Array.from({ length: count }, (_, row) => <ListRow key={row} title={<Skeleton className="h-5 w-32" />} description={<Skeleton className="h-4 w-48 max-w-full" />} trailing={<Skeleton className="h-6 w-10" />} />)}</ListGroup>)}</div> : !failure && <>{!active && data.models.isSuccess && tab !== "model" && <Alert tone="warning" title={t("noWhisper")} description={t("noWhisperBody")} action={<Button size="sm" variant="tonal" onClick={() => open("models")}>{t("chooseModel")}</Button>} />}{tab === "model" ? <ModelTab api={api} phone={phone} active={active} gpu={gpuSupported && gpu} gpuSupported={gpuSupported} keepLoaded={keepLoaded} onGpu={setGpu} onKeepLoaded={setKeepLoaded} deviceId={deviceId} devices={devices.data} onDevice={chooseDevice} onChoose={() => open("models")} onMicrophone={() => { void devices.refetch(); open("microphone"); }} onSaveRecording={(recording) => open("example", undefined, recording)} downloads={<div className="space-y-3">{[...tracked.entries()].map(([id, value]) => <SpeechDownload key={id} api={api} id={id} modelId={value.model} onUpdate={updateDownload} />)}</div>} /> : tab === "vocabulary" ? vocabulary : tab === "corrections" ? correctionsTab : examplesTab}</>}
    </Page>
    <ModelPicker api={api} open={panel === "models"} phone={phone} models={models} onClose={close} onDownload={getModel} pending={pending} />
    <Sheet open={panel === "microphone"} onClose={close} title={t("microphone")} description={t("microphoneBody")}><div className="space-y-3">{devices.isError && <FailureState error={devices.error} onRetry={() => void devices.refetch()} />}{devices.isPending ? <Skeleton className="h-24 w-full" /> : <ListGroup>{microphones.map((value) => <ListRowButton key={value.value} icon={<Mic size={icon.lg} />} title={value.label} chevron={false} disabled={value.disabled || action.isPending || !devices.data?.supported} trailing={value.value === (deviceId ?? "default") ? <Check size={icon.md} className="text-accent" /> : undefined} onClick={() => chooseDevice(value.value === "default" ? null : value.value)} />)}</ListGroup>}<Button variant="ghost" disabled={devices.isFetching} onClick={() => void devices.refetch()}>{t("refresh")}</Button></div></Sheet>
    {panel === "word" && (itemId === "new" || selectedWord) && <WordSheet key={`word:${itemId}`} api={api} open onClose={close} initial={selectedWord ?? null} />}
    {panel === "correction" && (itemId === "new" || selectedCorrection) && <CorrectionSheet key={`correction:${itemId}`} api={api} open onClose={close} initial={selectedCorrection ?? null} />}
    {panel === "example" && (itemId === "new" || selectedExample) && <ExampleSheet key={`example:${itemId}`} api={api} open onClose={close} initial={selectedExample ?? null} recordingSeed={testExample} deviceId={deviceId} modelId={active?.id ?? null} options={options} />}
    {(panel === "word" && data.words.isSuccess && itemId !== "new" && !selectedWord || panel === "correction" && data.corrections.isSuccess && itemId !== "new" && !selectedCorrection || panel === "example" && data.examples.isSuccess && itemId !== "new" && !selectedExample) && <Sheet open onClose={close} title={t("missingItem")}><Text variant="hint">{t("missingItem")}</Text></Sheet>}
  </>;
}
