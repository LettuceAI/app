import { useRef, useState, type ReactNode } from "react";
import { useBlocker } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Check, FileAudio, Trash2, Wand2 } from "lucide-react";
import { Alert, AudioItem, Button, confirm, Field, icon, IconButton, Input, ListGroup, ListRow, NumberInput, SelectMenu, Sheet, Switch, Text, Textarea, toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { AsrCorrectionView, AsrSuggestionView, AsrVocabularyView, AsrVoiceExampleView, AssetRef, TranscribeOptions } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useSpeechAction, allScopes } from "./data";
import { whisperLanguages } from "./languages";
import { RecordingControls, RecordingFailure } from "./RecordingControls";
import { useRecording } from "./use-recording";

type EditorProps = { api: ApiClient; open: boolean; onClose: () => void };
export function ScopeFields({ language, scope, onLanguage, onScope }: { language: string | null; scope: string; onLanguage: (value: string | null) => void; onScope: (value: string) => void }) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const languages = whisperLanguages.map((value) => ({ value: value.code, label: value.name }));
  if (language && !languages.some((item) => item.value === language)) languages.push({ value: language, label: language });
  return <div className="grid gap-4 @md:grid-cols-2">
    <Field label={t("language")} hint={t("languageHint")}><SelectMenu options={[{ value: "any", label: t("anyLanguage") }, ...languages]} value={language ?? "any"} onChange={(value) => onLanguage(value && value !== "any" ? value : null)} /></Field>
    <Field label={t("where")}><SelectMenu options={[...allScopes.map((value) => ({ value, label: t(value as "global") })), { value: "__custom__", label: t("customScope") }]} value={allScopes.includes(scope) ? scope : "__custom__"} onChange={(value) => { if (value) onScope(value === "__custom__" ? "" : value); }} /></Field>
    {!allScopes.includes(scope) && <Field label={t("scopeName")} required><Input value={scope} onChange={(event) => onScope(event.target.value)} /></Field>}
  </div>;
}
function useEditor(dirty: boolean) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const action = useSpeechAction();
  const leaving = useRef(false);
  const operation = useRef({ payload: "", id: "" });
  useBlocker({ shouldBlockFn: async () => {
    if (leaving.current) return false;
    if (action.isPending) return true;
    return dirty && !await confirm({ title: t("discardTitle"), message: t("discardBody"), confirmLabel: t("discard"), cancelLabel: t("keepEditing"), destructive: true });
  }, enableBeforeUnload: dirty });
  const operationId = (payload: unknown) => {
    const serialized = JSON.stringify(payload);
    if (operation.current.payload !== serialized || !operation.current.id) operation.current = { payload: serialized, id: crypto.randomUUID() };
    return operation.current.id;
  };
  const saved = (close: () => void) => { leaving.current = true; close(); };
  const remove = (name: string, run: () => Promise<unknown>, close: () => void) => {
    void confirm({ title: t("deleteTitle", { name }), message: t("deleteBody"), confirmLabel: t("delete"), cancelLabel: t("cancel"), destructive: true }).then((sure) => {
      if (sure) action.mutate(run, { onSuccess: () => saved(close) });
    });
  };
  return { action, operationId, saved, remove };
}
function EditorFooter({ isNew, valid, busy, onClose, onSave, onDelete }: { isNew: boolean; valid: boolean; busy: boolean; onClose: () => void; onSave: () => void; onDelete: () => void }) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  return <div className="flex items-center gap-2">{!isNew && <IconButton label={t("delete")} disabled={busy} onClick={onDelete}><Trash2 size={icon.md} /></IconButton>}<div className="flex-1" /><Button variant="ghost" disabled={busy} onClick={onClose}>{t("cancel")}</Button><Button variant="primary" disabled={!valid} loading={busy} onClick={onSave}>{t(isNew ? "add" : "save")}</Button></div>;
}
function EditorBody({ error, children }: { error: unknown; children: ReactNode }) {
  return <div className="space-y-4">{error !== null && <FailureState error={error} size="sm" />}{children}</div>;
}
export function WordSheet({ api, open, onClose, initial }: EditorProps & { initial: AsrVocabularyView | null }) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const seed = { term: initial?.term ?? "", category: initial?.category ?? "", language: initial?.language ?? null, scope: initial?.scope ?? "global", priority: initial?.priority ?? 50 };
  const [draft, setDraft] = useState(seed);
  const { action, operationId, saved, remove } = useEditor(JSON.stringify(draft) !== JSON.stringify(seed));
  const set = <K extends keyof typeof draft>(key: K, value: (typeof draft)[K]) => setDraft((previous) => ({ ...previous, [key]: value }));
  const save = () => {
    const payload = { ...draft, term: draft.term.trim(), category: draft.category.trim() || null, id: initial?.id ?? null, use_count: initial?.use_count ?? null };
    action.mutate(() => api.call("asrVocabularySave", { ...payload, client_operation_id: operationId(payload) }), { onSuccess: () => saved(onClose) });
  };
  return <Sheet open={open} onClose={onClose} title={t(initial ? "editWord" : "addWordTitle")} description={t("wordBody")} footer={<EditorFooter isNew={!initial} valid={!!draft.term.trim() && !!draft.scope.trim()} busy={action.isPending} onClose={onClose} onSave={save} onDelete={() => { if (initial) remove(draft.term, () => api.call("asrVocabularyDelete", { id: initial.id }), onClose); }} />}>
    <EditorBody error={action.error}><Field label={t("term")} required><Input value={draft.term} onChange={(event) => set("term", event.target.value)} autoFocus /></Field><Field label={t("group")} hint={t("groupHint")}><Input value={draft.category} onChange={(event) => set("category", event.target.value)} /></Field><ScopeFields language={draft.language} scope={draft.scope} onLanguage={(value) => set("language", value)} onScope={(value) => set("scope", value)} /><ListGroup><ListRow title={t("priority")} description={t("priorityBody")} trailing={<Field label={<span className="sr-only">{t("priority")}</span>} className="w-36 shrink-0 space-y-0"><NumberInput value={draft.priority} step={10} precision={0} onChange={(value) => set("priority", value)} /></Field>} /></ListGroup></EditorBody>
  </Sheet>;
}
export function CorrectionSheet({ api, open, onClose, initial }: EditorProps & { initial: AsrCorrectionView | null }) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const seed = { wrong: initial?.wrong ?? "", correct: initial?.correct ?? "", language: initial?.language ?? null, scope: initial?.scope ?? "global", user_approved: initial?.user_approved ?? true };
  const [draft, setDraft] = useState(seed);
  const { action, operationId, saved, remove } = useEditor(JSON.stringify(draft) !== JSON.stringify(seed));
  const set = <K extends keyof typeof draft>(key: K, value: (typeof draft)[K]) => setDraft((previous) => ({ ...previous, [key]: value }));
  const save = () => {
    const payload = { ...draft, wrong: draft.wrong.trim(), correct: draft.correct.trim(), id: initial?.id ?? null, confidence: initial?.confidence ?? null, use_count: initial?.use_count ?? null, accepted_count: initial?.accepted_count ?? null, rejected_count: initial?.rejected_count ?? null, seen_count: initial?.seen_count ?? null, last_seen_at: initial?.last_seen_at ?? null };
    action.mutate(() => api.call("asrCorrectionSave", { ...payload, client_operation_id: operationId(payload) }), { onSuccess: () => saved(onClose) });
  };
  return <Sheet open={open} onClose={onClose} title={t(initial ? "editCorrection" : "addCorrectionTitle")} description={t("correctionBody")} footer={<EditorFooter isNew={!initial} valid={!!draft.wrong.trim() && !!draft.correct.trim() && !!draft.scope.trim()} busy={action.isPending} onClose={onClose} onSave={save} onDelete={() => { if (initial) remove(`${draft.wrong} → ${draft.correct}`, () => api.call("asrCorrectionDelete", { id: initial.id }), onClose); }} />}>
    <EditorBody error={action.error}><div className="grid gap-4 @md:grid-cols-2"><Field label={t("wrong")} required><Input autoFocus value={draft.wrong} onChange={(event) => set("wrong", event.target.value)} /></Field><Field label={t("correct")} required><Input value={draft.correct} onChange={(event) => set("correct", event.target.value)} /></Field></div><ScopeFields language={draft.language} scope={draft.scope} onLanguage={(value) => set("language", value)} onScope={(value) => set("scope", value)} /><ListGroup><ListRow title={t("approved")} description={t(draft.user_approved ? "approvedBody" : "unapprovedBody")} trailing={<Switch checked={draft.user_approved} onChange={(value) => set("user_approved", value)} aria-label={t("approved")} />} /></ListGroup>{initial && <Text variant="hint">{t("correctionCounts", { uses: initial.use_count, seen: initial.seen_count })}</Text>}</EditorBody>
  </Sheet>;
}
export function ExampleSheet({ api, open, onClose, initial, recordingSeed, deviceId, modelId, options }: EditorProps & { initial: AsrVoiceExampleView | null; recordingSeed: { audio: AssetRef; whisperOutput: string | null } | null; deviceId: string | null; modelId: string | null; options: TranscribeOptions }) {
  const { t } = useTranslation("settings", { keyPrefix: "speech" });
  const seed = { expected_text: initial?.expected_text ?? "", whisper_output: initial?.whisper_output ?? recordingSeed?.whisperOutput ?? "", language: initial?.language ?? null, scope: initial?.scope ?? "global", correction_id: initial?.correction_id ?? null };
  const [draft, setDraft] = useState(seed);
  const [audio, setAudio] = useState<AssetRef | null>(initial?.audio ?? recordingSeed?.audio ?? null);
  const [suggestions, setSuggestions] = useState<AsrSuggestionView[] | null>(null);
  const recording = useRecording(api, deviceId);
  const { action, operationId, saved, remove } = useEditor(JSON.stringify(draft) !== JSON.stringify(seed) || (audio?.asset_id ?? null) !== (initial?.audio.asset_id ?? null) || recording.phase !== "idle");
  const busy = action.isPending || recording.phase !== "idle";
  const set = <K extends keyof typeof draft>(key: K, value: (typeof draft)[K]) => { setDraft((previous) => ({ ...previous, [key]: value })); setSuggestions(null); };
  const choose = () => action.mutate(async () => {
    const picked = await api.call("filesPickOpen", { kinds: ["audio"], multiple: false });
    if (!picked.sources[0]) return;
    const asset = await api.call("assetsIngest", { source: picked.sources[0], role: "voice_example" });
    setAudio(asset); set("whisper_output", "");
  });
  const save = () => {
    if (!audio) return;
    const payload = { ...draft, expected_text: draft.expected_text.trim(), whisper_output: draft.whisper_output.trim() || null, id: initial?.id ?? null, audio_asset_id: audio.asset_id, vocabulary_term_id: initial?.vocabulary_term_id ?? null };
    action.mutate(() => api.call("asrVoiceExampleSave", { ...payload, client_operation_id: operationId(payload) }), { onSuccess: () => saved(onClose) });
  };
  const transcribe = async () => { if (audio && modelId) { const result = await recording.transcribe(audio, modelId, { ...options, language: draft.language, scopes: [draft.scope] }); if (result) set("whisper_output", result.raw_text); } };
  const find = () => action.mutate(async () => { setSuggestions(await api.call("asrSuggestions", { before: draft.whisper_output, after: draft.expected_text, language: draft.language, scope: draft.scope })); });
  const promote = (suggestion: AsrSuggestionView) => action.mutate(async () => {
    const result = await api.call("asrSuggestionApprove", { client_operation_id: operationId(suggestion), suggestion });
    setDraft((previous) => ({ ...previous, correction_id: result.id })); setSuggestions(null); toast.success(t("savedCorrection"));
  });
  return <Sheet open={open} onClose={onClose} title={t(initial ? "editExample" : "addExampleTitle")} description={t("exampleBody")} size="lg" footer={<EditorFooter isNew={!initial} valid={!!audio && !!draft.expected_text.trim() && !!draft.scope.trim() && recording.phase === "idle"} busy={busy} onClose={onClose} onSave={save} onDelete={() => { if (initial) remove(initial.expected_text, () => api.call("asrVoiceExampleDelete", { id: initial.id }), onClose); }} />}>
    <EditorBody error={action.error}><section className="space-y-2"><Text variant="label">{t("recording")}</Text>{audio && recording.phase === "idle" ? <AudioItem src={audio.url} title={t("recording")} description={t("savedAudio")} actions={<IconButton label={t("removeRecording")} disabled={busy} onClick={() => { setAudio(null); recording.setAudio(null); set("whisper_output", ""); }}><Trash2 size={icon.md} /></IconButton>} /> : <><RecordingControls recording={recording} disabled={action.isPending} onStop={() => { void recording.stop().then((captured) => { if (captured) { setAudio(captured); set("whisper_output", ""); } }); }} />{recording.phase === "idle" && <Button block variant="ghost" leading={<FileAudio size={icon.sm} />} disabled={busy} onClick={choose}>{t("chooseFile")}</Button>}</>}</section><Field label={t("expected")} required><Textarea rows={2} value={draft.expected_text} onChange={(event) => set("expected_text", event.target.value)} /></Field><Field label={t("exampleHeard")} hint={modelId ? t("transcribeHint", { model: modelId }) : t("noTranscribeModel")}><Textarea rows={2} value={draft.whisper_output} onChange={(event) => set("whisper_output", event.target.value)} /></Field><Button size="sm" variant="ghost" leading={<Wand2 size={icon.sm} />} disabled={!audio || !modelId || busy} onClick={() => void transcribe()}>{t("transcribe")}</Button>{audio && recording.phase === "idle" && recording.error !== null && <RecordingFailure error={recording.error} />}{!!draft.whisper_output.trim() && !!draft.expected_text.trim() && draft.whisper_output.trim() !== draft.expected_text.trim() && <Button size="sm" variant="tonal" leading={<Wand2 size={icon.sm} />} disabled={busy} onClick={find}>{t("suggestCorrection")}</Button>}{suggestions?.map((suggestion) => <Alert key={`${suggestion.wrong}:${suggestion.correct}`} tone="info" title={t("suggestion")} description={`${suggestion.wrong} → ${suggestion.correct}`} action={<Button size="sm" variant="tonal" leading={<Check size={icon.sm} />} disabled={busy} onClick={() => promote(suggestion)}>{t("saveCorrection")}</Button>} />)}{suggestions?.length === 0 && <Text variant="hint">{t("noDiff")}</Text>}<ScopeFields language={draft.language} scope={draft.scope} onLanguage={(value) => set("language", value)} onScope={(value) => set("scope", value)} /></EditorBody>
  </Sheet>;
}
