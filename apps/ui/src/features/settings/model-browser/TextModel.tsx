import { useEffect, useRef, useState, type CSSProperties, type ReactNode } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import {
  ArrowDownToLine,
  Check,
  ExternalLink,
  FileText,
  Heart,
  Image as ImageIcon,
  Plus,
  Zap,
} from "lucide-react";
import {
  ActionBar,
  Alert,
  Avatar,
  Badge,
  Button,
  Chip,
  cn,
  Field,
  icon,
  Input,
  KeyValue,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  PartitionBar,
  relativeTime,
  SearchField,
  Segmented,
  Select,
  SelectMenu,
  Sheet,
  Skeleton,
  Slider,
  Switch,
  Tab,
  TabList,
  Table,
  Tabs,
  Text,
} from "@lettuceai/crisp";
import type {
  HfBrowserLinks,
  HfKvPlacement,
  HfModelInfo,
  HfModelOffload,
  HfCanonicalPlan,
  HfCanonicalFile,
  HfCanonicalRecommendation,
  HfGpuPolicy,
  HfPlanChoice,
  ProviderAccountView,
} from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsAction, useSettingsApi } from "../data";
import { useByteSize } from "../image/local-data";
import {
  browserKey,
  fitSettingsKey,
  repositoryUrl,
  contextTokens,
  contextValue,
  fitTone,
  hasSprout,
  placementKey,
  runStatus,
  useBrowserDebounce,
} from "./data";
import { BrowserDownloads } from "./downloads";
import { HfAccessNotice } from "./HfAccessNotice";
import { RepositoryReadme } from "./RepositoryReadme";
import { ExternalButton } from "./ModelBrowserPage";

type ModelFile = HfModelInfo["files"][number];

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
  const { t, i18n } = useTranslation("settings");
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
  const authors = info.data ? [info.data.author] : [];
  const avatars = useQuery({
    queryKey: [...browserKey, "avatars", authors],
    queryFn: () => api.call("hfAvatars", { authors }),
    enabled: authors.length > 0,
  });
  const number = new Intl.NumberFormat(i18n.language, { notation: "compact" });
  return (
    <div className="space-y-4">
      {info.isPending ? (
        <MetadataSkeleton header />
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
          <div className="space-y-3">
            <div className="flex flex-wrap items-center gap-2">
              <Button
                variant="ghost"
                size="sm"
                leading={
                  <Avatar
                    name={info.data.author}
                    src={
                      avatars.data?.avatars.find((avatar) => avatar.author === info.data.author)
                        ?.url ?? null
                    }
                    size="xs"
                  />
                }
                onClick={() => onAuthor(info.data.author)}
              >
                {info.data.author}
              </Button>
              <span className="flex-1" />
              <span className="flex items-center gap-3 whitespace-nowrap text-xs tabular-nums text-fg-3">
                <span className="flex items-center gap-1">
                  <Heart size={icon.xs} />
                  {number.format(info.data.likes)}
                </span>
                <span className="flex items-center gap-1">
                  <ArrowDownToLine size={icon.xs} />
                  {number.format(info.data.downloads)}
                </span>
                {info.data.last_modified && (
                  <span>
                    {t("hf.updated", {
                      when: relativeTime(new Date(info.data.last_modified), undefined, i18n.language),
                    })}
                  </span>
                )}
              </span>
            </div>
            <div className="flex flex-wrap gap-1.5">
              {info.data.architecture && <Badge>{info.data.architecture}</Badge>}
              {info.data.parameter_count !== null && (
                <Badge>
                  {t(phone ? "hf.parameters" : "hf.parametersWord", {
                    ...(phone
                      ? { count: Number((info.data.parameter_count / 1e9).toFixed(1)) }
                      : { value: Number((info.data.parameter_count / 1e9).toFixed(1)) }),
                  })}
                </Badge>
              )}
              {info.data.context_length !== null && (
                <Badge>
                  {t(phone ? "hf.contextShort" : "hf.contextBadge", {
                    value: contextTokens(info.data.context_length),
                  })}
                </Badge>
              )}
              {(info.data.files.some((file) => file.is_mmproj) ||
                info.data.tags.includes("image-text-to-text")) && (
                <Badge tone="info">{t("hf.vision")}</Badge>
              )}
              {!phone && <Badge>{t("hf.fileCount", { count: info.data.files.length })}</Badge>}
            </div>
            <Tabs value={tab} onChange={(value) => onTab(value as typeof tab)}>
              <TabList aria-label={repo}>
                <Tab value="info" icon={<FileText size={icon.sm} />}>
                  {t("hf.info")}
                </Tab>
                <Tab value="download" icon={<ArrowDownToLine size={icon.sm} />}>
                  {t("hf.download")}
                </Tab>
              </TabList>
            </Tabs>
          </div>
          <BrowserDownloads accountId={account?.id} destination={account?.label} repo={repo} />
          {!hasSprout(account) && (
            <Alert
              tone="warning"
              title={t("hf.noRecommendations")}
              description={t("hf.noRecommendationsBody")}
              action={
                <ExternalButton
                  size="sm"
                  icon={<ExternalLink size={icon.sm} />}
                  url="https://github.com/LettuceAI/Sprout"
                >
                  {t("hf.getSprout")}
                </ExternalButton>
              }
            />
          )}
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
                <Button
                  variant="primary"
                  leading={<ArrowDownToLine size={icon.sm} />}
                  onClick={() => onTab("download")}
                >
                  {t("hf.chooseFile")}
                </Button>
                <ExternalButton
                  icon={<ExternalLink size={icon.sm} />}
                  url={repositoryUrl(links.repository_base, repo)}
                >
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
  const queryClient = useQueryClient();
  const size = useByteSize();
  const action = useSettingsAction();
  const files = info.files.filter((file) => !file.is_mmproj && !file.is_mtp);
  const projectors = info.files.filter((file) => file.is_mmproj);
  const drafts = info.files.filter((file) => file.is_mtp);
  const [selected, setSelected] = useState("");
  const [knownFiles, setKnownFiles] = useState<Record<string, HfCanonicalFile>>({});
  const [checkedFits, setCheckedFits] = useState<Record<string, Record<string, HfCanonicalFile>>>(
    {},
  );
  const [context, setContext] = useState<number | null>(null);
  const [gpuPolicy, setGpuPolicy] = useState<HfGpuPolicy | null>(null);
  const [offload, setOffload] = useState<HfModelOffload>("auto");
  const [placement, setPlacement] = useState<HfKvPlacement>("auto");
  const [vision, setVision] = useState(false);
  const [projector, setProjector] = useState(projectors[0]?.filename ?? "");
  const [mtp, setMtp] = useState(false);
  const [kvChoice, setKvChoice] = useState<string | null>(null);
  const [draft, setDraft] = useState(drafts[0]?.filename ?? "");
  const [name, setName] = useState(info.model_id.split("/").at(-1) ?? info.model_id);
  const [filter, setFilter] = useState("");
  const [fits, setFits] = useState(false);
  const [report, setReport] = useState<"report" | "compare" | null>(null);
  const supportsFit = hasSprout(account);
  const requestFiles = files.map((file) => ({
    filename: file.filename,
    size: file.size,
    parts: file.parts ?? [],
  }));
  const base = {
    model_id: info.model_id,
    revision: info.revision,
    files: requestFiles,
    ollama_account_id: account?.id ?? null,
    gpu_policy: gpuPolicy,
  };
  const rememberPreview = (preview: HfCanonicalRecommendation | null, key: string) => {
    if (!preview?.files.length) return;
    const files = Object.fromEntries(preview.files.map((file) => [file.filename, file]));
    for (const file of preview.files)
      queryClient.setQueryData(
        [...browserKey, "capability", info.model_id, info.revision, file.filename],
        file,
      );
    setKnownFiles((previous) => ({ ...previous, ...files }));
    setCheckedFits((previous) => ({ ...previous, [key]: { ...previous[key], ...files } }));
  };
  const initialRequest = { ...base, gpu_policy: null };
  const recommendations = useQuery({
    queryKey: [...browserKey, "recommendation", initialRequest],
    queryFn: async () => {
      const result = await api.call("hfRecommendation", {
        ...initialRequest,
        sidecars: null,
        sidecar_reserve_bytes: 0,
        plan: null,
      });
      rememberPreview(
        result.preview,
        fitSettingsKey({ gpu_policy: null, sidecars: null, plan: null }),
      );
      return result;
    },
    enabled: supportsFit && files.length > 0,
  });
  const filename = selected || recommendations.data?.preview?.recommended_filename || "";
  const fileMetadata =
    knownFiles[filename] ??
    queryClient.getQueryData<HfCanonicalFile>([
      ...browserKey,
      "capability",
      info.model_id,
      info.revision,
      filename,
    ]) ??
    recommendations.data?.preview?.files.find((file) => file.filename === filename);
  const bundledAvailable = (fileMetadata?.nextn_predict_layers ?? 0) > 0;
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
        mmproj: mmproj
          ? { filename: mmproj.filename, size: mmproj.size, parts: mmproj.parts ?? [] }
          : null,
        mtp: externalMtp
          ? {
              filename: externalMtp.filename,
              size: externalMtp.size,
              parts: externalMtp.parts ?? [],
            }
          : null,
        mtp_bundled: bundledMtp,
      };
  const sidecar = (mmproj?.size ?? 0) + (externalMtp?.size ?? 0);
  const effectiveKv = kvChoice ?? recommendations.data?.preview?.recommended_kv_type ?? "q8_0";
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
    queryFn: async () => {
      const result = await api.call("hfRecommendation", settledRequest);
      const fitSignature = fitSettingsKey(settledRequest);
      rememberPreview(result.preview, fitSignature);
      return { signature: settledSignature, fitSignature, recommendation: result };
    },
    enabled: supportsFit && settledChoice !== null,
    placeholderData: (previous) => previous,
  });
  const fitSignature = fitSettingsKey(request);
  const data =
    plan.data?.signature === requestSignature ? plan.data.recommendation : recommendations.data;
  const canonical = data?.preview;
  const currentPlan =
    plan.data?.signature === requestSignature
      ? plan.data.recommendation.preview?.selected
      : undefined;
  const file = files.find((item) => item.filename === filename);
  const upgradeName =
    plan.data?.signature === requestSignature ? plan.data.recommendation.preview?.upgrade : null;
  const upgradeFile =
    upgradeName && upgradeName !== filename
      ? files.find((item) => item.filename === upgradeName)
      : undefined;
  const responseFiles =
    plan.data?.fitSignature === fitSignature
      ? plan.data.recommendation.preview?.files
      : fitSettingsKey({ gpu_policy: null, sidecars: null, plan: null }) === fitSignature
        ? recommendations.data?.preview?.files
        : [];
  const matchingFits = {
    ...checkedFits[fitSignature],
    ...Object.fromEntries((responseFiles ?? []).map((file) => [file.filename, file])),
  };
  const fileRec = matchingFits[filename];
  const max = fileMetadata?.max_context ?? fileRec?.max_context ?? info.context_length ?? 0;
  const fullSpeed =
    offload !== "cpu" && placement !== "ram" ? (fileRec?.gpu_optimal_context ?? 0) : 0;
  const scores = {
    data: canonical
      ? {
          scores: Object.values(matchingFits).flatMap((file) =>
            file.label ? [{ filename: file.filename, label: file.label }] : [],
          ),
        }
      : undefined,
  };
  const requiresPlan = supportsFit && fileRec?.label !== null && fileRec?.label !== undefined;
  const waitingForPreview =
    supportsFit &&
    Boolean(filename) &&
    (plan.isFetching || settledSignature !== requestSignature || plan.isError);
  const initialSuggestion = recommendations.data?.preview?.recommended_filename;
  const suggestedFit = initialSuggestion ? matchingFits[initialSuggestion] : undefined;
  const recommendedFilename =
    suggestedFit?.label &&
    suggestedFit.label !== "unrunnable" &&
    !(initialSuggestion === filename && waitingForPreview)
      ? initialSuggestion
      : null;
  const visible = files.filter(
    (item) =>
      item.filename.toLowerCase().includes(filter.toLowerCase()) &&
      (!fits ||
        scores.data?.scores.some(
          (score) => score.filename === item.filename && score.label !== "unrunnable",
        )),
  );
  const filesListRef = useRef<HTMLDivElement>(null);
  const centered = useRef(false);
  useEffect(() => {
    const list = filesListRef.current;
    const row = list?.querySelector<HTMLElement>('[aria-pressed="true"]');
    if (!list || !row || centered.current || files.length <= 10) return;
    list.scrollTop = row.offsetTop - list.clientHeight / 2 + row.offsetHeight / 2;
    centered.current = true;
  }, [filename, files.length, visible.length]);
  const busyPlan =
    plan.isFetching ||
    settledChoice?.filename !== filename ||
    settledChoice?.context_length !== effectiveContext ||
    settledChoice?.kv_type !== effectiveKv ||
    settledChoice?.model_offload !== offload ||
    settledChoice?.kv_placement !== placement;
  const downloadDisabled =
    !file ||
    !name.trim() ||
    waitingForPreview ||
    currentPlan?.label === "unrunnable" ||
    (requiresPlan && (!currentPlan || busyPlan || plan.isError || recommendations.isError));
  const download = () => {
    if (!file || downloadDisabled) return;
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
  const quantHeader = file && (
    <div className="flex flex-wrap items-baseline gap-2">
      <Text variant="section" className="font-mono">
        {file.quantization}
      </Text>
      <Text variant="hint" className="tabular-nums">
        {t("hf.planSubtitle", {
          size: size(file.size + sidecar),
          tokens: contextTokens(effectiveContext),
        })}
      </Text>
    </div>
  );
  const mode = currentPlan ? placementKey(currentPlan) : null;
  const modeTitle =
    currentPlan?.label === "unrunnable"
      ? t("hf.doesNotFitTitle")
      : mode
        ? t(`hf.mode.${mode}.title`)
        : "";
  const modeBody =
    currentPlan?.label === "unrunnable"
      ? t("hf.doesNotFitBody")
      : mode
        ? t(`hf.mode.${mode}.body`)
        : "";
  const free = currentPlan
    ? currentPlan.memory_fit.reduce(
        (total, pool) =>
          total +
          Math.max(
            0,
            pool.available_bytes - pool.model_bytes - pool.context_bytes - pool.working_bytes,
          ),
        0,
      )
    : 0;
  const stats = currentPlan
    ? [
        [t("hf.willRun"), t(`hf.${runStatus(currentPlan.label)}`)],
        [t("hf.headroom"), free > 0 ? size(free) : t("hf.reportNone")],
        ...(currentPlan.prefill_speed
          ? [[t("hf.prefill"), t(`hf.${currentPlan.prefill_speed}`)]]
          : []),
        ...(currentPlan.generation_speed
          ? [[t("hf.generation"), t(`hf.${currentPlan.generation_speed}`)]]
          : []),
      ]
    : [];
  const verdictBody = supportsFit && (
    <>
      {(waitingForPreview || !data) && !recommendations.isError && (
        <div className="p-4">
          <MetadataSkeleton />
        </div>
      )}
      {data && !waitingForPreview && (
        <>
          {currentPlan && (
            <div className="space-y-4 border-b border-line p-4 last:border-b-0">
              <MemoryPools plan={currentPlan} />
              <div
                className={cn("grid gap-2 pt-1", stats.length > 2 ? "grid-cols-4" : "grid-cols-2")}
              >
                {stats.map(([label, value]) => (
                  <div key={label} className="min-w-0">
                    <Text variant="caption" className="block">
                      {label}
                    </Text>
                    <Text variant="label" className="mt-0.5 block tabular-nums text-fg">
                      {value}
                    </Text>
                  </div>
                ))}
              </div>
            </div>
          )}
          {!data.hardware_available ? (
            <div className="p-4">
              <Text variant="hint">{t("hf.hardwareMissing")}</Text>
            </div>
          ) : (
            (!canonical || canonical.files.every((item) => item.label === null)) && (
              <div className="p-4">
                <Alert tone="warning" title={t("hf.metadataMissing")} />
              </div>
            )
          )}
        </>
      )}
      {recommendations.isError && (
        <div className="p-4">
          <FailureState
            error={recommendations.error}
            onRetry={() => void recommendations.refetch()}
            size="sm"
          />
        </div>
      )}
      {plan.isError && (
        <div className="p-4">
          <FailureState error={plan.error} onRetry={() => void plan.refetch()} size="sm" />
        </div>
      )}
    </>
  );
  const verdict =
    currentPlan && mode && !waitingForPreview ? (
      phone ? (
        <div className="space-y-1 border-b border-line p-4">
          <div className="flex items-center gap-3">
            <Text variant="title" className="min-w-0 flex-1 text-base">
              {modeTitle}
            </Text>
            <Badge tone={fitTone(currentPlan.label)} dot>
              {t(`hf.${currentPlan.label}`)}
            </Badge>
          </div>
          <Text variant="hint" className="block text-xs leading-snug">
            {modeBody}
          </Text>
        </div>
      ) : (
        <div className="space-y-2 border-b border-line p-4">
          {quantHeader}
          <div className="flex flex-wrap items-center gap-2">
            <Badge tone={fitTone(currentPlan.label)} dot>
              {t(`hf.${currentPlan.label}`)}
            </Badge>
            <Text variant="label">{modeTitle}</Text>
          </div>
          <Text variant="hint" className="block text-xs leading-snug">
            {modeBody}
          </Text>
        </div>
      )
    ) : phone ? null : (
      <div className="space-y-2 border-b border-line p-4">
        {quantHeader}
        {!file && !recommendations.isPending && <Text variant="hint">{t("hf.selectFile")}</Text>}
        {!supportsFit && (
          <Text variant="hint">{t("hf.pickSize", { name: account?.label ?? "" })}</Text>
        )}
        {supportsFit &&
          recommendations.isSuccess &&
          !recommendations.data.preview?.recommended_filename &&
          !selected && <Text variant="hint">{t("hf.noRecommendation")}</Text>}
      </div>
    );
  const reportButton = (
    <Button
      variant="ghost"
      size="sm"
      {...(phone ? { block: true } : {})}
      disabled={!currentPlan}
      onClick={() => setReport("report")}
    >
      {t("hf.fullReport")}
    </Button>
  );
  const nameField = !account && (
    <Field label={t("hf.nameInModels")}>
      <Input value={name} onChange={(event) => setName(event.target.value)} />
    </Field>
  );
  const primaryLabel = account
    ? t("hf.pull", { name: account.label })
    : phone || !file
      ? t("hf.download")
      : t("hf.downloadQuant", { quant: file.quantization });
  const downloadButton = (
    <Button
      variant="primary"
      {...(phone ? {} : { block: true, size: "lg" as const })}
      loading={action.isPending}
      leading={<ArrowDownToLine size={icon.md} />}
      disabled={downloadDisabled}
      onClick={download}
    >
      {primaryLabel}
    </Button>
  );
  const summary = (
    <div className="divide-y divide-line">
      {verdict}
      {verdictBody}
      {phone ? (
        <div className="p-2">{reportButton}</div>
      ) : (
        <div className="space-y-3 p-4">
          {nameField}
          {downloadButton}
          {supportsFit && (
            <div className="flex justify-center gap-1">
              {reportButton}
              <Button
                variant="ghost"
                size="sm"
                disabled={!currentPlan}
                onClick={() => setReport("compare")}
              >
                {t("hf.compare")}
              </Button>
            </div>
          )}
        </div>
      )}
    </div>
  );
  const filesLabel = (
    <span className="flex items-baseline justify-between gap-2">
      <span>
        {supportsFit
          ? t(phone ? "hf.filesFitAtShort" : "hf.filesFitAt", {
              tokens: contextTokens(effectiveContext),
            })
          : t("hf.files")}
      </span>
      {files.length > 10 && (
        <span className="normal-case tabular-nums">
          {t("hf.showingFiles", { shown: visible.length, total: files.length })}
        </span>
      )}
    </span>
  );
  const filesList = (
    <ListGroup label={filesLabel}>
      {files.length > 10 && (
        <div className="flex items-center gap-2 p-2">
          <div className="min-w-0 flex-1">
            <SearchField
              size="sm"
              value={filter}
              onChange={setFilter}
              placeholder={t("hf.filterFiles", { count: files.length })}
              aria-label={t("hf.files")}
            />
          </div>
          {supportsFit && (
            <Chip selected={fits} onClick={() => setFits(!fits)}>
              {t("hf.fits")}
            </Chip>
          )}
        </div>
      )}
      <div
        ref={filesListRef}
        className={cn(
          "relative divide-y divide-line",
          files.length > 10 &&
            (phone
              ? "max-h-[22rem] overflow-y-auto overscroll-contain"
              : "max-h-[26rem] overflow-y-auto overscroll-contain"),
        )}
      >
        {visible.map((item) => {
          const score = scores.data?.scores.find((value) => value.filename === item.filename);
          const on = item.filename === filename;
          const suggested = recommendedFilename === item.filename;
          const sizeText = item.size > 0 ? size(item.size) : t("hf.sizeUnavailable");
          return (
            <ListRowButton
              key={item.filename}
              chevron={false}
              aria-pressed={on}
              className={cn(phone ? "" : "min-h-10 py-2", on && "bg-accent/10")}
              title={
                phone ? (
                  <span className="flex items-center gap-2">
                    <span className={cn("font-mono text-sm", on && "text-accent")}>
                      {item.quantization}
                    </span>
                    {suggested && <Badge tone="accent">{t("hf.recommended")}</Badge>}
                  </span>
                ) : (
                  <span className="grid min-w-0 grid-cols-[7rem_4rem_minmax(0,1fr)] items-center gap-4">
                    <span className={cn("font-mono text-sm", on && "text-accent")}>
                      {item.quantization}
                    </span>
                    <span className="text-right text-sm font-normal tabular-nums text-fg-2">
                      {sizeText}
                    </span>
                    <span className="truncate text-xs font-normal text-fg-3">
                      {item.display_name ?? item.filename}
                    </span>
                  </span>
                )
              }
              description={
                phone ? (
                  <span className="block tabular-nums">
                    {sizeText}
                    {(item.parts?.length ?? 0) > 0 &&
                      ` · ${t("hf.partCount", { count: item.parts?.length ?? 0 })}`}
                  </span>
                ) : undefined
              }
              trailing={
                <span className="flex items-center gap-2">
                  {!phone && (item.parts?.length ?? 0) > 0 && (
                    <Text variant="hint">
                      {t("hf.partCount", { count: item.parts?.length ?? 0 })}
                    </Text>
                  )}
                  {!phone && suggested && <Badge tone="accent">{t("hf.recommended")}</Badge>}
                  {supportsFit && score && (
                    <Badge tone={fitTone(score.label)} dot>
                      {t(`hf.${score.label}`)}
                    </Badge>
                  )}
                  <span className="flex w-4 shrink-0">
                    {on && <Check size={icon.md} className="text-accent" />}
                  </span>
                </span>
              }
              onClick={() => {
                setSelected(item.filename);
                setContext(null);
              }}
            />
          );
        })}
        {visible.length === 0 && (
          <div className="px-4 py-6 text-center">
            <Text variant="hint">{t("hf.noFiles")}</Text>
          </div>
        )}
      </div>
    </ListGroup>
  );
  const projectorSelect = vision && projectors.length > 1 && (
    <div className="px-4 py-3">
      <Field label={t("hf.visionFile")} hint={t("hf.visionFileHint", { count: projectors.length })}>
        <SelectMenu
          value={projector}
          onChange={(value) => value && setProjector(value)}
          options={projectors.map((item) => ({
            value: item.filename,
            label: item.filename,
            meta: <span className="tabular-nums text-fg-3">{size(item.size)}</span>,
          }))}
        />
      </Field>
    </div>
  );
  const draftSelect = mtp && !bundledAvailable && drafts.length > 1 && (
    <div className="px-4 py-3">
      <Field label={t("hf.mtp")}>
        <SelectMenu
          value={draft}
          onChange={(value) => value && setDraft(value)}
          options={drafts.map((item) => ({
            value: item.filename,
            label: item.filename,
            meta: <span className="tabular-nums text-fg-3">{size(item.size)}</span>,
          }))}
        />
      </Field>
    </div>
  );
  const contextCard = supportsFit && (max > 0 || !account) && (
    <ListGroup label={account ? t("hf.contextOnly") : t("hf.options")}>
      {max > 0 && (
        <ContextControl
          value={Math.min(effectiveContext, max)}
          max={max}
          limit={fullSpeed}
          pending={waitingForPreview}
          onChange={setContext}
        />
      )}
      {!account && projectors.length > 0 && (
        <>
          <ListRow
            icon={<ImageIcon size={icon.lg} />}
            title={t("hf.imageSupport")}
            description={t(projectors.length > 1 ? "hf.imageSupportMany" : "hf.imageSupportBody", {
              count: projectors.length,
              size: size(projectors.find((item) => item.filename === projector)?.size ?? 0),
            })}
            trailing={
              <Switch checked={vision} onChange={setVision} aria-label={t("hf.imageSupport")} />
            }
          />
          {projectorSelect}
        </>
      )}
      {!account && (drafts.length > 0 || bundledAvailable) && (
        <>
          <ListRow
            icon={<Zap size={icon.lg} />}
            title={t("hf.mtp")}
            description={t(bundledAvailable ? "hf.mtpBundledBody" : "hf.mtpBody")}
            trailing={<Switch checked={mtp} onChange={setMtp} aria-label={t("hf.mtp")} />}
          />
          {draftSelect}
        </>
      )}
    </ListGroup>
  );
  const kvOptions = [
    ...(fileRec?.kv_types ?? []).map((item) => ({
      value: item.kv_type,
      label: item.kv_type.toUpperCase(),
      meta: <span className="tabular-nums text-fg-3">{size(item.cache_bytes)}</span>,
    })),
  ];
  if (!kvOptions.some((item) => item.value === effectiveKv))
    kvOptions.unshift({
      value: effectiveKv,
      label: effectiveKv.toUpperCase(),
      meta: <span className="tabular-nums text-fg-3">{t("hf.cacheUnavailable")}</span>,
    });
  const memoryCard = supportsFit && !account && (
    <ListGroup label={t("hf.memory")}>
      <div className="space-y-4 px-4 py-3">
        <Field label={t("hf.kvType")} hint={t(`hf.kvHint.${effectiveKv}`, { defaultValue: "" })}>
          <SelectMenu
            aria-label={t("hf.kvType")}
            value={effectiveKv}
            onChange={(value) => value && setKvChoice(value)}
            options={kvOptions}
          />
        </Field>
        <Field label={t("hf.offload")}>
          <Segmented
            block
            size="sm"
            value={offload}
            onChange={setOffload}
            aria-label={t("hf.offload")}
            options={(["auto", "gpu", "mixed", "cpu"] as const).map((value) => ({
              value,
              label: t(`hf.offloadChoices.${value}`),
            }))}
          />
        </Field>
        <Field label={t("hf.kvPlace")}>
          <Segmented
            block
            size="sm"
            value={placement}
            onChange={setPlacement}
            aria-label={t("hf.kvPlace")}
            options={(["auto", "vram", "ram"] as const).map((value) => ({
              value,
              label: t(`hf.placementChoices.${value}`),
            }))}
          />
        </Field>
      </div>
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
            <div className="px-4 py-3">
              <Field label={t("hf.gpus")}>
                <SelectMenu
                  multiple
                  value={(gpuPolicy ?? canonical.gpu_policy).gpu_device_ids.map(String)}
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
            </div>
          )}
        </>
      )}
    </ListGroup>
  );
  const upgradeNotice = upgradeFile && currentPlan && !waitingForPreview && (
    <Alert
      tone="success"
      title={t("hf.upgradeTitle", { quant: upgradeFile.quantization })}
      description={t("hf.upgradeBody", { size: size(upgradeFile.size) })}
      action={
        <Button
          size="sm"
          variant="secondary"
          onClick={() => {
            setSelected(upgradeFile.filename);
            setContext(null);
          }}
        >
          {t("hf.upgradeUse", { quant: upgradeFile.quantization })}
        </Button>
      }
    />
  );
  const optionCards =
    supportsFit && recommendations.isPending && !filename ? (
      <div className="grid grid-cols-1 gap-3 @xl:grid-cols-2">
        <OptionsSkeleton />
        <OptionsSkeleton memory />
      </div>
    ) : (
      <div className={cn("grid items-start gap-3", memoryCard && "@xl:grid-cols-2")}>
        {contextCard}
        {memoryCard}
      </div>
    );
  return (
    <div className="space-y-4">
      {phone && upgradeNotice}
      {phone && <ListGroup>{summary}</ListGroup>}
      <div className="grid items-start gap-5 @container @4xl:grid-cols-[minmax(0,1fr)_minmax(0,25rem)]">
        <div className="min-w-0 space-y-4 @container">
          {!phone && upgradeNotice}
          {filesList}
          {recommendations.data?.preview?.recommended_filename && (
            <Text variant="hint" className="px-1">
              {t("hf.recommendationScope")}
            </Text>
          )}
          {account && projectors.length > 0 && (
            <Text variant="hint" className="px-1">
              {t("hf.ollamaSidecars")}
            </Text>
          )}
          {optionCards}
          {phone && nameField}
        </div>
        {!phone && <ListGroup className="sticky top-0">{summary}</ListGroup>}
      </div>
      {action.isError && <FailureState error={action.error} size="sm" />}
      {phone && (
        <ActionBar
          pinned
          leading={
            file && (
              <span className="min-w-0">
                <span className="block font-mono text-sm text-fg">{file.quantization}</span>
                <span className="block text-xs text-fg-3">
                  {t("hf.planSubtitle", {
                    size: size(file.size + sidecar),
                    tokens: contextTokens(effectiveContext),
                  })}
                </span>
              </span>
            )
          }
          primary={downloadButton}
        />
      )}
      <ReportSheet
        report={report}
        onClose={() => setReport(null)}
        info={info}
        file={file}
        plan={currentPlan}
        kvType={effectiveKv}
        context={effectiveContext}
        fits={matchingFits}
        files={files}
        selectedName={filename}
        fullSpeed={fullSpeed}
      />
    </div>
  );
}

function ContextControl({
  value,
  max,
  limit,
  onChange,
  pending,
}: {
  pending?: boolean;
  value: number;
  max: number;
  limit: number;
  onChange: (value: number) => void;
}) {
  const { t, i18n } = useTranslation("settings");
  const [draft, setDraft] = useState<string | null>(null);
  const number = new Intl.NumberFormat(i18n.language);
  const min = Math.min(2048, max);
  const showLimit = limit >= min && limit < max;
  const spilling = limit > 0 && value > limit;
  const at = (point: number) =>
    `calc(9px + (100% - 18px) * ${max > min ? (point - min) / (max - min) : 0})`;
  const commit = () => {
    if (draft === null) return;
    const parsed = contextValue(draft, max);
    setDraft(null);
    if (parsed !== null) onChange(parsed);
  };
  return (
    <div className="px-4 py-3">
      <div className="flex items-baseline gap-2">
        <Text variant="label" className="flex-1">
          {t("hf.contextLength")}
        </Text>
        {pending ? (
          <Skeleton className="h-3 w-24" />
        ) : (
          limit > 0 && (
            <span className={cn("text-xs", spilling ? "text-warning" : "text-accent")}>
              {spilling ? t("hf.spills") : t("hf.fullSpeed")}
            </span>
          )
        )}
        {draft === null ? (
          <button
            type="button"
            title={t("hf.contextExact")}
            onClick={() => setDraft(String(value))}
            className="-mr-1 rounded-md px-1 text-right font-mono text-sm tabular-nums text-fg underline decoration-dotted decoration-fg-4 underline-offset-4 hover:bg-fill hover:decoration-fg-3"
          >
            {number.format(value)}
          </button>
        ) : (
          <input
            autoFocus
            aria-label={t("hf.contextInput")}
            value={draft}
            onChange={(event) => setDraft(event.target.value)}
            onFocus={(event) => event.target.select()}
            onBlur={commit}
            onKeyDown={(event) => {
              if (event.key === "Enter") commit();
              if (event.key === "Escape") {
                event.preventDefault();
                setDraft(null);
              }
            }}
            className="-mr-1 h-6 w-24 rounded-md border border-accent bg-fill-2 px-1.5 text-right font-mono text-sm tabular-nums text-fg focus:outline-none focus:ring-1 focus:ring-accent/35"
          />
        )}
      </div>
      <div className="relative">
        <Slider
          value={value}
          min={min}
          max={max}
          step={256}
          onChange={onChange}
          aria-label={t("hf.contextLength")}
          style={
            spilling ? ({ "--color-accent": "var(--color-warning)" } as CSSProperties) : undefined
          }
        />
        {showLimit && (
          <span
            className="pointer-events-none absolute top-1/2 h-3 w-0.5 -translate-x-1/2 -translate-y-1/2 rounded-full bg-fg-3"
            style={{ left: at(limit) }}
          />
        )}
      </div>
      <div className="relative h-4 text-2xs tabular-nums text-fg-3">
        <span className="absolute left-0">{number.format(min)}</span>
        {showLimit && (
          <button
            type="button"
            className="absolute flex -translate-x-1/2 items-center gap-0.5 hover:text-accent"
            style={{ left: at(limit) }}
            onClick={() => onChange(limit)}
          >
            <Zap size={icon.xs} />
            {number.format(limit)}
          </button>
        )}
        <button
          type="button"
          className="absolute right-0 hover:text-accent"
          onClick={() => onChange(max)}
        >
          {t("hf.max", { value: number.format(max) })}
        </button>
      </div>
    </div>
  );
}

function MetadataSkeleton({ header = false }: { header?: boolean }) {
  const { t } = useTranslation("settings");
  const memory = (
    <div className="space-y-4">
      {[0, 1].map((pool) => (
        <div key={pool} className="space-y-2">
          <div className="flex justify-between gap-3">
            <Skeleton className="h-4 w-28" />
            <Skeleton className="h-3 w-24" />
          </div>
          <Skeleton shape="block" className="h-2 w-full" />
          <div className="grid grid-cols-4 gap-2">
            {[0, 1, 2, 3].map((part) => (
              <div key={part} className="space-y-1">
                <Skeleton className="h-2 w-10 max-w-full" />
                <Skeleton className="h-4 w-14 max-w-full" />
              </div>
            ))}
          </div>
        </div>
      ))}
      <div className="grid grid-cols-4 gap-2 pt-1">
        {[0, 1, 2, 3].map((stat) => (
          <div key={stat} className="space-y-1">
            <Skeleton className="h-2 w-12 max-w-full" />
            <Skeleton className="h-4 w-16 max-w-full" />
          </div>
        ))}
      </div>
    </div>
  );
  return (
    <div role="status" aria-busy="true" aria-label={t("hf.checkingMetadata")} className="space-y-4">
      <Text variant="secondary">{t(header ? "hf.loadingFiles" : "hf.checkingMetadata")}</Text>
      {header ? (
        <>
          <div className="flex gap-3">
            <Skeleton className="h-7 w-24" />
            <Skeleton className="h-7 w-28" />
            <Skeleton className="h-7 w-20" />
          </div>
          <Skeleton className="h-8 w-48" />
          <div className="grid items-start gap-5 @4xl:grid-cols-[minmax(0,1fr)_minmax(0,25rem)]">
            <div className="space-y-4">
              <ListGroup label={t("hf.files")}>
                <div className="flex items-center gap-3 p-3">
                  <Skeleton shape="block" className="h-8 flex-1" />
                  <Skeleton shape="block" className="h-7 w-12" />
                </div>
                {[0, 1, 2, 3, 4, 5, 6, 7].map((row) => (
                  <ListRow
                    key={row}
                    className="py-2"
                    title={
                      <span className="grid grid-cols-[7rem_4rem_minmax(0,1fr)] items-center gap-4">
                        <Skeleton className="h-4 w-20" />
                        <Skeleton className="h-3 w-12" />
                        <Skeleton className="h-3 w-full" />
                      </span>
                    }
                    trailing={<Skeleton className="h-5 w-16" />}
                  />
                ))}
              </ListGroup>
              <div className="grid grid-cols-1 gap-3 @xl:grid-cols-2">
                <OptionsSkeleton />
                <OptionsSkeleton memory />
              </div>
            </div>
            <ListGroup>
              <div className="divide-y divide-line">
                <div className="space-y-2 p-4">
                  <div className="flex items-baseline gap-3">
                    <Skeleton className="h-6 w-24" />
                    <Skeleton className="h-3 w-32" />
                  </div>
                  <Skeleton className="h-5 w-40" />
                  <Skeleton className="h-3 w-full" />
                </div>
                <div className="p-4">{memory}</div>
                <div className="space-y-3 p-4">
                  <Skeleton className="h-3 w-28" />
                  <Skeleton shape="block" className="h-9 w-full" />
                  <Skeleton shape="block" className="h-10 w-full" />
                </div>
              </div>
            </ListGroup>
          </div>
        </>
      ) : (
        memory
      )}
    </div>
  );
}

function OptionsSkeleton({ memory = false }: { memory?: boolean }) {
  const { t } = useTranslation("settings");
  return (
    <ListGroup label={t(memory ? "hf.memory" : "hf.options")}>
      {[0, 1, 2].map((row) => (
        <ListRow
          key={row}
          title={<Skeleton className="h-4 w-24" />}
          trailing={<Skeleton shape="block" className="h-7 w-24" />}
        />
      ))}
    </ListGroup>
  );
}

function MemoryPools({ plan }: { plan: HfCanonicalPlan }) {
  const { t } = useTranslation("settings");
  const size = useByteSize();
  return (
    <div className="space-y-4">
      {[...plan.memory_fit]
        .sort((a, b) => Number(b.gpu) - Number(a.gpu))
        .map((pool, index) => {
          const used = pool.model_bytes + pool.context_bytes + pool.working_bytes;
          const free = Math.max(0, pool.available_bytes - used);
          const parts = [
            {
              id: "model",
              label: t("hf.model"),
              value: pool.model_bytes,
              tone: "accent" as const,
              dot: "bg-accent",
            },
            {
              id: "context",
              label: t("hf.context"),
              value: pool.context_bytes,
              tone: "info" as const,
              dot: "bg-info",
            },
            {
              id: "working",
              label: t("hf.working"),
              value: pool.working_bytes,
              tone: "neutral" as const,
              dot: "bg-fg-3",
            },
            {
              id: "free",
              label: t("hf.free"),
              value: free,
              tone: "neutral" as const,
              dot: "bg-fill-3",
            },
          ];
          return (
            <div key={index} className="space-y-2">
              <div className="flex items-baseline justify-between gap-3">
                <Text variant="label" className="min-w-0 truncate text-fg">
                  {plan.unified_memory
                    ? t("hf.sharedMemory")
                    : pool.gpu
                      ? `${pool.devices.join(" + ") || t("hf.gpu")} ${t("hf.vramShort")}`
                      : t("hf.systemRam")}
                </Text>
                <Text variant="caption" className="shrink-0 text-xs tabular-nums">
                  {used > 0 ? (
                    <>
                      <span className="font-medium text-fg">{size(used)}</span>{" "}
                      {t("hf.poolOf", { total: size(pool.available_bytes) })}
                    </>
                  ) : (
                    t("hf.poolUnused", { free: size(free) })
                  )}
                </Text>
              </div>
              <PartitionBar
                aria-label={t("hf.estimate")}
                segments={parts
                  .slice(0, 3)
                  .filter((part) => part.value > 0)
                  .map(({ id, label, value, tone }) => ({ id, label, value, tone }))}
                total={pool.available_bytes}
                showRemainder
                format={size}
                labels="none"
                size="sm"
              />
              {used > 0 && (
                <div className="grid grid-cols-4 gap-2">
                  {parts.map((part) => (
                    <div
                      key={part.id}
                      className={cn(
                        "min-w-0",
                        part.value === 0 && part.id !== "free" && "opacity-40",
                      )}
                    >
                      <Text variant="caption" className="flex items-center gap-1.5">
                        <span className={cn("h-2 w-2 shrink-0 rounded-full", part.dot)} />
                        {part.label}
                      </Text>
                      <Text variant="label" className="mt-0.5 block tabular-nums text-fg">
                        {part.value > 0 ? size(part.value) : "0"}
                      </Text>
                    </div>
                  ))}
                </div>
              )}
              {pool.capacity_bytes !== null && pool.capacity_bytes > pool.available_bytes && (
                <Text variant="caption" className="block">
                  {t("hf.alreadyUsed", { size: size(pool.capacity_bytes - pool.available_bytes) })}
                </Text>
              )}
            </div>
          );
        })}
    </div>
  );
}

function ReportSheet({
  report,
  onClose,
  info,
  file,
  plan,
  kvType,
  context,
  fits,
  files,
  selectedName,
  fullSpeed,
}: {
  report: "report" | "compare" | null;
  onClose: () => void;
  info: HfModelInfo;
  file: ModelFile | undefined;
  plan: HfCanonicalPlan | null | undefined;
  kvType: string;
  context: number;
  fits: Record<string, HfCanonicalFile>;
  files: ModelFile[];
  selectedName: string;
  fullSpeed: number;
}) {
  const { t } = useTranslation("settings");
  const size = useByteSize();
  const section = (label: string, items: { label: ReactNode; value: ReactNode }[]) => (
    <section className="space-y-2">
      <Text variant="label" className="block px-1">
        {label}
      </Text>
      <div className="rounded-xl border border-line px-4 py-3">
        <KeyValue items={items} />
      </div>
    </section>
  );
  const sum = (key: "model_bytes" | "context_bytes" | "working_bytes") =>
    plan?.memory_fit.reduce((total, pool) => total + pool[key], 0) ?? 0;
  const free =
    plan?.memory_fit.reduce(
      (total, pool) =>
        total +
        Math.max(
          0,
          pool.available_bytes - pool.model_bytes - pool.context_bytes - pool.working_bytes,
        ),
      0,
    ) ?? 0;
  const tokenText = (value: number) => t("hf.reportTokens", { count: contextTokens(value) });
  return (
    <Sheet
      open={report !== null}
      onClose={onClose}
      placement="auto"
      size={report === "compare" ? "lg" : "md"}
      title={report === "compare" ? t("hf.compare") : t("hf.reportTitle")}
      description={
        report === "compare"
          ? t("hf.compareSubtitle", { tokens: contextTokens(context) })
          : file
            ? t("hf.reportSubtitle", {
                quant: file.quantization,
                tokens: contextTokens(context),
              })
            : undefined
      }
    >
      {report === "compare" ? (
        <CompareTable files={files} fits={fits} start={selectedName} />
      ) : (
        plan && (
          <div className="space-y-5 pb-2">
            {section(
              t("hf.reportHardware"),
              plan.memory_fit.flatMap((pool) => [
                {
                  label: t("hf.reportFree", {
                    pool: pool.gpu
                      ? `${pool.devices.join(" + ") || t("hf.gpu")} ${t("hf.vramShort")}`
                      : t("hf.systemRam"),
                  }),
                  value: size(pool.available_bytes),
                },
              ]),
            )}
            {section(t("hf.reportModel"), [
              ...(info.architecture
                ? [{ label: t("hf.reportArchitecture"), value: info.architecture }]
                : []),
              ...(plan.total_layers > 0
                ? [{ label: t("hf.reportLayersTotal"), value: String(plan.total_layers) }]
                : []),
              ...(info.context_length !== null
                ? [{ label: t("hf.reportTrained"), value: tokenText(info.context_length) }]
                : []),
            ])}
            {section(t("hf.reportMemory"), [
              { label: t("hf.reportFile"), value: size(sum("model_bytes")) },
              {
                label: t("hf.reportCache", { type: kvType.toUpperCase() }),
                value: size(sum("context_bytes")),
              },
              { label: t("hf.reportWorking"), value: size(sum("working_bytes")) },
              {
                label: t("hf.reportTotal"),
                value: size(sum("model_bytes") + sum("context_bytes") + sum("working_bytes")),
              },
              { label: t("hf.reportRoom"), value: free > 0 ? size(free) : t("hf.reportNone") },
            ])}
            {section(t("hf.reportGpu"), [
              ...(plan.total_layers > 0
                ? [
                    {
                      label: t("hf.reportLayers"),
                      value: t("hf.reportLayersValue", {
                        gpu: plan.gpu_layers,
                        total: plan.total_layers,
                      }),
                    },
                  ]
                : []),
              {
                label: t("hf.reportFullSpeed"),
                value: fullSpeed > 0 ? tokenText(fullSpeed) : t("hf.reportNone"),
              },
            ])}
            <Text variant="hint" className="block px-1">
              {t("hf.reportHint")}
            </Text>
          </div>
        )
      )}
    </Sheet>
  );
}

function CompareTable({
  files,
  fits,
  start,
}: {
  files: ModelFile[];
  fits: Record<string, HfCanonicalFile>;
  start: string;
}) {
  const { t } = useTranslation("settings");
  const size = useByteSize();
  const [picked, setPicked] = useState<string[]>(() => {
    const index = Math.max(
      0,
      files.findIndex((item) => item.filename === start),
    );
    return [start, files[Math.min(index + 1, files.length - 1)]?.filename ?? start];
  });
  const rows: { key: string; label: string; get: (item: ModelFile) => ReactNode }[] = [
    { key: "size", label: t("hf.compareSize"), get: (item) => size(item.size) },
    {
      key: "fit",
      label: t("hf.compareFit"),
      get: (item) => {
        const label = fits[item.filename]?.label;
        return label ? (
          <Badge tone={fitTone(label)} dot>
            {t(`hf.${label}`)}
          </Badge>
        ) : (
          t("hf.compareUnchecked")
        );
      },
    },
    {
      key: "speed",
      label: t("hf.compareFullSpeed"),
      get: (item) => {
        const value = fits[item.filename]?.gpu_optimal_context;
        return value === undefined || value === null
          ? t("hf.compareUnchecked")
          : value > 0
            ? contextTokens(value)
            : t("hf.reportNone");
      },
    },
    {
      key: "most",
      label: t("hf.compareMost"),
      get: (item) => {
        const value = fits[item.filename]?.recommended_context;
        return value === undefined || value === null
          ? t("hf.compareUnchecked")
          : contextTokens(value);
      },
    },
  ];
  return (
    <div className="space-y-4 pb-2">
      <Table
        rows={rows}
        rowKey={(row) => row.key}
        columns={[
          { key: "metric", header: "", cell: (row) => <Text variant="hint">{row.label}</Text> },
          ...picked.map((name, index) => ({
            key: `file-${index}`,
            header: (
              <div className="space-y-1.5">
                <Select
                  aria-label={t("hf.compareFile", { index: index + 1 })}
                  value={name}
                  onChange={(event) =>
                    setPicked((list) =>
                      list.map((value, at) => (at === index ? event.target.value : value)),
                    )
                  }
                  options={files.map((item) => ({
                    value: item.filename,
                    label: item.quantization,
                  }))}
                />
                {picked.length > 1 && (
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={() => setPicked((list) => list.filter((_, at) => at !== index))}
                  >
                    {t("hf.compareRemove")}
                  </Button>
                )}
              </div>
            ),
            cell: (row: (typeof rows)[number]) => {
              const item = files.find((candidate) => candidate.filename === name);
              return item ? row.get(item) : null;
            },
          })),
        ]}
      />
      {picked.length < 3 && (
        <Button
          variant="secondary"
          size="sm"
          leading={<Plus size={icon.sm} />}
          onClick={() =>
            setPicked((list) => [...list, files[Math.max(0, files.length - 2)]?.filename ?? start])
          }
        >
          {t("hf.compareAdd")}
        </Button>
      )}
    </div>
  );
}
