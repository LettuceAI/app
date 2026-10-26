import { HfAccessNotice } from "./HfAccessNotice";
import { RepositoryReadme } from "./RepositoryReadme";
import { useCallback, useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ArrowDownToLine, Check, Cpu, ExternalLink, FileBox, Search, X } from "lucide-react";
import {
  Alert,
  Avatar,
  Badge,
  Button,
  Chip,
  confirm,
  EmptyState,
  Field,
  FilterBar,
  icon,
  IconButton,
  Input,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  MultiItemDownloadCard,
  SearchField,
  Tab,
  TabList,
  Tabs,
  Text,
} from "@lettuceai/crisp";
import type {
  HfBrowserLinks,
  ImageBundleAsset,
  ImageComponentRole,
  JobPage,
  JobView,
  HfSort,
  SdRunnability,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { useDownloads } from "@/entities/download";
import { downloadPhaseOf, downloadProgressOf } from "@/entities/job";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { ImageJob } from "../image/ImageJob";
import { inventoryQuery, useByteSize, useImageAction } from "../image/local-data";
import {
  browserKey,
  bundleProfilesQuery,
  bundleSearchQuery,
  bundleFilesQuery,
  repoFromLink,
  repositoryUrl,
  separator,
  useBrowserDebounce,
} from "./data";
import { ExternalButton, RepoCards, SortMenu, type BrowserSearch } from "./ModelBrowserPage";

export function ImageBrowser({
  search,
  update,
  links,
}: {
  search: BrowserSearch;
  links: HfBrowserLinks;
  update: (patch: Partial<BrowserSearch>, replace?: boolean) => void;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const client = useQueryClient();
  const size = useByteSize();
  const action = useImageAction();
  const profiles = useQuery(bundleProfilesQuery(api));
  const inventory = useQuery(inventoryQuery(api));
  const profile = profiles.data?.profiles.find((item) => item.id === search.profile);
  const [picks, setPicks] = useState<Partial<Record<ImageComponentRole, ImageBundleAsset>>>({});
  const [nameDraft, setName] = useState<string | null>(null);
  const name = nameDraft ?? profile?.display_name ?? "";
  const [input, setInput] = useState("");
  const query = useBrowserDebounce(input.trim());
  const [sort, setSort] = useState<HfSort>("trending_score");
  const [format, setFormat] = useState("any");
  const [tab, setTab] = useState("files");
  const [probe, setProbe] = useState<{ id: string; key: string } | null>(null);
  const [estimate, setEstimate] = useState<{
    key: string;
    verdict: SdRunnability;
  } | null>(null);
  const [accepted, setAccepted] = useState<{
    job_id: string;
    bundle_id: string;
  } | null>(null);
  const [registered, setRegistered] = useState<string | null>(null);
  const shared = useDownloads();
  const role: ImageComponentRole =
    profile?.required_roles.find((item) => item === search.role) ??
    profile?.required_roles[0] ??
    "diffusion_model";
  const repo = search.imageStep === "repo" ? search.repo : undefined;
  const step = search.imageStep ?? "search";
  const roles = profile?.required_roles ?? [];
  const complete = Boolean(profile) && roles.every((item) => picks[item]);
  const runtime = inventory.data?.active;
  const assets = roles.flatMap((item) => (picks[item] ? [picks[item]!] : []));
  const estimateKey = JSON.stringify([runtime, profile?.id, assets]);
  const currentEstimate = estimate?.key === estimateKey ? estimate.verdict : null;
  const ready =
    complete &&
    runtime &&
    currentEstimate &&
    !["incompatible_runtime", "failed", "not_installed", "inconclusive"].includes(
      currentEstimate.status,
    );
  const results = useQuery({
    ...bundleSearchQuery(
      api,
      profile?.id ?? "",
      role,
      query,
      sort,
      format === "any" ? null : format,
    ),
    enabled: Boolean(profile) && step === "search",
  });
  const files = useQuery({
    ...bundleFilesQuery(api, profile?.id ?? "", role, repo ?? ""),
    enabled: Boolean(profile && repo),
  });
  const readme = useQuery({
    queryKey: [...browserKey, "readme", repo],
    queryFn: () => api.call("hfReadme", { model_id: repo! }),
    enabled: Boolean(repo) && tab === "info",
  });
  const [settled, setSettled] = useState<string | null>(null);
  const onEstimate = useCallback(
    (job: JobView) => {
      if (probe) setSettled(probe.id);
      if (probe && job.result?.type === "runnability")
        setEstimate({ key: probe.key, verdict: job.result.verdict });
    },
    [probe],
  );
  const chooseProfile = async (id: string) => {
    if (
      profile &&
      profile.id !== id &&
      assets.length &&
      !(await confirm({
        title: t("image.local.bundle.changeTitle"),
        message: t("image.local.bundle.changeBody"),
        confirmLabel: t("hf.changeType"),
      }))
    )
      return;
    const next = profiles.data?.profiles.find((item) => item.id === id);
    if (!next) return;
    setPicks({});
    setName(next.display_name);
    setInput("");
    setAccepted(null);
    setRegistered(null);
    setProbe(null);
    setEstimate(null);
    update({
      profile: id,
      role: next.required_roles[0],
      imageStep: "search",
      repo: undefined,
    });
  };
  const choose = (asset: ImageBundleAsset) => {
    const next = { ...picks, [role]: asset };
    setPicks(next);
    setInput("");
    setProbe(null);
    const missing = roles.find((item) => !next[item]);
    update({
      repo: undefined,
      role: missing ?? role,
      imageStep: missing ? "search" : "review",
    });
  };
  const check = () => {
    if (!runtime || !profile || !complete) return;
    const key = estimateKey;
    action.mutate(async () => {
      const job = await api.call("sdBundleRunnability", {
        profile_id: profile.id,
        runtime_release: runtime.release,
        runtime_asset: runtime.asset,
        diffusion_bytes: picks.diffusion_model?.size ?? 0,
        text_encoder_bytes: picks.text_encoder?.size ?? 0,
        vae_bytes: picks.vae?.size ?? 0,
        vision_encoder_bytes: picks.vision_encoder?.size ?? 0,
        client_operation_id: crypto.randomUUID(),
      });
      setProbe({ id: job.job_id, key });
    });
  };
  const autoChecked = useRef("");
  useEffect(() => {
    if (step !== "review" || !complete || !runtime || currentEstimate || probe) return;
    if (autoChecked.current === estimateKey) return;
    autoChecked.current = estimateKey;
    check();
  });
  const install = () => {
    if (!runtime || !profile || !ready || !name.trim()) return;
    action.mutate(async () => {
      setAccepted(
        await api.call("hfImageBundleInstall", {
          profile_id: profile.id,
          display_name: name.trim(),
          runtime_release: runtime.release,
          runtime_asset: runtime.asset,
          assets,
        }),
      );
    });
  };
  const history = useQuery({
    queryKey: [...queryKeys.localImages.all, "bundle-history"],
    queryFn: async () => {
      const found: JobView[] = [];
      let cursor: string | null = null;
      do {
        const page: JobPage = await api.call("jobsList", {
          kinds: ["artifact_install"],
          states: [],
          subject: null,
          cursor,
          limit: 100,
        });
        found.push(...page.items.filter((job) => job.subject_detail?.type === "image_bundle"));
        cursor = page.next_cursor;
      } while (cursor);
      return found;
    },
  });
  const acceptedJob = accepted
    ? (shared.jobs.get(accepted.job_id) ?? history.data?.find((job) => job.id === accepted.job_id))
    : undefined;
  const modelId =
    registered ??
    (acceptedJob?.result?.type === "image_bundle" ? acceptedJob.result.model_id : null);
  const recommended =
    profile?.recommended_repositories
      .filter((item) => item.role === role)
      .map((item) => item.repository) ?? [];
  const models = [...(results.data?.models ?? [])].sort(
    (a, b) => Number(recommended.includes(b.model_id)) - Number(recommended.includes(a.model_id)),
  );
  const roleChips = (
    <div className="flex flex-wrap items-center gap-2">
      {roles.map((item, index) => (
        <Chip
          key={item}
          selected={role === item && step !== "review"}
          icon={
            picks[item] ? (
              <Check size={icon.sm} className="text-accent" />
            ) : (
              <span className="text-2xs tabular-nums">{index + 1}</span>
            )
          }
          disabled={Boolean(accepted)}
          title={picks[item] ? `${picks[item]!.model_id} / ${picks[item]!.relative_path}` : undefined}
          onClick={() => {
            setInput("");
            update({ role: item, repo: undefined, imageStep: "search" });
          }}
        >
          {t(`image.local.bundle.roles.${item}`)}
        </Chip>
      ))}
      {complete && step !== "review" && (
        <Button
          size="sm"
          variant="primary"
          onClick={() => update({ imageStep: "review", repo: undefined })}
        >
          {t("hf.review")}
        </Button>
      )}
    </div>
  );
  const changeType = (
    <Button
      size="sm"
      variant="ghost"
      disabled={Boolean(accepted)}
      onClick={() =>
        update({
          profile: undefined,
          repo: undefined,
          role: undefined,
          imageStep: undefined,
        })
      }
    >
      {t("hf.changeType")}
    </Button>
  );
  const roleName = t(`image.local.bundle.roles.${role}`);
  const stepCard = profile && (
    <ListGroup>
      <div className="px-4 py-3">
        <div className="flex items-center gap-3">
          <span className="min-w-0 flex-1">
            <span className="block text-xs text-fg-3">
              {t("hf.step", {
                step: roles.indexOf(role) + 1,
                total: roles.length,
                profile: profile.display_name,
              })}
            </span>
            <span className="block text-base font-semibold text-fg">
              {t("hf.chooseRole", { role: roleName.toLowerCase() })}
            </span>
          </span>
          {changeType}
        </div>
        <div className="mt-3">{roleChips}</div>
      </div>
    </ListGroup>
  );
  const repoCard = profile && (
    <ListGroup>
      <div className="px-4 py-3">
        <div className="flex items-center gap-3">
          <span className="min-w-0 flex-1 text-sm text-fg-2">
            {t("hf.pickingRole", { role: roleName.toLowerCase(), profile: profile.display_name })}
          </span>
          <span className="text-xs tabular-nums text-fg-3">
            {t("hf.pickedCount", { picked: assets.length, total: roles.length })}
          </span>
        </div>
        <div className="mt-2">{roleChips}</div>
      </div>
    </ListGroup>
  );
  const pendingProbe = probe !== null && settled !== probe.id;
  const totalBytes = assets.reduce((sum, asset) => sum + asset.size, 0);
  return (
    <div className="space-y-4">
      {profiles.isError && (
        <FailureState error={profiles.error} onRetry={() => void profiles.refetch()} />
      )}
      {inventory.isError && (
        <FailureState error={inventory.error} onRetry={() => void inventory.refetch()} />
      )}
      {inventory.data && !runtime && (
        <Alert
          tone="warning"
          title={t("hf.setupEngine")}
          description={t("image.local.bundle.engineBody")}
          action={
            <Button
              variant="secondary"
              onClick={() =>
                void navigate({
                  to: "/settings/$section",
                  params: { section: "imageGeneration" },
                  search: { localImageView: "engine" },
                })
              }
            >
              {t("hf.setupEngine")}
            </Button>
          }
        />
      )}
      {profiles.isPending ? (
        <LoadingState />
      ) : !profile ? (
        <>
          <div className="space-y-1">
            <Text variant="section">{t("hf.imageTitle")}</Text>
            <Text variant="secondary">{t("hf.imageBody")}</Text>
          </div>
          <div className="grid grid-cols-1 gap-3 @2xl:grid-cols-2">
            {profiles.data?.profiles.map((item) => (
              <ListGroup key={item.id}>
                <ListRowButton
                  chevron={false}
                  title={item.display_name}
                  description={
                    <span className="block space-y-2">
                      <span className="block">
                        {t("hf.profileMeta", {
                          family: item.family,
                          count: item.required_roles.length,
                        })}
                      </span>
                      <span className="block text-sm text-fg-2">{item.description}</span>
                      <span className="flex flex-wrap gap-1.5">
                        {item.required_roles.map((required) => (
                          <Badge key={required}>{t(`image.local.bundle.roles.${required}`)}</Badge>
                        ))}
                      </span>
                    </span>
                  }
                  onClick={() => void chooseProfile(item.id)}
                />
              </ListGroup>
            ))}
          </div>
        </>
      ) : step === "review" ? (
        <div className="grid items-start gap-5 @container @4xl:grid-cols-[minmax(0,1fr)_minmax(0,25rem)]">
          <div className="min-w-0 space-y-4">
            <ListGroup label={t("hf.files")}>
              {roles.map((item) => {
                const asset = picks[item];
                const label = t(`image.local.bundle.roles.${item}`);
                return (
                  <ListRow
                    key={item}
                    icon={<Check size={icon.lg} className="text-accent" />}
                    title={
                      <span className="block min-w-0">
                        <span className="flex items-center gap-1 text-xs font-normal text-fg-3">
                          {label}
                          {asset && (
                            <>
                              <span>{separator}</span>
                              <ExternalButton
                                size="sm"
                                url={repositoryUrl(links.repository_base, asset.model_id)}
                              >
                                {asset.model_id}
                              </ExternalButton>
                            </>
                          )}
                        </span>
                        <span className="block truncate font-mono text-sm text-fg">
                          {asset ? asset.relative_path : t("hf.selectFile")}
                        </span>
                      </span>
                    }
                    trailing={
                      <span className="flex items-center gap-2">
                        {asset?.format && <Badge>{asset.format}</Badge>}
                        {asset?.quantization && <Badge>{asset.quantization}</Badge>}
                        {asset && (
                          <span className="w-16 text-right text-sm tabular-nums text-fg-2">
                            {size(asset.size)}
                          </span>
                        )}
                        <IconButton
                          size="sm"
                          label={t("hf.changeRole", { role: label.toLowerCase() })}
                          disabled={Boolean(accepted)}
                          onClick={() => {
                            setPicks((current) => {
                              const next = { ...current };
                              delete next[item];
                              return next;
                            });
                            setProbe(null);
                            update({
                              role: item,
                              imageStep: "search",
                              repo: undefined,
                            });
                          }}
                        >
                          <X size={icon.sm} />
                        </IconButton>
                      </span>
                    }
                  />
                );
              })}
            </ListGroup>
            <Field label={t("hf.nameImage")}>
              <Input
                value={name}
                disabled={Boolean(accepted)}
                onChange={(event) => setName(event.target.value)}
              />
            </Field>
            {accepted && !modelId && <BundleDownload job={acceptedJob ?? null} title={name} />}
            {modelId && (
              <Alert
                tone="success"
                title={t("hf.bundleAdded", { name })}
                description={t("hf.bundleAddedBody")}
                action={
                  <Button
                    size="sm"
                    variant="secondary"
                    onClick={() =>
                      void navigate({
                        to: "/settings/models/$modelId",
                        params: { modelId },
                      })
                    }
                  >
                    {t("hf.openEditor")}
                  </Button>
                }
              />
            )}
            {acceptedJob &&
              acceptedJob.result?.type === "image_bundle" &&
              acceptedJob.result.state === "setup_failed" &&
              !modelId && (
                <Button
                  variant="primary"
                  loading={action.isPending}
                  onClick={() =>
                    action.mutate(async () => {
                      const result = await api.call("hfImageBundleRetryRegistration", {
                        bundle_id: accepted!.bundle_id,
                      });
                      setRegistered(result.model_id);
                    })
                  }
                >
                  {t("hf.retryRegistration")}
                </Button>
              )}
            {acceptedJob &&
              ["failed", "interrupted", "cancelled"].includes(acceptedJob.state) &&
              !modelId && (
                <Button
                  variant="secondary"
                  loading={action.isPending}
                  onClick={() =>
                    action.mutate(async () => {
                      const retried = await api.call("hfImageBundleRetry", {
                        bundle_id: accepted!.bundle_id,
                      });
                      if (retried.job_id) setAccepted({ ...accepted!, job_id: retried.job_id });
                      await client.invalidateQueries({
                        queryKey: queryKeys.localImages.all,
                      });
                    })
                  }
                >
                  {t("hf.retry")}
                </Button>
              )}
          </div>
          <ListGroup className="sticky top-0">
            <div className="divide-y divide-line">
              <div className="space-y-2 p-4">
                <div className="flex items-baseline gap-2">
                  <Text variant="section">{size(totalBytes)}</Text>
                  <Text variant="hint">{t("hf.toDownload")}</Text>
                </div>
                {currentEstimate ? (
                  <>
                    <div className="flex flex-wrap items-center gap-2">
                      <Badge tone={ready ? "accent" : "warning"} dot>
                        {t(`image.local.fit.${currentEstimate.status}`)}
                      </Badge>
                      {currentEstimate.estimate && (
                        <Text variant="label">{t(`hf.${currentEstimate.estimate.plan_mode}`)}</Text>
                      )}
                    </div>
                    {currentEstimate.estimate && (
                      <Text variant="hint" className="block text-xs leading-snug">
                        {t(`hf.modeBody.${currentEstimate.estimate.plan_mode}`)}
                      </Text>
                    )}
                  </>
                ) : (
                  <Text variant="hint" className="block text-xs leading-snug">
                    {t(pendingProbe ? "hf.checkingFit" : "hf.notChecked")}
                  </Text>
                )}
              </div>
              {currentEstimate?.estimate && (
                <div className="space-y-3 p-4">
                  {currentEstimate.estimate.devices.map((device) => (
                    <div key={device.id} className="flex items-center gap-2 text-sm">
                      <Cpu size={icon.sm} className="shrink-0 text-fg-3" />
                      <span className="min-w-0 flex-1 truncate font-medium text-fg">
                        {device.name}
                      </span>
                      <span className="shrink-0 text-xs tabular-nums text-fg-3">
                        <span className="font-medium text-fg">{size(device.budget_bytes)}</span>{" "}
                        {t("hf.deviceUsable", { total: size(device.free_bytes) })}
                      </span>
                    </div>
                  ))}
                  <div className="space-y-1.5">
                    {currentEstimate.estimate.placements.map((placement, index) => (
                      <div
                        key={`${placement.component}:${index}`}
                        className="flex items-center gap-3 text-xs"
                      >
                        <span className="w-28 shrink-0 text-fg-2">
                          {t(
                            placement.component === "dit"
                              ? "hf.componentDit"
                              : placement.component === "conditioner"
                                ? "hf.componentConditioner"
                                : "hf.componentVae",
                          )}
                        </span>
                        <span className="flex-1 tabular-nums text-fg-3">
                          {size(placement.params_bytes)}{" "}
                          <span className="text-fg-4">
                            {t("hf.workingExtra", { size: size(placement.compute_reserve_bytes) })}
                          </span>
                        </span>
                        <span className="font-mono text-fg-3">
                          {placement.cpu ? t("hf.cpu") : placement.targets.join(" + ")}
                        </span>
                      </div>
                    ))}
                  </div>
                </div>
              )}
              <div className="space-y-2 p-4">
                {!accepted && !currentEstimate && !pendingProbe && (
                  <Button
                    variant="secondary"
                    block
                    loading={action.isPending}
                    disabled={!runtime || !complete}
                    onClick={check}
                  >
                    {t("hf.checkFit")}
                  </Button>
                )}
                {!accepted && (
                  <Button
                    variant="primary"
                    block
                    size="lg"
                    loading={action.isPending}
                    leading={<ArrowDownToLine size={icon.md} />}
                    disabled={!ready || !name.trim()}
                    onClick={install}
                  >
                    {t("hf.downloadAdd")}
                  </Button>
                )}
                <Text variant="hint" className="block text-center">
                  {t("hf.engineChecked")}
                </Text>
                {probe && probe.key === estimateKey && pendingProbe && (
                  <ImageJob id={probe.id} title={t("image.local.fit.title")} onDone={onEstimate} />
                )}
              </div>
            </div>
          </ListGroup>
        </div>
      ) : repo ? (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <span className="flex items-center gap-2 text-sm text-fg-2">
              <Avatar name={repo.split("/")[0] ?? repo} size="xs" />
              {repo.split("/")[0]}
            </span>
          </div>
          {repoCard}
          <Tabs value={tab} onChange={setTab}>
            <TabList aria-label={repo}>
              <Tab value="files" {...(files.data ? { count: files.data.assets.length } : {})}>
                {t("hf.filesFit")}
              </Tab>
              <Tab value="info">{t("hf.info")}</Tab>
            </TabList>
          </Tabs>
          {tab === "info" ? (
            readme.isPending ? (
              <LoadingState />
            ) : readme.isError ? (
              <FailureState error={readme.error} onRetry={() => void readme.refetch()} />
            ) : (
              <RepositoryReadme
                repo={repo}
                repositoryBase={links.repository_base}
                markdown={readme.data?.markdown ?? ""}
              />
            )
          ) : files.isPending ? (
            <LoadingState />
          ) : files.isError ? (
            <>
              <FailureState error={files.error} onRetry={() => void files.refetch()} />
              <HfAccessNotice error={files.error} onToken={() => update({ token: true })} />
            </>
          ) : (
            <>
              {files.data.assets.some((asset) => asset.gated) && (
                <Alert
                  tone="warning"
                  title={t("hf.gatedFilesTitle")}
                  description={t("hf.gatedFilesBody")}
                />
              )}
              {files.data.assets.length === 0 ? (
                <EmptyState title={t("hf.noFiles")} />
              ) : (
                <ListGroup>
                  {files.data.assets.map((asset) => {
                    const on = picks[role]?.selection_id === asset.selection_id;
                    return (
                      <ListRow
                        key={asset.selection_id}
                        className={on ? "bg-accent/10" : undefined}
                        icon={<FileBox size={icon.lg} />}
                        title={
                          <span className="block truncate font-mono text-sm">
                            {asset.relative_path}
                          </span>
                        }
                        trailing={
                          <span className="flex items-center gap-2">
                            <Badge>{asset.format}</Badge>
                            {asset.quantization && <Badge>{asset.quantization}</Badge>}
                            {asset.gated && <Badge tone="warning">{t("hf.gated")}</Badge>}
                            <span className="w-16 text-right text-sm tabular-nums text-fg-2">
                              {size(asset.size)}
                            </span>
                            {on ? (
                              <Badge tone="accent" dot>
                                {t("hf.picked")}
                              </Badge>
                            ) : (
                              <Button size="sm" variant="primary" onClick={() => choose(asset)}>
                                {t("hf.useFile")}
                              </Button>
                            )}
                          </span>
                        }
                      />
                    );
                  })}
                </ListGroup>
              )}
            </>
          )}
        </>
      ) : (
        <>
          {stepCard}
          <div className="flex items-center gap-2">
            <div className="min-w-0 flex-1 [&>div]:w-full">
              <SearchField
                className="min-w-0 flex-1"
                value={input}
                onChange={setInput}
                aria-label={t("hf.search")}
                placeholder={t("hf.searchRole", { role: roleName.toLowerCase() })}
              />
            </div>
            <SortMenu value={sort} onChange={setSort} />
          </div>
          <FilterBar
            aria-label={t("hf.files")}
            value={format}
            onChange={setFormat}
            options={[
              { value: "any", label: t("hf.anyFormat") },
              { value: "gguf", label: t("hf.gguf") },
              { value: "safetensors", label: t("hf.safetensors") },
            ]}
          />
          {repoFromLink(input, links.repository_base) && (
            <Button
              variant="secondary"
              onClick={() =>
                update({
                  repo: repoFromLink(input, links.repository_base)!,
                  imageStep: "repo",
                })
              }
            >
              {t("hf.openRepo", {
                repo: repoFromLink(input, links.repository_base)!,
              })}
            </Button>
          )}
          <Text variant="label" className="block px-1 pt-1">
            {t("hf.reposFit")}
          </Text>
          {results.isPending ? (
            <LoadingState />
          ) : results.isError ? (
            <FailureState error={results.error} onRetry={() => void results.refetch()} />
          ) : models.length ? (
            <RepoCards
              models={models}
              recommended={recommended}
              onRepo={(model) => {
                setTab("files");
                update({ repo: model, imageStep: "repo" });
              }}
            />
          ) : (
            <EmptyState
              icon={<Search size={icon.xl} />}
              title={t("hf.nothingFits")}
              description={t("hf.nothingFitsBody", {
                role: roleName.toLowerCase(),
                profile: profile.display_name,
              })}
            />
          )}
          {results.isSuccess && recommended[0] && !models.some((model) => model.model_id === recommended[0]) && (
            <Button
              variant="ghost"
              leading={<ExternalLink size={icon.sm} />}
              onClick={() => {
                setInput("");
                setFormat("any");
              }}
            >
              {t("hf.showRecommended", { repo: recommended[0] })}
            </Button>
          )}
        </>
      )}
      {history.isError && (
        <FailureState error={history.error} onRetry={() => void history.refetch()} />
      )}
      {!accepted &&
        history.data?.map((job) => (
          <BundleHistoryItem key={job.id} job={shared.jobs.get(job.id) ?? job} />
        ))}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}

function BundleDownload({ job, title }: { job: JobView | null; title: string }) {
  const { t } = useTranslation("settings");
  const shared = useDownloads();
  const action = useImageAction();
  const progress = downloadProgressOf(job);
  return (
    <>
      <MultiItemDownloadCard
        title={title}
        items={[
          {
            id: job?.id ?? title,
            name: title,
            status: job ? downloadPhaseOf(job) : "queued",
            ...(progress.fraction === undefined ? {} : { progress: progress.fraction }),
            ...(progress.size === undefined ? {} : { size: progress.size }),
            ...(progress.speed === undefined ? {} : { speed: progress.speed }),
            ...(progress.eta === undefined ? {} : { eta: progress.eta }),
            error: t("errors.code.unavailable", { ns: "common" }),
          },
        ]}
        {...(job &&
        !["failed", "succeeded", "cancelled", "interrupted"].includes(job.state) &&
        shared.cancel
          ? { onCancelAll: () => action.mutate(() => shared.cancel!(job.id)) }
          : {})}
      />
      {action.isError && <FailureState error={action.error} size="sm" />}
    </>
  );
}

function BundleHistoryItem({ job }: { job: JobView }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const action = useImageAction();
  const shared = useDownloads();
  const [retryId, setRetryId] = useState<string | null>(null);
  const [registered, setRegistered] = useState<string | null>(null);
  const current = retryId ? (shared.jobs.get(retryId) ?? job) : job;
  if (job.subject_detail?.type !== "image_bundle") return null;
  const { bundle_id, display_name } = job.subject_detail;
  const result = current.result?.type === "image_bundle" ? current.result : null;
  const modelId = registered ?? result?.model_id;
  return (
    <ListGroup label={display_name}>
      <BundleDownload job={current} title={display_name} />
      {modelId && (
        <ListRowButton
          title={t("hf.openEditor")}
          onClick={() =>
            void navigate({
              to: "/settings/models/$modelId",
              params: { modelId },
            })
          }
        />
      )}
      {!modelId && ["failed", "cancelled", "interrupted"].includes(current.state) && (
        <ListRow
          title={t("hf.retry")}
          trailing={
            <Button
              variant="secondary"
              size="sm"
              loading={action.isPending}
              onClick={() =>
                action.mutate(async () => {
                  const retried = await api.call("hfImageBundleRetry", {
                    bundle_id,
                  });
                  if (retried.job_id) setRetryId(retried.job_id);
                })
              }
            >
              {t("hf.retry")}
            </Button>
          }
        />
      )}
      {!modelId && result?.state === "setup_failed" && (
        <ListRow
          title={t("hf.retryRegistration")}
          trailing={
            <Button
              variant="primary"
              size="sm"
              loading={action.isPending}
              onClick={() =>
                action.mutate(async () => {
                  const registered = await api.call("hfImageBundleRetryRegistration", {
                    bundle_id,
                  });
                  setRegistered(registered.model_id);
                })
              }
            >
              {t("hf.retryRegistration")}
            </Button>
          }
        />
      )}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </ListGroup>
  );
}
