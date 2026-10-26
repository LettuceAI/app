import { useState, type ReactNode } from "react";
import { useInfiniteQuery, useQuery, useQueryClient, type UseQueryResult } from "@tanstack/react-query";
import { useNavigate, useSearch, useRouter } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import {
  ArrowDownToLine,
  ArrowDownUp,
  ChevronDown,
  Clock,
  ExternalLink,
  HardDrive,
  Heart,
  Image as ImageIcon,
  KeyRound,
  Lock,
  Server,
  ThumbsUp,
  TrendingUp,
} from "lucide-react";
import {
  Alert,
  Avatar,
  Badge,
  Button,
  Chip,
  EmptyState,
  Field,
  FilterBar,
  icon,
  IconButton,
  ListGroup,
  ListRow,
  ListRowButton,
  LoadingState,
  Menu,
  MenuContent,
  MenuItem,
  MenuLabel,
  MenuTrigger,
  Page,
  relativeTime,
  SearchField,
  SecretInput,
  Sheet,
  Skeleton,
  Tab,
  TabList,
  Tabs,
  Text,
  Tooltip,
} from "@lettuceai/crisp";
import type { HfTokenStatus, HfBrowserLinks, HfModelSummary, HfSort } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { appStatusQuery } from "@/api/app-status-query";
import { openExternalUrl } from "@/api/external-links";
import { FailureState } from "@/shared/ui/FailureState";
import { useProviderData, useSettingsAction, useSettingsApi, useSettingsLayout } from "../data";
import {
  browserKey,
  bundleProfilesQuery,
  prettyPipeline,
  repositoryUrl,
  separator,
  useBrowserDebounce,
  hasSprout,
  pageSize,
  parameterBillions,
  repoFromLink,
} from "./data";
import { BrowserDownloads } from "./downloads";
import { TextModel } from "./TextModel";
import { DownloadedModels } from "./DownloadedModels";
import { ImageBrowser } from "./ImageBrowser";

export interface BrowserSearch {
  token?: boolean | undefined;
  kind?: "text" | "image" | undefined;
  view?: "downloaded" | undefined;
  repo?: string | undefined;
  author?: string | undefined;
  destination?: string | undefined;
  tab?: "info" | "download" | undefined;
  profile?: string | undefined;
  role?: string | undefined;
  imageStep?: "search" | "repo" | "review" | undefined;
}
export function ModelBrowserPage() {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const navigate = useNavigate();
  const router = useRouter();
  const search = useSearch({ from: "/settings/model-browser" });
  const { phone } = useSettingsLayout();
  const { accounts } = useProviderData();
  const status = useQuery(appStatusQuery(api));
  const links = useQuery({
    queryKey: [...browserKey, "links"],
    queryFn: () => api.call("hfBrowserLinks"),
  });
  const mobile = status.data?.platform === "android" || status.data?.platform === "ios";
  const servers = accounts.data?.filter((account) => account.provider_kind === "ollama") ?? [];
  const destination = search.destination ?? (mobile ? servers[0]?.id : undefined);
  const account = servers.find((item) => item.id === destination);
  const update = (patch: Partial<BrowserSearch>, replace = false) =>
    void navigate({
      to: "/settings/model-browser",
      search: { ...search, ...patch },
      replace,
      state: (previous) => ({
        ...previous,
        hfBrowserPushed: replace ? previous.hfBrowserPushed : true,
      }),
    });
  const back = () => {
    if (router.state.location.state.hfBrowserPushed) router.history.back();
    else if (search.repo || search.author || search.view || search.profile)
      update(
        {
          repo: undefined,
          author: undefined,
          view: undefined,
          profile: undefined,
          role: undefined,
          imageStep: undefined,
          tab: undefined,
        },
        true,
      );
    else
      void navigate({
        to: "/settings/$section",
        params: { section: "models" },
      });
  };
  const tokenOpen = search.token === true;
  const setTokenOpen = (open: boolean) => {
    if (open) update({ token: true });
    else if (router.state.location.state.hfBrowserPushed) router.history.back();
    else update({ token: undefined }, true);
  };
  const [destinationOpen, setDestinationOpen] = useState(false);
  const auth = useQuery({
    queryKey: [...browserKey, "auth"],
    queryFn: () => api.call("hfAuthStatus"),
  });
  const localFiles = useQuery({
    queryKey: queryKeys.localModels.all,
    queryFn: () => api.call("localModelsList"),
    enabled: !mobile && !account,
  });
  const serverModels = useQuery({
    queryKey: [...browserKey, "ollama", account?.id],
    queryFn: () => api.call("ollamaModelsList", { provider_account_id: account!.id }),
    enabled: Boolean(account),
  });
  const imageMode = search.kind === "image" && !mobile;
  const profiles = useQuery({
    ...bundleProfilesQuery(api),
    enabled: imageMode && Boolean(search.profile),
  });
  const imageProfile = profiles.data?.profiles.find((item) => item.id === search.profile);
  const reviewing = imageMode && search.imageStep === "review" && Boolean(imageProfile);
  const rootList = !search.repo && !search.author && !search.view && !reviewing;
  const downloadedCount = account
    ? serverModels.data?.models.length
    : localFiles.data?.files.length;
  const downloadedLabel =
    downloadedCount === undefined
      ? account
        ? t("hf.onServer", { name: account.label })
        : t("hf.downloadedLabel")
      : account
        ? t("hf.onServerCount", { name: account.label, count: downloadedCount })
        : t("hf.downloaded", { count: downloadedCount });
  const title = search.repo
    ? search.repo.split("/").at(-1)
    : search.author
      ? search.author
      : search.view === "downloaded"
        ? account
          ? t("hf.onServer", { name: account.label })
          : t("hf.downloadedTitle")
        : reviewing
          ? t("hf.reviewName", { name: imageProfile!.display_name })
          : t("hf.title");
  const meta = search.repo
    ? search.repo.split("/")[0]
    : search.author
      ? t("sections.models.title")
      : search.view === "downloaded"
        ? account
          ? t("hf.downloadedServerMeta", { host: account.base_url ?? account.label })
          : t("hf.downloadedLocalMeta")
        : reviewing
          ? t("hf.fileCount", { count: imageProfile!.required_roles.length })
          : t("hf.source");
  const backTarget = search.repo || reviewing ? t("hf.searchBack") : search.author || search.view ? t("hf.title") : t("sections.models.title");
  const destinationLabel = account ? account.label : t("hf.local");
  const devices = useQuery({
    queryKey: ["llama", "devices"],
    queryFn: () => api.call("llamaDevices"),
    enabled: !mobile,
  });
  const gpus =
    devices.data?.devices.filter((device) => device.device_type.toLowerCase() !== "cpu") ?? [];
  const localLabel = gpus[0]
    ? t("hf.localDevice", {
        device:
          gpus.length > 1
            ? t("hf.deviceMore", { name: gpus[0].name, count: gpus.length - 1 })
            : gpus[0].name,
      })
    : t("hf.local");
  const destinationItems = (
    <>
      {!mobile && (
        <>
          <MenuLabel>{t("hf.downloadHere")}</MenuLabel>
          <MenuItem
            icon={<HardDrive size={icon.md} />}
            selected={!account}
            onSelect={() => update({ destination: undefined })}
          >
            {localLabel}
          </MenuItem>
        </>
      )}
      <MenuLabel>{t("hf.pullToOllama")}</MenuLabel>
      {servers.map((server) => (
        <MenuItem
          key={server.id}
          icon={<Server size={icon.md} />}
          selected={server.id === destination}
          onSelect={() => update({ destination: server.id })}
        >
          {hasSprout(server) ? server.label : t("hf.noSprout", { name: server.label })}
        </MenuItem>
      ))}
    </>
  );
  const destinationControl = phone ? (
    <>
      <ListGroup>
        <ListRowButton
          icon={<Server size={icon.lg} />}
          title={account ? t("hf.pullTo", { name: account.label }) : destinationLabel}
          description={
            account
              ? [account.base_url, hasSprout(account) ? null : t("hf.noSproutHint")]
                  .filter(Boolean)
                  .join(" · ")
              : undefined
          }
          onClick={() => setDestinationOpen(true)}
        />
      </ListGroup>
      <Sheet
        open={destinationOpen}
        onClose={() => setDestinationOpen(false)}
        title={t("hf.destination")}
      >
        <ListGroup>
          {!mobile && (
            <ListRowButton
              icon={<HardDrive size={icon.lg} />}
              title={t("hf.local")}
              onClick={() => {
                update({ destination: undefined });
                setDestinationOpen(false);
              }}
            />
          )}
          {servers.map((server) => (
            <ListRowButton
              key={server.id}
              icon={<Server size={icon.lg} />}
              title={hasSprout(server) ? server.label : t("hf.noSprout", { name: server.label })}
              description={server.base_url ?? undefined}
              onClick={() => {
                update({ destination: server.id });
                setDestinationOpen(false);
              }}
            />
          ))}
        </ListGroup>
      </Sheet>
    </>
  ) : (
    <Menu>
      <MenuTrigger>
        <Button
          variant="secondary"
          className={account ? "border-accent/40 text-accent" : undefined}
          leading={account ? <Server size={icon.sm} /> : <HardDrive size={icon.sm} />}
          trailing={<ChevronDown size={icon.sm} />}
        >
          <span className="max-w-36 truncate">{destinationLabel}</span>
        </Button>
      </MenuTrigger>
      <MenuContent align="end" className="w-72">
        {destinationItems}
      </MenuContent>
    </Menu>
  );
  const showKinds = !phone && rootList;
  return (
    <Page
      title={title}
      meta={phone ? undefined : meta}
      onBack={back}
      backLabel={backTarget}
      contentClassName="space-y-4 @container"
      actions={
        <>
          {phone ? (
            <IconButton
              label={downloadedLabel}
              onClick={() =>
                update({
                  view: "downloaded",
                  repo: undefined,
                  author: undefined,
                })
              }
            >
              <HardDrive size={icon.lg} />
            </IconButton>
          ) : (
            <Button
              size="sm"
              variant="ghost"
              leading={<HardDrive size={icon.sm} />}
              onClick={() =>
                update({
                  view: "downloaded",
                  repo: undefined,
                  author: undefined,
                })
              }
            >
              {downloadedLabel}
            </Button>
          )}
          <Tooltip
            placement="left"
            content={
              auth.data?.type === "valid"
                ? t("hf.tokenValid", { name: auth.data.username })
                : t("hf.tokenTooltip")
            }
          >
            <IconButton label={t("hf.token")} onClick={() => setTokenOpen(true)}>
              <KeyRound
                size={icon.lg}
                className={auth.data?.type === "valid" ? "text-accent" : undefined}
              />
            </IconButton>
          </Tooltip>
        </>
      }
      filters={
        showKinds && (
          <Tabs
            value={search.kind ?? "text"}
            onChange={(kind) =>
              update({
                kind: kind as "text" | "image",
                repo: undefined,
                author: undefined,
                view: undefined,
              })
            }
          >
            <TabList aria-label={t("hf.title")}>
              <Tab value="text">{t("hf.text")}</Tab>
              <Tab value="image">{t("hf.image")}</Tab>
            </TabList>
          </Tabs>
        )
      }
    >
      {accounts.isPending || status.isPending || links.isPending ? (
        <LoadingState />
      ) : accounts.isError ? (
        <FailureState error={accounts.error} onRetry={() => void accounts.refetch()} />
      ) : status.isError ? (
        <FailureState error={status.error} onRetry={() => void status.refetch()} />
      ) : links.isError ? (
        <FailureState error={links.error} onRetry={() => void links.refetch()} />
      ) : mobile && !account ? (
        <EmptyState
          icon={<Server size={icon.xl} />}
          title={t("hf.noOllama")}
          description={t("hf.noOllamaBody")}
          action={
            <Button
              onClick={() =>
                void navigate({
                  to: "/settings/$section",
                  params: { section: "providers" },
                })
              }
            >
              {t("hf.providers")}
            </Button>
          }
        />
      ) : imageMode ? (
        <ImageBrowser search={search} update={update} links={links.data} />
      ) : (
        <>
          {phone && rootList && !search.author && destinationControl}
          {search.view === "downloaded" ? (
            <DownloadedModels account={account} />
          ) : search.repo ? (
            <TextModel
              key={`${search.repo}:${destination ?? "local"}`}
              repo={search.repo}
              links={links.data}
              account={account}
              phone={phone}
              tab={search.tab ?? "info"}
              onTab={(tab) => update({ tab }, true)}
              onAuthor={(author) => update({ author, repo: undefined, tab: undefined })}
              onToken={() => setTokenOpen(true)}
            />
          ) : (
            <TextSearch
              author={search.author}
              links={links.data}
              accountId={account?.id}
              destination={account?.label}
              destinationControl={phone ? undefined : destinationControl}
              onRepo={(repo) => update({ repo, tab: "info" })}
            />
          )}
        </>
      )}
      <TokenSheet
        open={tokenOpen}
        onClose={() => setTokenOpen(false)}
        auth={auth}
      />
    </Page>
  );
}

function TokenSheet({
  open,
  onClose,
  auth,
}: {
  open: boolean;
  onClose: () => void;
  auth: UseQueryResult<HfTokenStatus>;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const client = useQueryClient();
  const action = useSettingsAction();
  const [token, setToken] = useState("");
  const refresh = () =>
    Promise.all([
      auth.refetch(),
      client.invalidateQueries({ queryKey: browserKey }),
      client.invalidateQueries({ queryKey: queryKeys.localImages.all }),
    ]);
  const valid = auth.data?.type === "valid";
  return (
    <Sheet open={open} onClose={onClose} title={t("hf.token")} description={t("hf.tokenSheetHint")}>
      <div className="space-y-4 pb-2">
        {auth.isPending ? (
          <LoadingState />
        ) : auth.isError ? (
          <FailureState error={auth.error} onRetry={() => void auth.refetch()} />
        ) : auth.data.type === "valid" ? (
          <Alert
            tone="success"
            title={t("hf.tokenValid", { name: auth.data.username })}
            description={t("hf.tokenValidBody")}
            action={
              <Button
                size="sm"
                variant="ghost"
                disabled={action.isPending}
                onClick={() =>
                  action.mutate(async () => {
                    await api.call("hfAuthClear");
                    await refresh();
                  })
                }
              >
                {t("hf.removeToken")}
              </Button>
            }
          />
        ) : auth.data.type === "invalid" ? (
          <Alert tone="danger" title={t("hf.tokenRefused")} description={t("hf.tokenRefusedBody")} />
        ) : auth.data.type === "unknown" ? (
          <Text variant="hint">{t("hf.tokenUnknown")}</Text>
        ) : null}
        {!valid && (
          <>
            <Field label={t("hf.tokenField")} hint={t("hf.tokenFieldHint")}>
              <SecretInput value={token} onChange={setToken} placeholder={tokenPlaceholder} />
            </Field>
            <Text variant="hint" className="flex items-start gap-1.5">
              <Lock size={icon.sm} className="mt-0.5 shrink-0" />
              {t("hf.tokenPrivacy")}
            </Text>
            <div className="flex justify-end gap-2">
              <Button variant="ghost" onClick={onClose}>
                {t("manage.cancel")}
              </Button>
              <Button
                variant="primary"
                disabled={!token.trim()}
                loading={action.isPending}
                onClick={() =>
                  action.mutate(async () => {
                    await api.call("hfAuthSave", { token: token.trim() });
                    setToken("");
                    await refresh();
                  })
                }
              >
                {t("hf.saveToken")}
              </Button>
            </div>
          </>
        )}
        {action.isError && <FailureState error={action.error} size="sm" />}
      </div>
    </Sheet>
  );
}

const tokenPlaceholder = "hf_…";

const sortIcons = {
  trending_score: <TrendingUp size={icon.sm} />,
  downloads: <ArrowDownToLine size={icon.sm} />,
  likes: <ThumbsUp size={icon.sm} />,
  last_modified: <Clock size={icon.sm} />,
} as const;

export function SortMenu({ value, onChange }: { value: HfSort; onChange: (sort: HfSort) => void }) {
  const { t } = useTranslation("settings");
  const { phone } = useSettingsLayout();
  return (
    <Menu>
      <MenuTrigger>
        {phone ? (
          <IconButton
            label={`${t("hf.sort")}: ${t(`hf.${value}`)}`}
            variant="secondary"
            size="lg"
          >
            <ArrowDownUp size={icon.md} />
          </IconButton>
        ) : (
          <Button
            variant="secondary"
            leading={<ArrowDownUp size={icon.sm} />}
            trailing={<ChevronDown size={icon.sm} />}
          >
            {t(`hf.${value}`)}
          </Button>
        )}
      </MenuTrigger>
      <MenuContent align="end" className="w-52">
        <MenuLabel>{t("hf.sort")}</MenuLabel>
        {(["trending_score", "downloads", "likes", "last_modified"] as const).map((sort) => (
          <MenuItem
            key={sort}
            icon={sortIcons[sort]}
            selected={sort === value}
            onSelect={() => onChange(sort)}
          >
            {t(`hf.${sort}`)}
          </MenuItem>
        ))}
      </MenuContent>
    </Menu>
  );
}

export function RepoCards({
  models,
  onRepo,
  recommended,
}: {
  models: HfModelSummary[];
  onRepo: (repo: string) => void;
  recommended?: string[];
}) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const { phone } = useSettingsLayout();
  const authors = [...new Set(models.map((item) => item.author))].sort();
  const avatars = useQuery({
    queryKey: [...browserKey, "avatars", authors],
    queryFn: () => api.call("hfAvatars", { authors }),
    enabled: authors.length > 0,
  });
  const number = new Intl.NumberFormat(i18n.language, { notation: "compact" });
  return (
    <div className="@container">
      <div className="grid grid-cols-1 gap-2 @2xl:grid-cols-2 @5xl:grid-cols-3">
        {models.map((model) => {
          const billions = parameterBillions(model.model_id);
          const parts = [
            prettyPipeline(model.pipeline_tag),
            billions === null ? null : t("hf.parameters", { count: billions }),
            model.last_modified
              ? relativeTime(new Date(model.last_modified), undefined, i18n.language)
              : null,
          ].filter(Boolean);
          return (
            <ListGroup key={model.model_id}>
              <ListRowButton
                chevron={false}
                icon={
                  <Avatar
                    name={model.author}
                    src={
                      avatars.data?.avatars.find((avatar) => avatar.author === model.author)?.url ??
                      null
                    }
                    size="sm"
                  />
                }
                title={
                  <span className="flex min-w-0 items-center gap-2">
                    <span className="truncate">{model.model_id}</span>
                    {recommended?.includes(model.model_id) && (
                      <Badge tone="accent">{t("hf.recommended")}</Badge>
                    )}
                  </span>
                }
                description={
                  <span className="flex items-center gap-1 truncate">
                    <span className="truncate">{parts.join(" · ")}</span>
                    {phone && (
                      <>
                        <span className="text-fg-4">{separator}</span>
                        <span className="flex shrink-0 items-center gap-0.5 tabular-nums">
                          <ArrowDownToLine size={icon.xs} />
                          {number.format(model.downloads)}
                        </span>
                      </>
                    )}
                  </span>
                }
                trailing={
                  phone ? undefined : (
                    <span className="flex shrink-0 items-center gap-2.5 text-xs tabular-nums text-fg-3">
                      <span className="flex items-center gap-1">
                        <ArrowDownToLine size={icon.xs} />
                        {number.format(model.downloads)}
                      </span>
                      <span className="flex items-center gap-1">
                        <Heart size={icon.xs} />
                        {number.format(model.likes)}
                      </span>
                    </span>
                  )
                }
                onClick={() => onRepo(model.model_id)}
              />
            </ListGroup>
          );
        })}
      </div>
    </div>
  );
}

function TextSearch({
  destinationControl,
  author,
  links,
  accountId,
  destination,
  onRepo,
}: {
  author: string | undefined;
  destinationControl: ReactNode;
  links: HfBrowserLinks;
  accountId: string | undefined;
  destination: string | undefined;
  onRepo: (repo: string) => void;
}) {
  const { t, i18n } = useTranslation("settings");
  const api = useSettingsApi();
  const { phone } = useSettingsLayout();
  const [input, setInput] = useState("");
  const query = useBrowserDebounce(input.trim(), 250);
  const [sort, setSort] = useState<HfSort>("trending_score");
  const [size, setSize] = useState("all");
  const [vision, setVision] = useState(false);
  const result = useInfiniteQuery({
    queryKey: [...browserKey, "search", query, sort, author],
    initialPageParam: 0,
    queryFn: ({ pageParam }) =>
      api.call("hfSearch", {
        query,
        sort,
        author: author ?? null,
        offset: pageParam,
        limit: pageSize,
        mode: "llm",
        unfiltered: false,
      }),
    getNextPageParam: (last, pages) =>
      last.models.length < pageSize ? undefined : pages.length * pageSize,
  });
  const profile = useQuery({
    queryKey: [...browserKey, "author", author],
    queryFn: () =>
      api.call("hfAuthor", {
        author: author!,
        search: null,
        limit: 1,
        offset: 0,
        sort: null,
      }),
    enabled: Boolean(author),
  });
  const all = result.data?.pages.flatMap((page) => page.models) ?? [];
  const models = all.filter((model) => {
    const b = parameterBillions(model.model_id);
    return (
      (!vision ||
        model.tags.some((tag) => /vision|image-text-to-text|multimodal/i.test(tag)) ||
        model.pipeline_tag === "image-text-to-text") &&
      (size === "all" ||
        (b !== null &&
          (size === "small"
            ? b <= 4
            : size === "mid"
              ? b > 4 && b <= 9
              : size === "large"
                ? b > 9 && b <= 16
                : size === "xl"
                  ? b > 16 && b <= 35
                  : b > 35)))
    );
  });
  const filtered = size !== "all" || vision;
  const pasted = repoFromLink(input, links.repository_base);
  const authorProfile =
    profile.data?.profile.type === "found" ? profile.data.profile.overview : null;
  const number = new Intl.NumberFormat(i18n.language, { notation: "compact" });
  return (
    <div className="space-y-4">
      {author && (
        <ListGroup>
          <ListRow
            icon={<Avatar name={author} src={authorProfile?.avatar_url ?? null} />}
            title={
              <span className="flex items-center gap-2">
                {authorProfile?.fullname ?? author}
                {authorProfile?.kind === "org" && <Badge>{t("hf.organization")}</Badge>}
              </span>
            }
            description={
              authorProfile
                ? [
                    author,
                    t("hf.authorModels", {
                      count: number.format(authorProfile.num_models),
                      followers: number.format(authorProfile.num_followers),
                    }),
                  ].join(" · ")
                : author
            }
            trailing={
              <ExternalButton
                size="sm"
                icon={<ExternalLink size={icon.sm} />}
                url={repositoryUrl(links.repository_base, author)}
              >
                {t("hf.openProfile")}
              </ExternalButton>
            }
          />
        </ListGroup>
      )}
      {profile.isError && (
        <FailureState error={profile.error} onRetry={() => void profile.refetch()} size="sm" />
      )}
      <div className="flex w-full items-center gap-2">
        <div className="min-w-0 flex-1 [&>div]:w-full">
          <SearchField
            value={input}
            onChange={setInput}
            placeholder={author ? t("hf.searchAuthor", { author }) : t("hf.search")}
            aria-label={t("hf.search")}
            className="min-w-0 flex-1"
          />
        </div>
        <SortMenu value={sort} onChange={setSort} />
        {destinationControl}
      </div>
      <div className="flex items-center gap-2">
        <FilterBar
          className="min-w-0 flex-1"
          aria-label={t("hf.sizes")}
          value={size}
          onChange={setSize}
          options={[
            { value: "all", label: t("hf.all") },
            { value: "small", label: t("hf.sizeSmall") },
            { value: "mid", label: t("hf.sizeMid") },
            { value: "large", label: t("hf.sizeLarge") },
            { value: "xl", label: t("hf.sizeXl") },
            { value: "xxl", label: t("hf.sizeXxl") },
          ]}
        />
        <Chip
          selected={vision}
          icon={<ImageIcon size={icon.sm} />}
          onClick={() => setVision(!vision)}
        >
          {t("hf.vision")}
        </Chip>
      </div>
      {!author && <BrowserDownloads accountId={accountId} destination={destination} variant="cards" />}
      {pasted && (
        <Button
          variant="secondary"
          leading={<ExternalLink size={icon.sm} />}
          onClick={() => onRepo(pasted)}
        >
          {t("hf.openRepo", { repo: pasted })}
        </Button>
      )}
      {result.isPending ? (
        <div className={phone ? "grid gap-2" : "grid gap-2 @2xl:grid-cols-2 @5xl:grid-cols-3"}>
          {Array.from({ length: 12 }, (_, index) => (
            <div
              key={index}
              className="flex items-center gap-3 rounded-2xl border border-line bg-surface-1 px-3.5 py-3"
            >
              <Skeleton shape="circle" className="h-8 w-8" />
              <div className="min-w-0 flex-1 space-y-1.5">
                <Skeleton className="h-3 w-3/4" />
                <Skeleton className="h-2.5 w-1/2" />
              </div>
            </div>
          ))}
        </div>
      ) : result.isError && all.length === 0 ? (
        <FailureState error={result.error} onRetry={() => void result.refetch()} />
      ) : (
        <>
          {models.length > 0 && (
            <Text variant="label" className="block px-1 pt-1">
              {query
                ? t("hf.resultCount", { count: models.length })
                : t(`hf.${sort}`)}
            </Text>
          )}
          <RepoCards models={models} onRepo={onRepo} />
          {models.length === 0 && (
            <EmptyState
              title={filtered ? t("hf.emptyFiltered") : t("hf.emptyNone")}
              description={filtered ? t("hf.emptyFilteredBody") : t("hf.emptyNoneBody")}
              {...(filtered
                ? {
                    action: (
                      <Button
                        variant="secondary"
                        onClick={() => {
                          setSize("all");
                          setVision(false);
                        }}
                      >
                        {t("hf.clearFilters")}
                      </Button>
                    ),
                  }
                : {})}
            />
          )}
          {result.isError && (
            <FailureState error={result.error} onRetry={() => void result.fetchNextPage()} />
          )}
          {result.hasNextPage && (
            <div className="flex justify-center pt-2">
              <Button
                variant="ghost"
                loading={result.isFetchingNextPage}
                onClick={() => void result.fetchNextPage()}
              >
                {t("hf.more")}
              </Button>
            </div>
          )}
        </>
      )}
    </div>
  );
}


export function ExternalButton({
  url,
  children,
  size,
  icon: leading,
}: {
  url: string;
  children: React.ReactNode;
  size?: "sm" | "md";
  icon?: ReactNode;
}) {
  const action = useSettingsAction();
  return (
    <>
      <Button
        variant="ghost"
        {...(size ? { size } : {})}
        {...(leading ? { leading } : {})}
        onClick={() => action.mutate(() => openExternalUrl(url))}
      >
        {children}
      </Button>
      {action.isError && <FailureState error={action.error} size="sm" />}
    </>
  );
}
