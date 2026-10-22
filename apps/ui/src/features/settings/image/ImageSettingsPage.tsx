import { useEffect, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { Check, Image, PenLine, UserRound } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, EmptyState, icon, ListGroup, ListRow, ListRowButton, Page, Skeleton, Switch } from "@lettuceai/crisp";
import type { SettingsImageGenerationSettings } from "@/api/generated/bindings";
import { appStatusQuery } from "@/api/app-status-query";
import { queryKeys } from "@/api/query-keys";
import { modelsQuery } from "@/features/conversations";
import { useClosingValue } from "@/shared/hooks/useClosingValue";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { useSettingsWrite } from "../use-settings-write";
import { ImageModelSheet, type ImageModelChoice, type ImagePicker } from "./ImageModelSheet";
import { LocalImagePage, LocalImageSettings } from "./LocalImageSettings";

const modelFields = { avatar: "avatar_model_profile_id", scene: "scene_model_profile_id", writer: "scene_writer_model_profile_id" } as const;
const sceneModes = ["auto", "ask_first", "manual"] as const;

export function ImageSettingsPage() {
  const { t } = useTranslation("settings");
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
  const addModel = () => { void navigate({ to: "/settings/models/new", replace: picker !== null }); };
  const byAccount = new Map((accounts.data ?? []).map((account) => [account.id, account]));
  const candidates: ImageModelChoice[] = (models.data?.models ?? []).flatMap((model) => {
    const account = byAccount.get(model.provider_account_id);
    return account ? [{ model, account }] : [];
  });
  const mobile = status.data?.platform === "android" || status.data?.platform === "ios";
  const images = candidates.filter(({ model, account }) => model.output_scopes.includes("image") && !(mobile && account.provider_kind.toLowerCase() === "sdcpp"));
  const writers = candidates.filter(({ model }) => model.input_scopes.includes("text") && model.input_scopes.includes("image") && model.output_scopes.includes("text"));
  const describe = (kind: ImagePicker) => {
    if (!value) return undefined;
    const id = value[modelFields[kind]];
    if (id === null) return t(kind === "writer" ? "image.writer.first" : "image.first");
    const chosen = (kind === "writer" ? writers : images).find(({ model }) => model.id === id);
    return chosen ? t("image.selected", { model: chosen.model.display_name, provider: chosen.account.label }) : t("image.unavailable");
  };
  const choose = (id: string | null) => {
    if (!picker || busy) return;
    void change({ [modelFields[picker]]: id });
    close();
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
            {images.length === 0 && <EmptyState title={t("image.empty.title")} description={t("image.empty.body")} action={<Button variant="secondary" onClick={addModel}>{t("image.addModel")}</Button>} />}
            <div className="grid gap-7 @4xl:grid-cols-2 @4xl:items-start @4xl:gap-x-10">
              <div className="min-w-0 space-y-7">
                <ListGroup label={t("image.avatar.group")}>
                  <ListRow icon={<UserRound size={icon.lg} />} title={t("image.avatar.title")} description={t("image.avatar.body")} trailing={<Switch checked={value.avatar_enabled} onChange={(enabled) => { void change({ avatar_enabled: enabled }); }} disabled={busy} aria-label={t("image.avatar.title")} />} />
                  <ListRowButton icon={<Image size={icon.lg} />} title={t("image.avatar.model")} description={value.avatar_enabled ? describe("avatar") : t("image.avatar.disabled")} disabled={busy || !value.avatar_enabled} onClick={() => open("avatar")} />
                </ListGroup>
                <ListGroup label={t("image.writer.group")}>
                  <ListRowButton icon={<PenLine size={icon.lg} />} title={t("image.writer.model")} description={describe("writer")} disabled={busy} onClick={() => open("writer")} />
                </ListGroup>
              </div>
              <div className="min-w-0 space-y-7">
                <ListGroup label={t("image.scene.group")}>
                  <ListRow icon={<Image size={icon.lg} />} title={t("image.scene.title")} description={t("image.scene.body")} trailing={<Switch checked={value.scene_enabled} onChange={(enabled) => { void change({ scene_enabled: enabled }); }} disabled={busy} aria-label={t("image.scene.title")} />} />
                  <ListRowButton icon={<Image size={icon.lg} />} title={t("image.scene.model")} description={value.scene_enabled ? describe("scene") : t("image.scene.disabled")} disabled={busy || !value.scene_enabled} onClick={() => open("scene")} />
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
      {shown && ready && <ImageModelSheet key={shown} picker={shown} open={picker !== null} choices={shown === "writer" ? writers : images} value={selected} busy={busy} failure={failed ? { error: failed.error, retry: () => void failed.refetch() } : subscriptionError ? { error: subscriptionError.error, retry: () => window.location.reload() } : null} onChange={choose} onClose={close} onAdd={addModel} />}
    </>
  );
}
