import { useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  Button,
  Field,
  fileSize,
  Input,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  Select,
  Sheet,
  Text,
} from "@lettuceai/crisp";
import type { SettingsLlamaDefaultKvCacheType, SettingsView } from "@/api/generated/bindings";
import { appStatusQuery } from "@/api/app-status-query";
import { queryKeys } from "@/api/query-keys";
import { DownloadJob, useDownloads } from "@/entities/download";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsAction, useSettingsApi } from "./data";
import { LoraLibraryView } from "./image/LoraLibraryView";

const searchPageSize = 30;
export function ModelTools({ open, tool, onClose }: { open: boolean; tool: "browse" | "runtime" | "loras"; onClose: () => void }) {
  const { t } = useTranslation("settings");
  return (
    <Sheet open={open} onClose={onClose} title={t(`models.${tool}`)}>
      {tool === "browse" ? <GgufBrowser /> : tool === "runtime" ? <RuntimeDefaults /> : <LoraLibraryView open={open} />}
    </Sheet>
  );
}

function RuntimeDefaults() {
  const api = useSettingsApi();
  const settings = useQuery({ queryKey: queryKeys.settings.detail(), queryFn: () => api.call("settingsGet") });
  if (settings.isPending) return <LoadingState />;
  if (settings.isError) return <FailureState error={settings.error} onRetry={() => void settings.refetch()} />;
  return <RuntimeForm key={settings.data.revision} settings={settings.data} />;
}

function RuntimeForm({ settings }: { settings: SettingsView }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useSettingsAction();
  const [context, setContext] = useState(String(settings.global.llama_default_context_length ?? ""));
  const [kv, setKv] = useState<SettingsLlamaDefaultKvCacheType | "">(settings.global.llama_default_kv_cache_type ?? "");
  const invalid = context !== "" && (!Number.isSafeInteger(Number(context)) || Number(context) < 1);
  const save = () =>
    action.mutate(() =>
      api.call("settingsUpdate", {
        expected_revision: settings.revision,
        patch: {
          section: "local_runtime",
          context_length: context === "" ? null : Number(context),
          kv_cache_type: kv || null,
        },
      }),
    );
  return (
    <div className="space-y-4">
      {action.isError && <FailureState error={action.error} size="sm" />}
      <Field label={t("tools.context")} hint={t("tools.defaultHint")}>
        <Input type="number" min={1} step={1} value={context} onChange={(e) => setContext(e.target.value)} />
      </Field>
      <Field label={t("tools.kv")}>
        <Select
          value={kv}
          options={[
            { value: "", label: t("tools.inherit") },
            ...(["auto", "f16", "q8_0", "q4_0"] as const).map((value) => ({ value, label: value })),
          ]}
          onChange={(e) => setKv(e.target.value as typeof kv)}
        />
      </Field>
      <Text variant="hint">{t("tools.runtimeHint")}</Text>
      <Button loading={action.isPending} disabled={invalid} onClick={save}>
        {t("manage.save")}
      </Button>
    </div>
  );
}

function GgufBrowser() {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useSettingsAction();
  const status = useQuery(appStatusQuery(api));
  const mobile = status.data?.platform === "android" || status.data?.platform === "ios";
  const [input, setInput] = useState("");
  const [search, setSearch] = useState("");
  const [repo, setRepo] = useState("");
  const [jobId, setJobId] = useState<string | null>(null);
  const [mmproj, setMmproj] = useState("");
  const downloads = useDownloads();
  const client = useQueryClient();
  const downloadIds = [
    ...new Set([
      ...(jobId ? [jobId] : []),
      ...(downloads.downloads.filter((job) => job.subject_detail?.type === "model_download" && job.subject_detail.file.toLowerCase().endsWith(".gguf")).map((job) => job.id)),
    ]),
  ];
  const results = useInfiniteQuery({
    queryKey: ["hf", "search", search],
    initialPageParam: 0,
    queryFn: ({ pageParam }) =>
      api.call("hfSearch", {
        query: search,
        mode: "llm",
        limit: searchPageSize,
        sort: "downloads",
        offset: pageParam,
        author: null,
        unfiltered: false,
      }),
    getNextPageParam: (last, pages) => (last.models.length < searchPageSize ? undefined : pages.length * searchPageSize),
  });
  const found = results.data?.pages.flatMap((page) => page.models) ?? [];
  const files = useQuery({
    queryKey: ["hf", "files", repo],
    queryFn: () => api.call("hfModelFiles", { model_id: repo, mode: "llm" }),
    enabled: Boolean(repo),
  });
  const download = (revision: string | null, file: string) =>
    action.mutate(async () => {
      const accepted = await api.call("hfDownload", {
        repo,
        revision,
        file,
        mmproj_file: mmproj || null,
        mtp_file: null,
        mtp_bundled: false,
        setup: {
          display_name: null,
          context_length: null,
          kv_type: null,
          offload_kqv: null,
          gpu_layers: null,
          model_offload: null,
          create_model: true,
        },
        client_operation_id: crypto.randomUUID(),
      });
      setJobId(accepted.job_id);
    });
  return (
    <div className="space-y-4">
      {action.isError && <FailureState error={action.error} size="sm" />}
      {mobile && <Text variant="hint">{t("tools.mobileDownloads")}</Text>}
      {downloads.error !== null && <FailureState error={downloads.error} onRetry={downloads.retry} size="sm" />}
      {downloadIds.map((id) => (
        <DownloadJob key={id} api={api} id={id} onDone={() => client.invalidateQueries({ queryKey: queryKeys.models.all })} />
      ))}
      {!repo ? (
        <>
          <form
            className="flex gap-2"
            onSubmit={(e) => {
              e.preventDefault();
              setSearch(input.trim());
            }}
          >
            <Input
              aria-label={t("tools.search")}
              value={input}
              onChange={(e) => setInput(e.target.value)}
              placeholder={t("tools.search")}
            />
            <Button type="submit">{t("manage.search")}</Button>
          </form>
          {results.isPending ? (
            <LoadingState />
          ) : results.isError && found.length === 0 ? (
            <FailureState error={results.error} onRetry={() => void results.refetch()} />
          ) : (
            <ListGroup>
              {found.map((model) => (
                <ListRowButton
                  key={model.model_id}
                  title={model.model_id}
                  description={t("tools.downloads", { count: model.downloads })}
                  onClick={() => {
                    setRepo(model.model_id);
                    setMmproj("");
                  }}
                />
              ))}
            </ListGroup>
          )}
          {results.isError && found.length > 0 && (
            <FailureState error={results.error} size="sm" onRetry={() => void results.fetchNextPage()} />
          )}
          {results.isSuccess && found.length === 0 && <Text variant="hint">{t("tools.noResults")}</Text>}
          {results.hasNextPage && (
            <Button variant="ghost" loading={results.isFetchingNextPage} onClick={() => void results.fetchNextPage()}>
              {t("tools.loadMore")}
            </Button>
          )}
        </>
      ) : (
        <>
          <Button variant="ghost" onClick={() => setRepo("")}>
            {t("back")}
          </Button>
          <Text>{repo}</Text>
          {files.isPending ? (
            <LoadingState />
          ) : files.isError ? (
            <FailureState error={files.error} onRetry={() => void files.refetch()} />
          ) : (
            <>
              {files.data.files.some((file) => file.is_mmproj) && (
                <Field label={t("tools.mmproj")}>
                  <Select
                    value={mmproj}
                    options={[
                      { value: "", label: t("tools.none") },
                      ...files.data.files
                        .filter((file) => file.is_mmproj)
                        .map((file) => ({ value: file.filename, label: file.filename })),
                    ]}
                    onChange={(e) => setMmproj(e.target.value)}
                  />
                </Field>
              )}
              <ListGroup>
                {files.data.files
                  .filter((file) => !file.is_mmproj && !file.is_mtp)
                  .map((file) => (
                    <ListRow
                      key={file.filename}
                      title={file.filename}
                      description={fileSize(file.size, i18n.language)}
                      trailing={
                        <Button
                          size="sm"
                          disabled={action.isPending || mobile}
                          onClick={() => download(files.data.revision, file.filename)}
                        >
                          {t("manage.download")}
                        </Button>
                      }
                    />
                  ))}
              </ListGroup>
            </>
          )}
        </>
      )}
    </div>
  );
}
