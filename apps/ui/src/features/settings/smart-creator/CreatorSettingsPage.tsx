import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Check, Globe, Radio, Wand2 } from "lucide-react";
import { icon, ListGroup, ListRow, ListRowButton, Page, Skeleton, Switch, Text } from "@lettuceai/crisp";
import type { SettingsCreationHelperSettings } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { ModelChoiceSheet } from "@/entities/model";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { useSettingsWrite } from "../use-settings-write";

type ToolFormat = SettingsCreationHelperSettings["tool_fallback"];
const formats: readonly ToolFormat[] = ["native", "json", "xml"];

function LoadingRows() {
  const { t } = useTranslation("creator");
  return (
    <div className="space-y-7" role="status" aria-label={t("settingsPage.loading")}>
      {[1, 2, 1, 3].map((count, index) => (
        <ListGroup key={index}>
          {Array.from({ length: count }, (_, row) => (
            <ListRow key={row} title={<Skeleton className="h-5 w-32" />} description={<Skeleton className="h-4 w-56 max-w-full" />} />
          ))}
        </ListGroup>
      ))}
    </div>
  );
}

export function CreatorSettingsPage() {
  const { t } = useTranslation("creator");
  const { t: common } = useTranslation("common");
  const { t: settingsText } = useTranslation("settings");
  const api = useSettingsApi();
  const { phone, back } = useSettingsLayout();
  const navigate = useNavigate();
  const router = useRouter();
  const pushed = useLocation({ select: (location) => location.state.creatorPanelPushed });
  const search = useSearch({ from: "/settings/$section" });
  const [busy, setBusy] = useState(false);
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  const models = useQuery({ queryKey: queryKeys.models.list(), queryFn: () => api.call("modelsList") });
  const accounts = useQuery({ queryKey: queryKeys.providers.accounts(), queryFn: () => api.call("providerAccountsList") });
  const write = useSettingsWrite(api);
  const value = settings.data?.global.creation_helper;

  const change = async (patch: Partial<SettingsCreationHelperSettings>) => {
    setBusy(true);
    try {
      return await write((current) => ({ section: "creation_helper", value: { ...current.global.creation_helper, ...patch } }));
    } finally {
      setBusy(false);
    }
  };
  const openModel = () =>
    void navigate({ to: "/settings/$section", params: { section: "smartCreator" }, search: { creatorModel: true }, state: { creatorPanelPushed: true }, resetScroll: false });
  const closeModel = () => {
    if (pushed) router.history.back();
    else void navigate({ to: "/settings/$section", params: { section: "smartCreator" }, search: {}, replace: true, resetScroll: false });
  };

  const modelId = value?.model_profile_id ?? null;
  const model = models.data?.models.find((item) => item.id === modelId);
  const account = model ? accounts.data?.find((item) => item.id === model.provider_account_id) : undefined;
  const native = value?.tool_fallback === "native";

  return (
    <Page title={settingsText("sections.smartCreator.title")} meta={phone ? undefined : settingsText("sections.smartCreator.subtitle")} {...back} contentClassName="space-y-7">
      {settings.isError && <FailureState error={settings.error} onRetry={() => void settings.refetch()} />}
      {!value ? (
        settings.isError ? null : <LoadingRows />
      ) : (
        <>
          <ListGroup>
            <ListRowButton icon={<Wand2 size={icon.lg} />} title={t("openCreator")} description={t("openCreatorBody")} onClick={() => void navigate({ to: "/creator" })} />
          </ListGroup>

          <ListGroup label={t("settingsPage.modelGroup")}>
            <ListRowButton
              icon={model && account ? <ModelMark name={model.display_name} modelId={model.external_model_id} providerKind={account.provider_kind} /> : <Globe size={icon.lg} />}
              title={t("model")}
              description={
                models.isPending || accounts.isPending ? (
                  <Skeleton className="h-4 w-32" />
                ) : model && account ? (
                  `${account.label} · ${model.display_name}`
                ) : modelId ? (
                  t("modelUnavailable")
                ) : (
                  t("appDefault")
                )
              }
              disabled={busy}
              onClick={openModel}
            />
          </ListGroup>

          <ListGroup label={t("settingsPage.repliesGroup")}>
            <ListRow
              icon={<Radio size={icon.lg} />}
              title={t("streaming")}
              description={native ? t("streamingBody") : t("settingsPage.streamingNeedsNative")}
              trailing={<Switch checked={native && value.streaming} disabled={busy || !native} aria-label={t("streaming")} onChange={(streaming) => void change({ streaming })} />}
            />
          </ListGroup>

          <section className="space-y-2">
            <ListGroup label={t("toolFormat")}>
              {formats.map((format) => (
                <ListRowButton
                  key={format}
                  title={format === "native" ? t("settingsPage.nativeTitle") : common(`structuredFallback.formats.${format}.title`)}
                  description={format === "native" ? t("settingsPage.nativeBody") : common(`structuredFallback.formats.${format}.body`)}
                  tone={format === value.tool_fallback ? "accent" : "neutral"}
                  chevron={false}
                  aria-pressed={format === value.tool_fallback}
                  disabled={busy}
                  trailing={format === value.tool_fallback ? <Check size={icon.md} className="text-accent" /> : undefined}
                  onClick={() => void change({ tool_fallback: format })}
                />
              ))}
            </ListGroup>
            <Text variant="hint" className="px-1">
              {t("settingsPage.toolFormatHint")}
            </Text>
          </section>
        </>
      )}
      <ModelChoiceSheet
        api={api}
        open={search.creatorModel === true}
        onClose={closeModel}
        title={t("model")}
        value={modelId}
        busy={busy}
        requirement={{ kind: "chat", input: ["text"], output: ["text"] }}
        nullChoice={{ title: t("appDefault") }}
        onAddModel={() => void navigate({ to: "/settings/models/new", replace: true })}
        onChange={async (model_profile_id) => {
          const saved = await change({ model_profile_id });
          if (!saved) return false;
        }}
      />
    </Page>
  );
}
