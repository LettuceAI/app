import { useCallback, useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Check, FileBox } from "lucide-react";
import {
  Alert,
  Button,
  confirm,
  Field,
  icon,
  Input,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  SearchField,
  SecretInput,
  Sheet,
} from "@lettuceai/crisp";
import type {
  ImageBundleAsset,
  ImageBundleProfile,
  ImageComponentRole,
  JobPage,
  JobView,
  SdRunnability,
  SdRuntimeRef,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { ImageJob } from "./ImageJob";
import { bundleProfilesQuery, bundleSearchQuery, bundleFilesQuery } from "../model-browser/data";
import { inventoryQuery, useByteSize, useImageAction } from "./local-data";

export function BundleView({
  onJob,
  onEngine,
}: {
  onJob: (id: string, title: string) => void;
  onEngine: () => void;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const profiles = useQuery(bundleProfilesQuery(api));
  const inventory = useQuery(inventoryQuery(api));
  const [profileId, setProfile] = useState<string | null>(null);
  const profile = profiles.data?.profiles.find((item) => item.id === profileId);
  const chooseProfile = async (id: string) => {
    if (id === profileId) return;
    if (
      profileId &&
      !(await confirm({
        title: t("image.local.bundle.changeTitle"),
        message: t("image.local.bundle.changeBody"),
        confirmLabel: t("image.local.bundle.change"),
      }))
    )
      return;
    setProfile(id);
  };
  return (
    <div className="space-y-5">
      {profiles.isError && (
        <FailureState error={profiles.error} onRetry={() => void profiles.refetch()} />
      )}
      {inventory.isError && (
        <FailureState error={inventory.error} onRetry={() => void inventory.refetch()} />
      )}
      {inventory.data && !inventory.data.active && (
        <Alert
          tone="warning"
          title={t("image.local.catalog.engineFirst")}
          description={t("image.local.bundle.engineBody")}
          action={
            <Button variant="secondary" size="sm" onClick={onEngine}>
              {t("image.local.engine.setup")}
            </Button>
          }
        />
      )}
      {!profiles.data ? (
        !profiles.isError && <LoadingState />
      ) : (
        <ListGroup label={t("image.local.bundle.architecture")}>
          {profiles.data.profiles.map((item) => (
            <ListRowButton
              key={item.id}
              title={item.display_name}
              description={item.description}
              chevron={false}
              trailing={
                profileId === item.id ? <Check size={icon.md} className="text-accent" /> : undefined
              }
              onClick={() => void chooseProfile(item.id)}
            />
          ))}
        </ListGroup>
      )}
      {profile && (
        <BundleForm
          key={profile.id}
          profile={profile}
          runtime={inventory.data?.active ?? null}
          onJob={onJob}
        />
      )}
      <BundleHistory onJob={onJob} />
    </div>
  );
}

function BundleForm({
  profile,
  runtime,
  onJob,
}: {
  profile: ImageBundleProfile;
  runtime: SdRuntimeRef | null;
  onJob: (id: string, title: string) => void;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useImageAction();
  const size = useByteSize();
  const [name, setName] = useState(profile.display_name);
  const [assets, setAssets] = useState<Partial<Record<ImageComponentRole, ImageBundleAsset>>>({});
  const [editing, setEditing] = useState<ImageComponentRole | null>(null);
  const [estimate, setEstimate] = useState<{ key: string; verdict: SdRunnability } | null>(null);
  const [probe, setProbe] = useState<{ id: string; key: string } | null>(null);
  const [submitted, setSubmitted] = useState(false);
  const role = editing;
  const complete = profile.required_roles.every((required) => assets[required]);
  const estimateKey = JSON.stringify([
    runtime,
    profile.required_roles.map((required) => assets[required]),
  ]);
  const currentEstimate = estimate?.key === estimateKey ? estimate.verdict : null;
  const ready =
    complete &&
    currentEstimate &&
    !["incompatible_runtime", "failed", "not_installed", "inconclusive"].includes(
      currentEstimate.status,
    );
  const onEstimate = useCallback(
    (job: JobView) => {
      if (job.result?.type === "runnability" && probe)
        setEstimate({ key: probe.key, verdict: job.result.verdict });
    },
    [probe],
  );
  const check = () => {
    if (!runtime) return;
    action.mutate(async () => {
      const accepted = await api.call("sdBundleRunnability", {
        profile_id: profile.id,
        runtime_release: runtime.release,
        runtime_asset: runtime.asset,
        diffusion_bytes: assets.diffusion_model?.size ?? 0,
        text_encoder_bytes: assets.text_encoder?.size ?? 0,
        vae_bytes: assets.vae?.size ?? 0,
        vision_encoder_bytes: assets.vision_encoder?.size ?? 0,
        client_operation_id: crypto.randomUUID(),
      });
      setProbe({ id: accepted.job_id, key: estimateKey });
    });
  };
  const install = () => {
    if (!runtime || !ready || !name.trim()) return;
    action.mutate(async () => {
      const accepted = await api.call("hfImageBundleInstall", {
        profile_id: profile.id,
        display_name: name.trim(),
        runtime_release: runtime.release,
        runtime_asset: runtime.asset,
        assets: profile.required_roles.flatMap((required) =>
          assets[required] ? [assets[required]!] : [],
        ),
      });
      onJob(accepted.job_id, name.trim());
      setSubmitted(true);
    });
  };
  return (
    <div className="space-y-5">
      <ListGroup label={t("image.local.bundle.files")}>
        {profile.required_roles.map((required, index) => (
          <ListRowButton
            key={required}
            icon={<FileBox size={icon.lg} />}
            tone={assets[required] ? "accent" : "neutral"}
            title={t(`image.local.bundle.roles.${required}`)}
            description={
              assets[required] ? (
                <span className="font-mono text-xs">
                  {t("image.local.bundle.path", {
                    repo: assets[required]!.model_id,
                    file: assets[required]!.relative_path,
                  })}
                </span>
              ) : (
                t("image.local.bundle.step", {
                  step: index + 1,
                  total: profile.required_roles.length,
                })
              )
            }
            trailing={assets[required] ? size(assets[required]!.size) : undefined}
            disabled={submitted}
            onClick={() => setEditing(required)}
          />
        ))}
      </ListGroup>
      <Sheet
        open={role !== null && !submitted}
        onClose={() => setEditing(null)}
        placement="auto"
        title={role ? t(`image.local.bundle.roles.${role}`) : undefined}
        description={t("image.local.bundle.matching")}
      >
        {role && (
          <BundleFiles
            key={role}
            profile={profile.id}
            role={role}
            onChoose={(asset) => {
              setAssets((current) => ({ ...current, [role]: asset }));
              setEditing(null);
            }}
          />
        )}
      </Sheet>
      {complete && (
        <>
          <ListGroup label={t("image.local.bundle.before")}>
            <ListRow
              title={t("image.local.fit.title")}
              description={
                currentEstimate
                  ? t(`image.local.fit.${currentEstimate.status}`)
                  : t("image.local.fit.notChecked")
              }
              trailing={
                <Button
                  variant="ghost"
                  size="sm"
                  disabled={!runtime || action.isPending}
                  onClick={check}
                >
                  {t("image.local.fit.check")}
                </Button>
              }
            />
            <ListRow
              title={t("image.local.bundle.total")}
              trailing={size(
                profile.required_roles.reduce(
                  (sum, required) => sum + (assets[required]?.size ?? 0),
                  0,
                ),
              )}
            />
          </ListGroup>
          <Field label={t("image.local.bundle.name")} hint={t("image.local.bundle.nameHint")}>
            <Input
              value={name}
              disabled={submitted}
              aria-label={t("image.local.bundle.name")}
              onChange={(event) => setName(event.target.value)}
            />
          </Field>
          {!submitted && (
            <Button
              block
              disabled={!runtime || !ready || !name.trim() || action.isPending}
              loading={action.isPending}
              onClick={install}
            >
              {t("image.local.bundle.download")}
            </Button>
          )}
        </>
      )}
      {probe && probe.key === estimateKey && (
        <ImageJob id={probe.id} title={t("image.local.fit.title")} onDone={onEstimate} />
      )}
      {submitted && <ListRow title={t("image.local.bundle.queued")} />}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}

function BundleFiles({
  profile,
  role,
  onChoose,
}: {
  profile: string;
  role: ImageComponentRole;
  onChoose: (asset: ImageBundleAsset) => void;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const size = useByteSize();
  const action = useImageAction();
  const [input, setInput] = useState("");
  const [query, setQuery] = useState("");
  const [repo, setRepo] = useState<string | null>(null);
  const [token, setToken] = useState("");
  const found = useQuery({
    ...bundleSearchQuery(api, profile, role, query),
    enabled: repo === null,
  });
  const files = useQuery(bundleFilesQuery(api, profile, role, repo ?? ""));
  const failure = repo ? files : found;
  return (
    <div className="space-y-3">
      {repo ? (
        <Button variant="ghost" onClick={() => setRepo(null)}>
          {t("back")}
        </Button>
      ) : (
        <form
          className="flex gap-2"
          onSubmit={(event) => {
            event.preventDefault();
            setQuery(input.trim());
          }}
        >
          <SearchField
            value={input}
            onChange={setInput}
            aria-label={t("image.local.bundle.search")}
            placeholder={t("image.local.bundle.search")}
          />
          <Button type="submit" variant="secondary">
            {t("manage.search")}
          </Button>
        </form>
      )}
      {failure.isError ? (
        <>
          <FailureState error={failure.error} onRetry={() => void failure.refetch()} />
          <ListGroup>
            <ListRow
              title={t("image.local.bundle.token")}
              trailing={
                <SecretInput
                  value={token}
                  onChange={setToken}
                  aria-label={t("image.local.bundle.token")}
                />
              }
            />
            <ListRow
              title={t("image.local.bundle.tokenBody")}
              trailing={
                <Button
                  size="sm"
                  disabled={!token.trim() || action.isPending}
                  onClick={() =>
                    action.mutate(async () => {
                      await api.call("hfAuthSave", { token: token.trim() });
                      setToken("");
                    })
                  }
                >
                  {t("manage.save")}
                </Button>
              }
            />
          </ListGroup>
        </>
      ) : failure.isPending ? (
        <LoadingState />
      ) : (
        <ListGroup label={repo ?? t(`image.local.bundle.roles.${role}`)}>
          {repo
            ? files.data?.assets.map((asset) => (
                <ListRowButton
                  key={`${asset.revision}/${asset.relative_path}`}
                  title={asset.relative_path}
                  description={`${asset.quantization ?? asset.format} · ${size(asset.size)}`}
                  onClick={() => onChoose(asset)}
                />
              ))
            : found.data?.models.map((model) => (
                <ListRowButton
                  key={model.model_id}
                  title={model.model_id}
                  onClick={() => setRepo(model.model_id)}
                />
              ))}
          {(repo ? files.data?.assets.length === 0 : found.data?.models.length === 0) && (
            <ListRow title={t("tools.noResults")} />
          )}
        </ListGroup>
      )}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </div>
  );
}

function BundleHistory({ onJob }: { onJob: (id: string, title: string) => void }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useImageAction();
  const [registered, setRegistered] = useState<Record<string, string>>({});
  const jobs = useQuery({
    queryKey: [...queryKeys.jobs.lists(), "image-bundles"],
    queryFn: async () => {
      const result: JobView[] = [];
      let cursor: string | null = null;
      do {
        const page: JobPage = await api.call("jobsList", {
          kinds: ["artifact_install"],
          states: [],
          subject: null,
          cursor,
          limit: 100,
        });
        result.push(...page.items.filter((job) => job.subject_detail?.type === "image_bundle"));
        cursor = page.next_cursor;
      } while (cursor);
      return result;
    },
  });
  return (
    <>
      {jobs.isError && <FailureState error={jobs.error} onRetry={() => void jobs.refetch()} />}
      {jobs.data?.map((job) => {
        if (job.subject_detail?.type !== "image_bundle") return null;
        const { bundle_id, display_name } = job.subject_detail;
        const result = job.result?.type === "image_bundle" ? job.result : null;
        const modelId = registered[bundle_id] ?? result?.model_id;
        return (
          <ListGroup key={job.id} label={display_name}>
            <ListRow title={t(`tools.job.${job.state}`)} />
            {modelId && <BundleOpenModel id={modelId} />}
            {!modelId && ["failed", "cancelled", "interrupted"].includes(job.state) && (
              <ListRow
                title={t("image.local.bundle.retryDownload")}
                trailing={
                  <Button
                    size="sm"
                    disabled={action.isPending}
                    onClick={() =>
                      action.mutate(async () => {
                        const retried = await api.call("hfImageBundleRetry", { bundle_id });
                        if (retried.job_id) onJob(retried.job_id, display_name);
                      })
                    }
                  >
                    {t("image.local.bundle.retryDownload")}
                  </Button>
                }
              />
            )}
            {!modelId &&
              (result?.state === "setup_failed" ||
                ["failed", "interrupted"].includes(job.state)) && (
                <ListRow
                  title={t("image.local.bundle.retryRegistration")}
                  trailing={
                    <Button
                      size="sm"
                      variant="ghost"
                      disabled={action.isPending}
                      onClick={() =>
                        action.mutate(async () => {
                          const result = await api.call("hfImageBundleRetryRegistration", {
                            bundle_id,
                          });
                          setRegistered((current) => ({
                            ...current,
                            [bundle_id]: result.model_id,
                          }));
                        })
                      }
                    >
                      {t("image.local.bundle.retryRegistration")}
                    </Button>
                  }
                />
              )}
          </ListGroup>
        );
      })}
      {action.isError && <FailureState error={action.error} size="sm" />}
    </>
  );
}

function BundleOpenModel({ id }: { id: string }) {
  const { t } = useTranslation("settings");
  const navigate = useNavigate();
  return (
    <ListRowButton
      title={t("image.local.bundle.openModel")}
      onClick={() => void navigate({ to: "/settings/models/$modelId", params: { modelId: id } })}
    />
  );
}
