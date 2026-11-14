import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { Check, Image, ImagePlus, PenLine, Plus, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { icon, ListGroup, ListRow, ListRowButton, Page, Skeleton, Switch } from "@lettuceai/crisp";
import type { SettingsImageGenerationSettings } from "@/api/generated/bindings";
import { appStatusQuery } from "@/api/app-status-query";
import { queryKeys } from "@/api/query-keys";
import { modelsQuery } from "@/features/conversations";
import { meetsRequirement, ModelChoiceSheet, modelChoices, type ModelChoice, type ModelRequirement } from "@/entities/model";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { useSettingsWrite } from "../use-settings-write";
import { LocalImagePage, LocalImageSettings } from "./LocalImageSettings";
import { ImagePromptSetting, type ImagePromptKind } from "./ImagePromptSetting";

type ImagePicker = "avatar" | "scene" | "writer";
const modelFields = { avatar: "avatar_model_profile_id", scene: "scene_model_profile_id", writer: "scene_writer_model_profile_id" } as const;
const sceneModes = ["auto", "ask_first", "manual"] as const;

export function ImageSettingsPage() {
  const { t } = useTranslation("settings");
  const { t: tPlayground } = useTranslation("playground");
  const api = useSettingsApi();
  const { back } = useSettingsLayout();
  const write = useSettingsWrite(api);
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  const models = useQuery(modelsQuery(api));
  const accounts = useQuery({ queryKey: queryKeys.providers.accounts(), queryFn: () => api.call("providerAccountsList") });
  const status = useQuery(appStatusQuery(api));
  const router = useRouter();
  const navigate = useNavigate();
  const picker = useSearch({ from: "/settings/$section", select: (search) => search.imagePicker ?? null });
  const prompt = useSearch({ from: "/settings/$section", select: (search) => search.imagePrompt ?? null });
  const localView = useSearch({ from: "/settings/$section", select: (search) => search.localImageView ?? null });
  const pushed = useLocation({ select: (location) => location.state.imagePickerPushed });
  const [subscriptionError, setSubscriptionError] = useState<{ error: unknown } | null>(null);
  const { shown } = useClosingValue(picker);
  const failed = [settings, models, accounts, status].find((query) => query.isError);
  const ready = settings.data && models.data && accounts.data && status.data;
  const value = settings.data?.global.image_generation;
  const busy = failed !== undefined || subscriptionError !== null;
  useEffect(() => {
    let active = true;
    let stop: (() => void) | undefined;
    void api.subscribe((event) => {
      if (event.type === "database_reloaded") void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: {}, replace: true, resetScroll: false });
    }).then((unsubscribe) => {
      if (active) stop = unsubscribe;
      else unsubscribe();
    }).catch((error: unknown) => { if (active) setSubscriptionError({ error }); });
    return () => { active = false; stop?.(); };
  }, [api, navigate]);
  const close = () => {
    if (pushed) router.history.back();
    else void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: {}, replace: true, resetScroll: false });
  };
  const open = (next: ImagePicker) => {
    if (busy) return;
    void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: { imagePicker: next }, state: { imagePickerPushed: true }, resetScroll: false });
  };
  const change = (patch: Partial<SettingsImageGenerationSettings>) => {
    if (busy || !value) return;
    return write((latest) => ({ section: "image_generation", value: { ...latest.global.image_generation, ...patch } }));
  };
  const openPrompt = (imagePrompt: ImagePromptKind) => {
    if (busy) return;
    void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: { imagePrompt }, state: { imagePickerPushed: true }, resetScroll: false });
  };
  const addModel = () => { void navigate({ to: "/settings/models/new", replace: picker !== null }); };
  const mobile = status.data?.platform === "android" || status.data?.platform === "ios";
  const notLocalOnPhone = ({ account }: ModelChoice) => !(mobile && account.provider_kind.toLowerCase() === "sdcpp");
  const requirements: Record<ImagePicker, ModelRequirement> = {
    scene: { output: ["image"], allow: notLocalOnPhone },
    avatar: { input: ["image"], output: ["image"], allow: notLocalOnPhone },
    writer: { input: ["text", "image"], output: ["text"] },
  };
  const candidates = modelChoices(models.data?.models ?? [], accounts.data ?? []);
  const images = candidates.filter((choice) => meetsRequirement(choice, requirements.scene));
  const avatars = candidates.filter((choice) => meetsRequirement(choice, requirements.avatar));
  const writers = candidates.filter((choice) => meetsRequirement(choice, requirements.writer));
  const promptRow = (kind: ImagePromptKind, disabled = false) => {
    if (!value) return null;
    const model = kind === "avatar_generation" || kind === "avatar_edit_request"
      ? avatars.find(({ model }) => model.id === value.avatar_model_profile_id) ?? avatars[0]
      : kind === "scene_generation" ? images.find(({ model }) => model.id === value.scene_model_profile_id) ?? images[0] : undefined;
    const local = model?.account.provider_kind.toLowerCase() === "sdcpp";
    return <ImagePromptSetting key={kind} kind={kind} value={value} disabled={busy || disabled || local}
      {...(local ? { disabledReason: t("image.prompts.localDirect") } : {})}
      open={prompt === kind} onOpen={openPrompt} onClose={close} onChange={change} />;
  };
  const describe = (kind: ImagePicker) => {
    if (!value) return undefined;
    const id = value[modelFields[kind]];
    if (id === null) return t(kind === "writer" ? "image.writer.first" : "image.first");
    const chosen = (kind === "writer" ? writers : kind === "avatar" ? avatars : images).find(({ model }) => model.id === id);
    return chosen ? t("image.selected", { model: chosen.model.display_name, provider: chosen.account.label }) : t("image.unavailable");
  };
  const choose = (id: string | null) => {
    if (!picker) return;
    void change({ [modelFields[picker]]: id });
  };
  const selected = shown && value ? value[modelFields[shown]] : null;
  return (
    <>
      <div className={localView && !mobile ? "hidden" : "h-full min-h-0"}>
      <Page title={t("sections.imageGeneration.title")} {...back}>
        {subscriptionError && <FailureState error={subscriptionError.error} />}
        {failed && <FailureState error={failed.error} onRetry={() => void failed.refetch()} />}
        {!ready || !value ? !failed && (
          <div className="space-y-7" role="status" aria-label={t("image.loading")}>
            {[0, 1, 2].map((index) => <div key={index} className="space-y-3"><Skeleton className="h-4 w-28" /><Skeleton className="h-24 w-full" /></div>)}
          </div>
        ) : (
          <div className="@container">
            {!mobile && images.length > 0 && <ListGroup className="mb-7"><ListRowButton icon={<ImagePlus size={icon.lg} />} title={tPlayground("open")} description={tPlayground("openDescription")} onClick={() => { void navigate({ to: "/playground" }); }} /></ListGroup>}
            {images.length === 0 && <ListGroup className="mb-7"><ListRowButton icon={<Plus size={icon.lg} />} title={t("image.empty.title")} description={t("image.empty.body")} trailing={<span className="text-sm text-fg-3">{t("image.addModel")}</span>} onClick={addModel} /></ListGroup>}
            <div className="grid grid-cols-1 gap-7 @4xl:grid-cols-2 @4xl:items-start @4xl:gap-x-10">
              <div className="min-w-0 space-y-7">
                <ListGroup label={t("image.avatar.group")}>
                  <ListRow icon={<UserRound size={icon.lg} />} title={t("image.avatar.title")} description={t("image.avatar.body")} trailing={<Switch checked={value.avatar_enabled} onChange={(enabled) => { void change({ avatar_enabled: enabled }); }} disabled={busy} aria-label={t("image.avatar.title")} />} />
                  <ListRowButton icon={<Image size={icon.lg} />} title={t("image.avatar.model")} description={value.avatar_enabled ? describe("avatar") : t("image.avatar.disabled")} disabled={busy || !value.avatar_enabled || avatars.length === 0} onClick={() => open("avatar")} />
                  {promptRow("avatar_generation", !value.avatar_enabled)}
                  {promptRow("avatar_edit_request", !value.avatar_enabled)}
                </ListGroup>
                <ListGroup label={t("image.writer.group")}>
                  <ListRowButton icon={<PenLine size={icon.lg} />} title={t("image.writer.model")} description={describe("writer")} disabled={busy || writers.length === 0} onClick={() => open("writer")} />
                  {promptRow("scene_prompt_writer")}
                  {promptRow("design_reference_writer")}
                </ListGroup>
              </div>
              <div className="min-w-0 space-y-7">
                <ListGroup label={t("image.scene.group")}>
                  <ListRow icon={<Image size={icon.lg} />} title={t("image.scene.title")} description={t("image.scene.body")} trailing={<Switch checked={value.scene_enabled} onChange={(enabled) => { void change({ scene_enabled: enabled }); }} disabled={busy} aria-label={t("image.scene.title")} />} />
                  <ListRowButton icon={<Image size={icon.lg} />} title={t("image.scene.model")} description={value.scene_enabled ? describe("scene") : t("image.scene.disabled")} disabled={busy || !value.scene_enabled || images.length === 0} onClick={() => open("scene")} />
                  {promptRow("scene_generation", !value.scene_enabled)}
                  <ListRow icon={<Image size={icon.lg} />} title={t("image.scene.automatic.title")} description={t(value.scene_enabled ? "image.scene.automatic.body" : "image.scene.automatic.disabled")} trailing={<Switch checked={value.scene_automatic_enabled} onChange={(enabled) => { void change({ scene_automatic_enabled: enabled }); }} disabled={busy || !value.scene_enabled} aria-label={t("image.scene.automatic.title")} />} />
                </ListGroup>
                {value.scene_enabled && value.scene_automatic_enabled && <ListGroup label={t("image.modes.group")}>
                  {sceneModes.map((mode) => <ListRowButton key={mode} title={t(`image.modes.${mode}.title`)} description={t(`image.modes.${mode}.body`)} chevron={false} aria-pressed={value.scene_mode === mode} trailing={value.scene_mode === mode ? <Check size={icon.md} /> : undefined} disabled={busy} onClick={() => { void change({ scene_mode: mode }); }} />)}
                </ListGroup>}
              </div>
            </div>
            {!mobile && <div className="mt-7"><LocalImageSettings /></div>}
          </div>
        )}
      </Page>
      </div>
      {localView && !mobile && <LocalImagePage view={localView} />}
      {shown && ready && <ModelChoiceSheet key={shown} api={api} open={picker !== null} onClose={close} title={t(`image.${shown}.model`)} value={selected} requirement={requirements[shown]} nullChoice={{ title: t(shown === "writer" ? "image.writer.first" : "image.first"), description: t(shown === "writer" ? "image.writer.firstBody" : "image.firstBody") }} busy={busy} onChange={choose} onAddModel={addModel} />}
    </>
  );
}
