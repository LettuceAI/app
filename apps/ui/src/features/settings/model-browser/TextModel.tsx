import { useEffect, useRef, useState, type CSSProperties } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Check } from "lucide-react";
import {
  ActionBar,
  Alert,
  Badge,
  Button,
  Chip,
  Field,
  icon,
  Input,
  KeyValue,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  PartitionBar,
  SearchField,
  SelectMenu,
  Sheet,
  Slider,
  Switch,
  Tab,
  TabList,
  Tabs,
  Text,
} from "@lettuceai/crisp";
import type {
  HfBrowserLinks,
  HfKvPlacement,
  HfModelInfo,
  HfModelOffload,
  HfCanonicalPlan,
  HfGpuPolicy,
  HfPlanChoice,
  ProviderAccountView,
} from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { SelectMenuField } from "@/shared/ui/SelectMenuField";
import { useSettingsAction, useSettingsApi } from "../data";
import { useByteSize } from "../image/local-data";
import {
  browserKey,
  repositoryUrl,
  contextValue,
  fitTone,
  hasSprout,
  useBrowserDebounce,
} from "./data";
import { BrowserDownloads } from "./downloads";
import { HfAccessNotice } from "./HfAccessNotice";
import { RepositoryReadme } from "./RepositoryReadme";
import { ExternalButton } from "./ModelBrowserPage";

export function TextModel({
  repo,
  links,
  account,
  phone,
  tab,
  onTab,
  onAuthor,
  onToken,
}: {
  repo: string;
  links: HfBrowserLinks;
  account: ProviderAccountView | undefined;
  phone: boolean;
  tab: "info" | "download";
  onTab: (tab: "info" | "download") => void;
  onAuthor: (author: string) => void;
  onToken: () => void;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const info = useQuery({
    queryKey: [...browserKey, "files", repo],
    queryFn: () => api.call("hfModelFiles", { model_id: repo, mode: "llm" }),
  });
  const readme = useQuery({
    queryKey: [...browserKey, "readme", repo],
    queryFn: () => api.call("hfReadme", { model_id: repo }),
    enabled: tab === "info",
  });
  return (
    <div className="space-y-4">
      {info.isPending ? (
        <LoadingState />
      ) : info.isError ? (
        <>
          <FailureState error={info.error} onRetry={() => void info.refetch()} />
          <HfAccessNotice error={info.error} onToken={onToken} />
          <ExternalButton url={repositoryUrl(links.repository_base, repo)}>
            {t("hf.openHf")}
          </ExternalButton>
        </>
      ) : (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <Button variant="ghost" onClick={() => onAuthor(info.data.author)}>
              {info.data.author}
            </Button>
            <Text variant="hint">
              {t("hf.stats", {
                downloads: info.data.downloads,
                likes: info.data.likes,
              })}
            </Text>
            {info.data.architecture && <Badge>{info.data.architecture}</Badge>}
            {info.data.parameter_count !== null && (
              <Badge>{t("hf.parameters", { count: info.data.parameter_count / 1e9 })}</Badge>
            )}
            {info.data.context_length !== null && (
              <Badge>{info.data.context_length.toLocaleString()}</Badge>
            )}
            {!phone && <Badge>{t("hf.fileCount", { count: info.data.files.length })}</Badge>}
          </div>
          <BrowserDownloads accountId={account?.id} destination={account?.label} repo={repo} />
          <Tabs value={tab} onChange={(value) => onTab(value as typeof tab)}>
            <TabList aria-label={repo}>
              <Tab value="info">{t("hf.info")}</Tab>
              <Tab value="download">{t("hf.download")}</Tab>
            </TabList>
          </Tabs>
          {tab === "info" ? (
            <div className="space-y-4">
              {readme.isPending ? (
                <LoadingState />
              ) : readme.isError ? (
                <FailureState error={readme.error} onRetry={() => void readme.refetch()} />
              ) : readme.data.markdown ? (
                <RepositoryReadme
                  repo={repo}
                  repositoryBase={links.repository_base}
                  revision={info.data.revision ?? undefined}
                  markdown={readme.data.markdown}
                />
              ) : (
                <Text variant="hint">{t("hf.noReadme")}</Text>
              )}
              <div className="flex flex-wrap gap-2">
                <Button variant="primary" onClick={() => onTab("download")}>
                  {t("hf.chooseFile")}
                </Button>
                <ExternalButton url={repositoryUrl(links.repository_base, repo)}>
                  {t("hf.openHf")}
                </ExternalButton>
              </div>
            </div>
          ) : (
            <DownloadPlan info={info.data} account={account} phone={phone} links={links} />
          )}
        </>
      )}
    </div>
  );
}

function DownloadPlan({
  info,
  links,
  account,
  phone,
}: {
  info: HfModelInfo;
  links: HfBrowserLinks;
  account: ProviderAccountView | undefined;
  phone: boolean;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const size = useByteSize();
  const action = useSettingsAction();
  const files = info.files.filter((file) => !file.is_mmproj && !file.is_mtp);
  const projectors = info.files.filter((file) => file.is_mmproj);
  const drafts = info.files.filter((file) => file.is_mtp);
  const [selected, setSelected] = useState("");
  const [context, setContext] = useState<number | null>(null);
  const [gpuPolicy, setGpuPolicy] = useState<HfGpuPolicy | null>(null);
  const [offload, setOffload] = useState<HfModelOffload>("auto");
  const [placement, setPlacement] = useState<HfKvPlacement>("auto");
  const [vision, setVision] = useState(false);
  const [projector, setProjector] = useState(projectors[0]?.filename ?? "");
  const [mtp, setMtp] = useState(false);
  const [draft, setDraft] = useState(drafts[0]?.filename ?? "");
  const [name, setName] = useState(info.model_id.split("/").at(-1) ?? info.model_id);
  const [filter, setFilter] = useState("");
  const [fits, setFits] = useState(false);
  const [report, setReport] = useState<"report" | "compare" | null>(null);
  const supportsFit = hasSprout(account);
  const requestFiles = files.map((file) => ({
    filename: file.filename,
    size: file.size,
  }));
  const base = {
    model_id: info.model_id,
    revision: info.revision,
    files: requestFiles,
    ollama_account_id: account?.id ?? null,
    gpu_policy: gpuPolicy,
  };
  const recommendations = useQuery({
    queryKey: [...browserKey, "recommendation", base],
    queryFn: () =>
      api.call("hfRecommendation", {
        ...base,
        sidecars: null,
        sidecar_reserve_bytes: 0,
        plan: null,
      }),
    enabled: supportsFit && files.length > 0,
  });
  const filename = selected || recommendations.data?.preview?.recommended_filename || "";
  const bundledAvailable =
    (recommendations.data?.preview?.files.find((file) => file.filename === filename)
      ?.nextn_predict_layers ?? 0) > 0;
  const bundledMtp = !account && mtp && bundledAvailable;
  const externalMtp =
    !account && mtp && !bundledAvailable
      ? (drafts.find((file) => file.filename === draft) ?? null)
      : null;
  const mmproj =
    !account && vision ? (projectors.find((file) => file.filename === projector) ?? null) : null;
  const sidecars = account
    ? null
    : {
        mmproj: mmproj ? { filename: mmproj.filename, size: mmproj.size } : null,
        mtp: externalMtp ? { filename: externalMtp.filename, size: externalMtp.size } : null,
        mtp_bundled: bundledMtp,
      };
  const sidecar = (mmproj?.size ?? 0) + (externalMtp?.size ?? 0);
  const effectiveKv = "q8_0";
  const effectiveContext = context ?? 8192;
  const choice: HfPlanChoice | null = filename
    ? {
        filename,
        context_length: effectiveContext,
        kv_type: effectiveKv,
        model_offload: offload,
        kv_placement: placement,
      }
    : null;
  const settledChoice = useBrowserDebounce(choice);
  const request = {
    ...base,
    sidecars,
    sidecar_reserve_bytes: sidecar,
    plan: choice,
  };
  const requestSignature = JSON.stringify(request);
  const settledRequest = {
    ...base,
    sidecars,
    sidecar_reserve_bytes: sidecar,
    plan: settledChoice,
  };
  const settledSignature = JSON.stringify(settledRequest);
  const plan = useQuery({
    queryKey: [...browserKey, "plan", settledRequest],
    queryFn: async () => ({
      signature: settledSignature,
      recommendation: await api.call("hfRecommendation", settledRequest),
    }),
    enabled: supportsFit && settledChoice !== null,
    placeholderData: (previous) => previous,
  });
  const data =
    plan.data?.signature === requestSignature ? plan.data.recommendation : recommendations.data;
  const canonical = data?.preview;
  const currentPlan =
    plan.data?.signature === requestSignature
      ? plan.data.recommendation.preview?.selected
      : undefined;
  const file = files.find((item) => item.filename === filename);
  const fileRec = canonical?.files.find((item) => item.filename === filename);
  const max = fileRec?.max_context ?? info.context_length ?? 0;
  const fullSpeed =
    offload !== "cpu" && placement !== "ram" ? (fileRec?.gpu_optimal_context ?? 0) : 0;
  const scores = {
    data: canonical
      ? {
          scores: canonical.files.flatMap((file) =>
            file.label ? [{ filename: file.filename, label: file.label }] : [],
          ),
        }
      : undefined,
  };
  const requiresPlan = supportsFit && fileRec?.label !== null && fileRec?.label !== undefined;
  const waitingForPreview =
    supportsFit &&
    Boolean(filename) &&
    (recommendations.isPending ||
      plan.isFetching ||
      settledSignature !== requestSignature ||
      plan.isError);
  const visible = files.filter(
    (item) =>
      item.filename.toLowerCase().includes(filter.toLowerCase()) &&
      (!fits ||
        scores.data?.scores.some(
          (score) => score.filename === item.filename && score.label !== "unrunnable",
        )),
  );
  const selectedRef = useRef<HTMLDivElement>(null);
  const filesListRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (files.length > 10 && filesListRef.current && selectedRef.current)
      filesListRef.current.scrollTop =
        selectedRef.current.offsetTop - filesListRef.current.offsetTop;
  }, [filename, files.length]);
  const busyPlan =
    plan.isFetching ||
    settledChoice?.filename !== filename ||
    settledChoice?.context_length !== effectiveContext ||
    settledChoice?.kv_type !== effectiveKv ||
    settledChoice?.model_offload !== offload ||
    settledChoice?.kv_placement !== placement;
  const download = () => {
    if (
      !file ||
      !name.trim() ||
      waitingForPreview ||
      (requiresPlan && (!currentPlan || busyPlan || plan.isError || recommendations.isError))
    )
      return;
    const operationId = crypto.randomUUID();
    action.mutate(() =>
      account
        ? api.call("ollamaPull", {
            provider_account_id: account.id,
            model: `${links.ollama_prefix}${info.model_id}:${file.quantization}`,
            client_operation_id: operationId,
          })
        : api.call("hfDownload", {
            repo: info.model_id,
            revision: info.revision,
            file: file.filename,
            mmproj_file: vision ? projector || null : null,
            mtp_file: externalMtp?.filename ?? null,
            mtp_bundled: bundledMtp,
            setup: {
              create_model: true,
              display_name: name.trim(),
              context_length: currentPlan?.context_length ?? effectiveContext,
              kv_type: effectiveKv,
              offload_kqv: currentPlan?.offload_kqv ?? null,
              gpu_layers: currentPlan?.gpu_layers ?? null,
              model_offload: offload,
              gpu_policy: currentPlan?.gpu_policy ?? gpuPolicy ?? canonical?.gpu_policy ?? null,
            },
            client_operation_id: operationId,
          }),
    );
  };
  const downloadButton = (
    <Button
      variant="primary"
      block
      loading={action.isPending}
      disabled={
        !file ||
        !name.trim() ||
        waitingForPreview ||
        (requiresPlan && (!currentPlan || busyPlan || plan.isError || recommendations.isError))
      }
      onClick={download}
    >
      {account ? t("hf.pull", { name: account.label }) : t("hf.download")}
    </Button>
  );
  const summary = (
    <div className="space-y-4">
      {file && (
        <>
          <Text variant="title">{file.quantization}</Text>
          <Text variant="hint">
            {size(file.size)}
            {effectiveContext !== null ? ` · ${effectiveContext.toLocaleString()}` : ""}
          </Text>
        </>
      )}
      {!file && <Text variant="hint">{t("hf.selectFile")}</Text>}
      {supportsFit && recommendations.isSuccess && !canonical?.recommended_filename && (
        <Text variant="hint">{t("hf.noRecommendation")}</Text>
      )}
      {!supportsFit ? (
        <Text>{t("hf.pickSize")}</Text>
      ) : recommendations.isError && !data ? (
        <FailureState
          error={recommendations.error}
          onRetry={() => void recommendations.refetch()}
        />
      ) : recommendations.isPending || !data ? (
        <LoadingState />
      ) : (
        <>
          {recommendations.isError && (
            <FailureState
              error={recommendations.error}
              onRetry={() => void recommendations.refetch()}
              size="sm"
            />
          )}
          {plan.isError && (
            <FailureState error={plan.error} onRetry={() => void plan.refetch()} size="sm" />
          )}
          {!data.hardware_available ? (
            <Text variant="hint">{t("hf.hardwareMissing")}</Text>
          ) : (
            currentPlan && (
              <>
                <Badge tone={fitTone(currentPlan.label)} dot>
                  {t(`hf.${currentPlan.label}`)}
                </Badge>
                <Text variant="section">
                  {t(
                    currentPlan.gpu_layers === 0
                      ? "hf.cpu"
                      : currentPlan.gpu_layers === currentPlan.total_layers
                        ? "hf.full"
                        : "hf.most_layers",
                  )}
                </Text>
                <Text variant="secondary">{t("hf.modeDescription")}</Text>
                <MemoryPools plan={currentPlan} />
                <KeyValue
                  layout="inline"
                  items={[
                    {
                      label: t("hf.willRun"),
                      value: t(currentPlan.label === "unrunnable" ? "hf.no" : "hf.yes"),
                    },
                    {
                      label: t("hf.headroom"),
                      value: size(
                        Math.max(
                          0,
                          Math.min(
                            ...currentPlan.memory_fit.map(
                              (pool) =>
                                pool.available_bytes -
                                pool.model_bytes -
                                pool.context_bytes -
                                pool.working_bytes,
                            ),
                          ),
                        ),
                      ),
                    },
                    { label: t("hf.prefill"), value: t("hf.notMeasured") },
                    { label: t("hf.generation"), value: t("hf.notMeasured") },
                  ]}
                />
              </>
            )
          )}
          {(!canonical || canonical.files.every((file) => file.label === null)) && (
            <Alert tone="warning" title={t("hf.metadataMissing")} />
          )}
        </>
      )}
      {!phone && (
        <>
          <Field label={t("hf.name")}>
            <Input value={name} onChange={(event) => setName(event.target.value)} />
          </Field>
          {downloadButton}
          <div className="flex flex-wrap gap-2">
            <Button
              variant="ghost"
              size="sm"
              disabled={!currentPlan}
              onClick={() => setReport("report")}
            >
              {t("hf.fullReport")}
            </Button>
            <Button
              variant="ghost"
              size="sm"
              disabled={!scores.data}
              onClick={() => setReport("compare")}
            >
              {t("hf.compare")}
            </Button>
          </div>
        </>
      )}
    </div>
  );
  return (
    <div className="space-y-4">
      {!supportsFit && (
        <Alert
          tone="warning"
          title={t("hf.noRecommendations")}
          description={t("hf.noRecommendationsBody")}
          action={
            <ExternalButton url="https://github.com/LettuceAI/Sprout">
              {t("hf.getSprout")}
            </ExternalButton>
          }
        />
      )}
      {phone && (
        <ListGroup>
          <div className="p-4">{summary}</div>
        </ListGroup>
      )}
      <div className="grid items-start gap-5 @container @4xl:grid-cols-[minmax(0,1fr)_minmax(0,25rem)]">
        <div className="min-w-0 space-y-4">
          {files.length > 10 && (
            <>
              <SearchField
                value={filter}
                onChange={setFilter}
                placeholder={t("hf.filterFiles", { count: files.length })}
                aria-label={t("hf.files")}
              />
              <div className="flex items-center justify-between">
                {supportsFit && (
                  <Chip selected={fits} onClick={() => setFits(!fits)}>
                    {t("hf.fits")}
                  </Chip>
                )}
                <Text variant="hint">
                  {t("hf.showingFiles", {
                    shown: visible.length,
                    total: files.length,
                  })}
                </Text>
              </div>
            </>
          )}
          <div
            className={
              files.length > 10
                ? phone
                  ? "max-h-[22rem] overflow-auto"
                  : "max-h-[26rem] overflow-auto"
                : ""
            }
          >
            <ListGroup label={t("hf.files")}>
              {visible.map((item) => {
                const score = scores.data?.scores.find((value) => value.filename === item.filename);
                return (
                  <div
                    key={item.filename}
                    ref={item.filename === filename ? selectedRef : undefined}
                  >
                    <ListRowButton
                      title={
                        <span className="flex min-w-0 items-center gap-2">
                          <span className="font-mono">{item.quantization}</span>
                          <Text variant="hint">{size(item.size)}</Text>
                          {canonical?.recommended_filename === item.filename && (
                            <Badge>{t("hf.recommended")}</Badge>
                          )}
                        </span>
                      }
                      description={
                        <span className="block truncate font-mono text-xs">{item.filename}</span>
                      }
                      trailing={
                        <span className="flex items-center gap-2">
                          {supportsFit && score && (
                            <Badge tone={fitTone(score.label)}>{t(`hf.${score.label}`)}</Badge>
                          )}
                          {item.filename === filename && <Check size={icon.md} />}
                        </span>
                      }
                      onClick={() => {
                        setSelected(item.filename);
                        setContext(null);
                      }}
                    />
                  </div>
                );
              })}
            </ListGroup>
          </div>
          {visible.length === 0 && <Text variant="hint">{t("hf.noFiles")}</Text>}
          {account && projectors.length > 0 && <Text variant="hint">{t("hf.ollamaSidecars")}</Text>}
          {supportsFit && (
            <div className="grid items-start gap-3 @xl:grid-cols-2">
              <ListGroup label={account ? t("hf.contextOnly") : t("hf.options")}>
                <div className="p-4">
                  {max > 0 && effectiveContext !== null && (
                    <ContextControl
                      value={Math.min(effectiveContext, max)}
                      max={max}
                      limit={fullSpeed}
                      onChange={setContext}
                    />
                  )}
                </div>
                {account && (
                  <ListRow title={t("hf.fitPreview")} description={t("hf.ollamaPreview")} />
                )}
                {!account && projectors.length > 0 && (
                  <>
                    <ListRow
                      title={t("hf.imageSupport")}
                      description={t("hf.imageSupportBody", {
                        size: size(
                          projectors.find((item) => item.filename === projector)?.size ?? 0,
                        ),
                      })}
                      trailing={
                        <Switch
                          checked={vision}
                          onChange={setVision}
                          aria-label={t("hf.imageSupport")}
                        />
                      }
                    />
                    {vision && projectors.length > 1 && (
                      <ListRow
                        title={t("hf.vision")}
                        trailing={
                          <SelectMenuField
                            aria-label={t("hf.vision")}
                            value={projector}
                            onChange={(value) => value && setProjector(value)}
                            options={projectors.map((item) => ({
                              value: item.filename,
                              label: item.filename,
                              meta: size(item.size),
                            }))}
                          />
                        }
                      />
                    )}
                  </>
                )}
                {!account && (drafts.length > 0 || bundledAvailable) && (
                  <>
                    <ListRow
                      title={t("hf.mtp")}
                      description={t("hf.mtpBody")}
                      trailing={<Switch checked={mtp} onChange={setMtp} aria-label={t("hf.mtp")} />}
                    />
                    {mtp && !bundledAvailable && drafts.length > 1 && (
                      <ListRow
                        title={t("hf.mtp")}
                        trailing={
                          <SelectMenuField
                            aria-label={t("hf.mtp")}
                            value={draft}
                            onChange={(value) => value && setDraft(value)}
                            options={drafts.map((item) => ({
                              value: item.filename,
                              label: item.filename,
                              meta: size(item.size),
                            }))}
                          />
                        }
                      />
                    )}
                  </>
                )}
              </ListGroup>
              {!account && (
                <ListGroup label={t("hf.memory")}>
                  <ListRow
                    title={t("hf.kvType")}
                    trailing={
                      <Text className="font-mono">
                        {t("hf.q8Cache", {
                          size: fileRec?.kv_types[0]
                            ? size(fileRec.kv_types[0].cache_bytes)
                            : t("hf.cacheUnavailable"),
                        })}
                      </Text>
                    }
                  />
                  {canonical && canonical.devices.length > 1 && (
                    <>
                      <ListRow
                        title={t("hf.multiGpu")}
                        trailing={
                          <Switch
                            aria-label={t("hf.multiGpu")}
                            checked={(gpuPolicy ?? canonical.gpu_policy).multi_gpu_enabled}
                            onChange={(enabled) => {
                              const policy = gpuPolicy ?? canonical.gpu_policy;
                              setGpuPolicy({
                                ...policy,
                                multi_gpu_enabled: enabled,
                                single_gpu_device_id: null,
                                gpu_device_ids:
                                  enabled && policy.gpu_device_ids.length < 2
                                    ? canonical.devices
                                        .filter((device) => device.device_type !== "IntegratedGpu")
                                        .map((device) => device.index)
                                    : policy.gpu_device_ids,
                              });
                            }}
                          />
                        }
                      />
                      {(gpuPolicy ?? canonical.gpu_policy).multi_gpu_enabled && (
                        <ListRow
                          title={t("hf.gpus")}
                          trailing={
                            <Field label={t("hf.gpus")}>
                              <SelectMenu
                                multiple
                                value={(gpuPolicy ?? canonical.gpu_policy).gpu_device_ids.map(
                                  String,
                                )}
                                options={canonical.devices
                                  .filter((device) => device.device_type !== "IntegratedGpu")
                                  .map((device) => ({
                                    value: String(device.index),
                                    label: device.name,
                                    meta: size(device.memory_free),
                                  }))}
                                onChange={(values) =>
                                  setGpuPolicy({
                                    ...(gpuPolicy ?? canonical.gpu_policy),
                                    gpu_device_ids: values.map(Number),
                                  })
                                }
                              />
                            </Field>
                          }
                        />
                      )}
                    </>
                  )}
                  <ListRow
                    title={t("hf.offload")}
                    trailing={
                      <SelectMenuField
                        aria-label={t("hf.offload")}
                        value={offload}
                        onChange={(value) => value && setOffload(value as HfModelOffload)}
                        options={(["auto", "gpu", "mixed", "cpu"] as const).map((value) => ({
                          value,
                          label: t(`hf.${value}`),
                        }))}
                      />
                    }
                  />
                  <ListRow
                    title={t("hf.kvPlace")}
                    trailing={
                      <SelectMenuField
                        aria-label={t("hf.kvPlace")}
                        value={placement}
                        onChange={(value) => value && setPlacement(value as HfKvPlacement)}
                        options={(["auto", "vram", "ram"] as const).map((value) => ({
                          value,
                          label: t(`hf.${value}`),
                        }))}
                      />
                    }
                  />
                </ListGroup>
              )}
            </div>
          )}
          {phone && !account && (
            <Field label={t("hf.name")}>
              <Input value={name} onChange={(event) => setName(event.target.value)} />
            </Field>
          )}
        </div>
        {!phone && (
          <ListGroup className="sticky top-0">
            <div className="p-4">{summary}</div>
          </ListGroup>
        )}
      </div>
      {action.isError && <FailureState error={action.error} size="sm" />}
      {phone && <ActionBar pinned primary={downloadButton} />}
      <Sheet
        open={report !== null}
        onClose={() => setReport(null)}
        title={report === "compare" ? t("hf.compare") : t("hf.report")}
      >
        <ListGroup>
          {report === "compare"
            ? scores.data?.scores.map((score) => (
                <ListRow
                  key={score.filename}
                  title={score.filename}
                  trailing={<Badge tone={fitTone(score.label)}>{t(`hf.${score.label}`)}</Badge>}
                />
              ))
            : currentPlan && (
                <>
                  <ListRow
                    title={t("hf.willRun")}
                    trailing={t(currentPlan.label === "unrunnable" ? "hf.no" : "hf.yes")}
                  />
                  <ListRow title={t("hf.headroom")} trailing={t("hf.estimate")} />
                  <ListRow
                    title={t("hf.offload")}
                    description={
                      currentPlan.total_layers === 0
                        ? t(
                            currentPlan.gpu_layers === 0
                              ? "hf.cpu"
                              : currentPlan.gpu_layers === currentPlan.total_layers
                                ? "hf.full"
                                : "hf.most_layers",
                          )
                        : t("hf.layerCount", {
                            gpu: currentPlan.gpu_layers,
                            total: currentPlan.total_layers,
                          })
                    }
                  />
                </>
              )}
        </ListGroup>
      </Sheet>
    </div>
  );
}

function ContextControl({
  value,
  max,
  limit,
  onChange,
}: {
  value: number;
  max: number;
  limit: number;
  onChange: (value: number) => void;
}) {
  const { t, i18n } = useTranslation("settings");
  const [draft, setDraft] = useState<string | null>(null);
  const spilling = limit > 0 && value > limit;
  const number = new Intl.NumberFormat(i18n.language);
  const commit = () => {
    if (draft !== null) {
      const parsed = contextValue(draft, max);
      if (parsed !== null) onChange(parsed);
      setDraft(null);
    }
  };
  const showLimit = limit >= Math.min(2048, max) && limit < max;
  return (
    <div className="space-y-2">
      <div className="flex flex-wrap items-center gap-2">
        <Text variant="label">{t("hf.context")}</Text>
        <Text variant="hint" className={spilling ? "text-warning" : "text-accent"}>
          {spilling ? t("hf.spills") : t("hf.fullSpeed")}
        </Text>
        <div className="ml-auto">
          {draft === null ? (
            <Button size="sm" variant="ghost" onClick={() => setDraft(String(value))}>
              {number.format(value)}
            </Button>
          ) : (
            <Input
              autoFocus
              className="w-28"
              value={draft}
              aria-label={t("hf.context")}
              onChange={(event) => setDraft(event.target.value)}
              onBlur={commit}
              onKeyDown={(event) => {
                if (event.key === "Enter") commit();
                if (event.key === "Escape") {
                  event.preventDefault();
                  setDraft(null);
                }
              }}
            />
          )}
        </div>
      </div>
      <div className="relative">
        <Slider
          value={value}
          min={Math.min(2048, max)}
          max={max}
          step={256}
          onChange={onChange}
          aria-label={t("hf.context")}
          style={
            spilling ? ({ "--color-accent": "var(--color-warning)" } as CSSProperties) : undefined
          }
        />
        {showLimit && (
          <span
            className="pointer-events-none absolute bottom-0 top-0 w-px bg-warning"
            style={{
              left: `${((limit - Math.min(2048, max)) / (max - Math.min(2048, max))) * 100}%`,
            }}
          />
        )}
      </div>
      <div className="flex items-center justify-between gap-2">
        <Text variant="hint">{number.format(Math.min(2048, max))}</Text>
        {showLimit && (
          <Button size="sm" variant="ghost" onClick={() => onChange(limit)}>
            {number.format(limit)}
          </Button>
        )}
        <Button size="sm" variant="ghost" onClick={() => onChange(max)}>
          {t("hf.max", { value: number.format(max) })}
        </Button>
      </div>
    </div>
  );
}

function MemoryPools({ plan }: { plan: HfCanonicalPlan }) {
  const { t } = useTranslation("settings");
  const size = useByteSize();
  return (
    <div className="space-y-3">
      {plan.memory_fit.map((pool, index) => {
        const used = pool.model_bytes + pool.context_bytes + pool.working_bytes;
        const free = Math.max(0, pool.available_bytes - used);
        const segments = [
          {
            id: "model",
            label: t("hf.model"),
            value: pool.model_bytes,
            tone: "accent" as const,
          },
          {
            id: "context",
            label: t("hf.context"),
            value: pool.context_bytes,
            tone: "info" as const,
          },
          {
            id: "working",
            label: t("hf.working"),
            value: pool.working_bytes,
            tone: "secondary" as const,
          },
          {
            id: "free",
            label: t("hf.free"),
            value: free,
            tone: "neutral" as const,
          },
        ];
        return (
          <div key={index} className="space-y-2">
            <div className="flex justify-between gap-2">
              <Text variant="label">
                {plan.unified_memory ? t("hf.sharedMemory") : pool.gpu ? t("hf.vram") : t("hf.ram")}
              </Text>
              <Text variant="hint">
                {used > 0
                  ? t("hf.poolUsage", {
                      used: size(used),
                      total: size(pool.available_bytes),
                    })
                  : t("hf.poolUnused", { free: size(free) })}
              </Text>
            </div>
            {pool.devices.length > 0 && <Text variant="hint">{pool.devices.join(" + ")}</Text>}
            <PartitionBar
              aria-label={t("hf.estimate")}
              segments={segments}
              total={pool.available_bytes}
              format={size}
              labels="none"
              size="sm"
            />
            <div className="grid grid-cols-4 gap-1">
              {segments.map((segment) => (
                <div key={segment.id}>
                  <Text variant="hint" className="block">
                    {segment.label}
                  </Text>
                  <Text variant="hint" className="block tabular-nums">
                    {size(segment.value)}
                  </Text>
                </div>
              ))}
            </div>
            {pool.capacity_bytes !== null && (
              <Text variant="hint">
                {t("hf.alreadyUsed", {
                  size: size(Math.max(0, pool.capacity_bytes - pool.available_bytes)),
                })}
              </Text>
            )}
          </div>
        );
      })}
    </div>
  );
}
