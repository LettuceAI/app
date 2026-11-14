import { useEffect, useMemo, useRef, useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { SlidersHorizontal, TerminalSquare } from "lucide-react";
import { AppBar, icon, IconButton, Resizable, Sheet, toast, useMediaQuery } from "@lettuceai/crisp";
import type { ImageGenerateRequest_Deserialize } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useApi } from "@/api/use-api";
import { watchJob } from "@/entities/job";
import { meetsRequirement, ModelChoiceSheet, modelChoices, type ModelRequirement } from "@/entities/model";
import { modelsQuery } from "@/features/conversations";
import { useOverlayBack } from "@/shared/hooks/useOverlayBack";
import { Feed } from "./Feed";
import { PromptPane } from "./PromptPane";
import { SettingsPane } from "./SettingsPane";
import {
  buildGenerateRequest,
  capabilitiesQuery,
  defaultStrength,
  historyEntries,
  historyQuery,
  playgroundKeys,
  regenerateRequest,
  upscalersQuery,
  useFailureText,
  type PlaygroundAsset,
  type PlaygroundEntry,
  type StartImage,
} from "./playground-data";
import { usePlaygroundDraft } from "./use-playground-draft";

const imageModels: ModelRequirement = { output: ["image"] };

export function PlaygroundPage() {
  const { t } = useTranslation("playground");
  const api = useApi();
  const navigate = useNavigate();
  const client = useQueryClient();
  const failureText = useFailureText();
  const wide = useMediaQuery("(min-width: 1024px)");
  const models = useQuery(modelsQuery(api));
  const accounts = useQuery({ queryKey: queryKeys.providers.accounts(), queryFn: () => api.call("providerAccountsList") });
  const choices = useMemo(
    () => modelChoices(models.data?.models ?? [], accounts.data ?? []).filter((choice) => meetsRequirement(choice, imageModels)),
    [models.data, accounts.data],
  );
  const { selected, draft, base, selectModel, update } = usePlaygroundDraft(choices);
  const capabilities = useQuery(capabilitiesQuery(api, selected?.model.id ?? null));
  const upscalers = useQuery(upscalersQuery(api));
  const history = useInfiniteQuery(historyQuery(api));
  const entries = useMemo(() => historyEntries(history.data?.pages), [history.data]);

  const [prompt, setPrompt] = useState("");
  const [negative, setNegative] = useState("");
  const [startImage, setStartImage] = useState<StartImage | null>(null);
  const [sheet, setSheet] = useState<"prompt" | "settings" | null>(null);
  const [modelPicker, setModelPicker] = useState(false);
  const [submitting, setSubmitting] = useState(false);
  const [upscaling, setUpscaling] = useState(false);
  const upscaleWatch = useRef<AbortController | null>(null);
  useEffect(() => () => upscaleWatch.current?.abort(), []);
  useOverlayBack(sheet !== null || modelPicker, () => {
    setSheet(null);
    setModelPicker(false);
  });

  const generating = submitting || entries.some((entry) => entry.status === "pending" && entry.job_id !== null);
  const caps = capabilities.data;
  const showNegative = caps?.negative_prompt === true;
  const showStartImage = caps !== undefined && selected !== null && (caps.local || caps.negative_prompt || selected.model.input_scopes.includes("image"));
  const canGenerate = prompt.trim().length > 0 && caps !== undefined && selected !== null && !generating;
  const refreshHistory = () => client.invalidateQueries({ queryKey: playgroundKeys.history });

  const submit = async (request: ImageGenerateRequest_Deserialize) => {
    setSubmitting(true);
    try {
      await api.call("imageGenerate", request);
      await refreshHistory();
    } catch (error) {
      toast.error(failureText(error));
    } finally {
      setSubmitting(false);
    }
  };

  const generate = () => {
    if (!canGenerate) return;
    setSheet(null);
    void submit(buildGenerateRequest({ modelId: selected.model.id, prompt, negative, capabilities: caps, draft, startImage: showStartImage ? startImage : null }));
  };

  const regenerate = (entry: PlaygroundEntry) => {
    if (generating) return;
    const choice = choices.find((candidate) => candidate.model.id === entry.model_id);
    if (!choice) {
      toast.error(t("feed.modelMissing"));
      return;
    }
    void submit(regenerateRequest(entry, choice.model.id));
  };

  const reuseSeed = (entry: PlaygroundEntry) => {
    if (entry.seed === null) return;
    update({ seed: entry.seed });
    toast.success(t("feed.seedReused", { seed: entry.seed }));
  };

  const useAsStart = (asset: PlaygroundAsset) => {
    setStartImage({ assetId: asset.asset_id, url: asset.preview_url ?? asset.url, strength: startImage?.strength ?? defaultStrength });
    toast.success(t("prompt.initImageSet"));
  };

  const upscale = async (entry: PlaygroundEntry, asset: PlaygroundAsset) => {
    if (upscaling || generating) return;
    const watch = new AbortController();
    upscaleWatch.current = watch;
    const { signal } = watch;
    setUpscaling(true);
    try {
      const { job_id } = await api.call("imageUpscale", {
        asset_id: asset.asset_id,
        origin: { type: "playground", entry_id: entry.id },
        client_operation_id: crypto.randomUUID(),
      });
      const job = await watchJob(api, job_id, () => undefined, signal);
      if (job.state === "failed" || job.state === "interrupted") {
        toast.error(t("feed.upscaleFailed"), job.failure?.image?.message ?? failureText(null));
      }
      await refreshHistory();
    } catch (error) {
      if (!signal.aborted) toast.error(t("feed.upscaleFailed"), failureText(error));
    } finally {
      if (!signal.aborted) setUpscaling(false);
    }
  };

  const failed = [models, accounts, capabilities].find((query) => query.isError);
  const promptPane = (
    <PromptPane
      prompt={prompt}
      onPromptChange={setPrompt}
      negative={negative}
      onNegativeChange={setNegative}
      showNegative={showNegative}
      showStartImage={showStartImage}
      startImage={startImage}
      onStartImageChange={setStartImage}
      canGenerate={canGenerate}
      generating={generating}
      onGenerate={generate}
    />
  );
  const settingsPane = (
    <SettingsPane
      choice={selected}
      loading={models.isPending || accounts.isPending}
      failure={failed ? { error: failed.error, retry: () => void failed.refetch() } : null}
      capabilities={caps}
      draft={draft}
      base={base}
      update={update}
      hiresUpscalers={upscalers.data?.hires_upscaler_names ?? []}
      onPickModel={() => setModelPicker(true)}
    />
  );
  const feed = (
    <Feed
      api={api}
      entries={entries}
      loading={history.isPending}
      error={history.isError ? history.error : null}
      onRetry={() => void history.refetch()}
      hasOlder={history.hasNextPage}
      loadingOlder={history.isFetchingNextPage}
      onLoadOlder={() => void history.fetchNextPage()}
      actions={{
        disabled: generating || upscaling,
        onReuseSeed: reuseSeed,
        onRegenerate: regenerate,
        ...((upscalers.data?.models.length ?? 0) > 0 ? { onUpscale: (entry: PlaygroundEntry, asset: PlaygroundAsset) => void upscale(entry, asset) } : {}),
        ...(showStartImage ? { onUseAsStart: useAsStart } : {}),
      }}
    />
  );

  return (
    <div className="@container flex h-full min-h-0 flex-col overflow-clip">
      <AppBar
        title={t("title")}
        backLabel={t("back")}
        onBack={() => void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, replace: true })}
        actions={
          wide ? undefined : (
            <div className="flex items-center gap-1">
              <IconButton label={t("promptTab")} variant="ghost" onClick={() => setSheet("prompt")}>
                <TerminalSquare size={icon.lg} />
              </IconButton>
              <IconButton label={t("settingsTab")} variant="ghost" onClick={() => setSheet("settings")}>
                <SlidersHorizontal size={icon.lg} />
              </IconButton>
            </div>
          )
        }
      />
      {wide ? (
        <Resizable
          defaultSize={300}
          minSize={230}
          maxSize={480}
          storageKey="playground:pane:left"
          panel={<div className="h-full overflow-y-auto p-4">{promptPane}</div>}
          className="min-h-0 flex-1"
        >
          <Resizable
            side="right"
            defaultSize={340}
            minSize={270}
            maxSize={540}
            storageKey="playground:pane:right"
            panel={<div className="h-full overflow-y-auto p-4">{settingsPane}</div>}
            className="h-full min-h-0"
          >
            <div className="@container h-full min-h-0">{feed}</div>
          </Resizable>
        </Resizable>
      ) : (
        <div className="@container min-h-0 flex-1">{feed}</div>
      )}
      <Sheet open={!wide && sheet === "prompt"} onClose={() => setSheet(null)} size="md" title={t("promptTab")}>
        {promptPane}
      </Sheet>
      <Sheet open={!wide && sheet === "settings"} onClose={() => setSheet(null)} size="md" title={t("settingsTab")}>
        {settingsPane}
      </Sheet>
      <ModelChoiceSheet
        api={api}
        open={modelPicker}
        onClose={() => setModelPicker(false)}
        title={t("settings.pickModel")}
        value={selected?.model.id ?? null}
        requirement={imageModels}
        onChange={(id) => {
          if (id) selectModel(id);
        }}
        onAddModel={() => void navigate({ to: "/settings/models/new" })}
      />
    </div>
  );
}
