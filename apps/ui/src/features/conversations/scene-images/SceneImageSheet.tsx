import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { Image, PenLine, Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Alert, Avatar, Button, Checkbox, EmptyState, Field, icon, ListGroup, ListRow, ListRowButton, LoadingState, Sheet, Spinner, Textarea } from "@lettuceai/crisp";
import { ApiFailure, type ApiClient } from "@/api/client";
import { appStatusQuery } from "@/api/app-status-query";
import type { SceneCastSelection, SceneCastView, TimelineMessage } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { accountsQuery, modelsQuery } from "../data/queries";
import { mediaOf, textOf } from "../data/messages";
import { useScenePrompt } from "./use-scene-prompt";

function castOf(view: SceneCastView): SceneCastSelection {
  return { character_ids: view.characters.filter((member) => member.selected).map((member) => member.character_id), include_persona: Boolean(view.persona?.selected) };
}

export function SceneImageSheet({ api, conversationId, message, busy, onClose }: { api: ApiClient; conversationId: string; message: TimelineMessage; busy: boolean; onClose: () => void }) {
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const navigate = useNavigate();
  const client = useQueryClient();
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  const models = useQuery(modelsQuery(api));
  const accounts = useQuery(accountsQuery(api));
  const status = useQuery(appStatusQuery(api));
  const castView = useQuery({ queryKey: queryKeys.conversations.sceneCast(conversationId, message.id), queryFn: () => api.call("messageSceneCast", { message_id: message.id }) });
  const [picked, setPicked] = useState<SceneCastSelection | null>(null);
  const [editor, setEditor] = useState(Boolean(message.scene_image?.prompt));
  const [prompt, setPrompt] = useState(message.scene_image?.prompt ?? "");
  const [submitting, setSubmitting] = useState(false);
  const [error, setError] = useState<ApiFailure | null>(null);
  const submittingRef = useRef(false);
  const alive = useRef(true);
  const attempt = useRef<{ key: string; operationId: string } | null>(null);
  const writer = useScenePrompt(api, message.id);
  const signature = JSON.stringify([message.candidate_index, message.parts]);
  const original = useRef(signature);
  useEffect(() => { if (signature !== original.current) onClose(); }, [signature, onClose]);
  useEffect(() => {
    alive.current = true;
    return () => { alive.current = false; };
  }, []);
  const configure = () => void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: {} });
  const failure = [settings, models, accounts, status, castView].find((query) => query.isError);
  const ready = settings.data && models.data && accounts.data && status.data && castView.data;
  const cast = picked ?? (castView.data ? castOf(castView.data) : null);
  const castEmpty = cast !== null && cast.character_ids.length === 0 && !cast.include_persona;
  const choices = (castView.data?.characters.length ?? 0) + (castView.data?.persona ? 1 : 0);
  const toggle = (id: string | null, on: boolean) => {
    if (!cast) return;
    setPicked(id === null ? { ...cast, include_persona: on } : { ...cast, character_ids: on ? [...cast.character_ids, id] : cast.character_ids.filter((current) => current !== id) });
  };
  const value = settings.data?.global.image_generation;
  const mobile = status.data?.platform === "android" || status.data?.platform === "ios";
  const candidates = (models.data?.models ?? []).filter((model) => accounts.data?.some((account) => account.id === model.provider_account_id && !(mobile && account.provider_kind === "sdcpp")));
  const model = value?.scene_model_profile_id ? candidates.find((model) => model.id === value.scene_model_profile_id && model.output_scopes.includes("image")) : candidates.find((model) => model.output_scopes.includes("image"));
  const local = accounts.data?.find((account) => account.id === model?.provider_account_id)?.provider_kind === "sdcpp";
  const writers = candidates.filter((model) => model.input_scopes.includes("text") && model.output_scopes.includes("text") && (local || model.input_scopes.includes("image")));
  const canWrite = value?.scene_writer_model_profile_id ? writers.some((model) => model.id === value.scene_writer_model_profile_id) : writers.length > 0;
  const canGenerate = Boolean(ready && !failure && value?.scene_enabled && model);
  const waiting = submitting || writer.writing;
  const active = message.scene_image && ["approved", "running", "awaiting_turn"].includes(message.scene_image.state);
  const write = async () => {
    if (!canWrite || !canGenerate || waiting || busy || active || castEmpty) return;
    setEditor(true);
    setError(null);
    const text = await writer.start(cast);
    if (text && alive.current) setPrompt(text);
  };
  const generate = async () => {
    const text = prompt.trim();
    if (!canGenerate || !text || waiting || submittingRef.current || busy || active || castEmpty) return;
    submittingRef.current = true;
    setSubmitting(true);
    setError(null);
    try {
      if (message.scene_image?.state === "pending") {
        await api.call("messageSceneImageApprove", { message_id: message.id, prompt: text, ...(cast ? { cast } : {}) });
      } else {
        const key = JSON.stringify([text, cast]);
        if (attempt.current?.key !== key) attempt.current = { key, operationId: crypto.randomUUID() };
        await api.call("messageSceneImageGenerate", { message_id: message.id, prompt: text, client_operation_id: attempt.current.operationId, ...(cast ? { cast } : {}) });
      }
      void client.invalidateQueries({ queryKey: queryKeys.conversations.messages(conversationId) });
      if (alive.current) onClose();
    } catch (error) {
      if (alive.current) setError(ApiFailure.fromRejection(error));
    } finally {
      submittingRef.current = false;
      if (alive.current) setSubmitting(false);
    }
  };
  const quote = textOf(message);
  return <Sheet open onClose={onClose} placement="auto" size="md" title={t(message.scene_image?.state === "pending" ? "sceneImage.review" : mediaOf(message).some((item) => item.kind === "image") ? "sceneImage.regenerate" : "sceneImage.title")} description={quote ? <span className="line-clamp-2">{quote}</span> : undefined}
    footer={canGenerate && editor ? <div className="flex gap-2">
      <Button variant="secondary" className="flex-1" onClick={onClose}>{common("actions.cancel")}</Button>
      <Button variant="primary" className="flex-1" leading={<Image size={icon.md} />} loading={submitting} disabled={waiting || busy || Boolean(active) || castEmpty || !prompt.trim()} onClick={() => void generate()}>{t("sceneImage.generate")}</Button>
    </div> : undefined}>
    {failure ? <FailureState error={failure.error} onRetry={() => void failure.refetch()} /> : !ready ? <LoadingState /> : !value?.scene_enabled || !model ? <EmptyState size="sm" title={t(!value?.scene_enabled ? "sceneImage.disabled" : "sceneImage.noModel")} description={t("sceneImage.setupHint")} action={<Button variant="secondary" onClick={configure}>{t("sceneImage.settings")}</Button>} /> : <div className="space-y-4 pb-1">
      {error && <Alert tone="danger" title={t("sceneImage.failed")} description={common(`errors.code.${error.code}`)} />}
      {writer.error && <Alert tone="danger" title={t("sceneImage.promptFailed")} description={common(`errors.code.${writer.error.code}`)} />}
      {writer.reason && <Alert tone="danger" title={t("sceneImage.promptFailed")} description={t(`sceneImage.writerError.${writer.reason}`, { defaultValue: t("sceneImage.writerError.failed") })} />}
      {(busy || active) && <Alert title={t(active ? "sceneImage.generating" : "menu.regenerateBusy")} />}
      {castView.data && cast && choices > 1 && <div className="space-y-2">
        <ListGroup label={t("sceneImage.cast")}>
          {castView.data.characters.map((member) => <ListRow key={member.character_id} icon={<Avatar src={member.avatar?.url ?? null} name={member.name} size="sm" />} title={<span className="block truncate">{member.name}</span>} trailing={<Checkbox checked={cast.character_ids.includes(member.character_id)} aria-label={member.name} disabled={waiting || Boolean(active)} onChange={(on) => toggle(member.character_id, on)} />} />)}
          {castView.data.persona && <ListRow icon={<Avatar src={castView.data.persona.avatar?.url ?? null} name={castView.data.persona.name} size="sm" />} title={<span className="block truncate">{castView.data.persona.name}</span>} description={t("sceneImage.castPersona")} trailing={<Checkbox checked={cast.include_persona} aria-label={castView.data.persona.name} disabled={waiting || Boolean(active)} onChange={(on) => toggle(null, on)} />} />}
        </ListGroup>
        <p className={castEmpty ? "text-sm text-danger" : "text-sm text-fg-3"}>{t(castEmpty ? "sceneImage.castEmpty" : "sceneImage.castHint")}</p>
      </div>}
      {editor && <Field label={t("sceneImage.prompt")} hint={t("sceneImage.promptHint")}>
        <Textarea rows={7} value={writer.writing ? writer.text : prompt} onChange={(event) => setPrompt(event.target.value)} disabled={waiting || Boolean(active)} aria-label={t("sceneImage.prompt")} placeholder={t("sceneImage.placeholder")} />
      </Field>}
      {writer.writing && <div role="status" className="flex items-center gap-2 text-sm text-fg-3"><Spinner size="sm" />{t(writer.thinking ? "sceneImage.thinking" : "sceneImage.writing")}</div>}
      <ListGroup>
        {!editor && <ListRowButton icon={<PenLine size={icon.lg} />} title={t("sceneImage.write")} description={t("sceneImage.writeHint")} disabled={waiting || busy || Boolean(active)} onClick={() => setEditor(true)} />}
        <ListRowButton icon={<Sparkles size={icon.lg} />} title={t(editor ? "sceneImage.rewrite" : "sceneImage.askAi")} description={t(canWrite ? "sceneImage.askAiHint" : "sceneImage.noWriter")} disabled={!canWrite || waiting || busy || Boolean(active) || castEmpty} onClick={() => void write()} />
        {!canWrite && <ListRowButton title={t("sceneImage.settings")} onClick={configure} />}
      </ListGroup>
    </div>}
  </Sheet>;
}
