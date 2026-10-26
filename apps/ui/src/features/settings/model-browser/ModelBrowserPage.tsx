import { useState } from "react";
import { useInfiniteQuery, useQuery, useQueryClient } from "@tanstack/react-query";
import { useNavigate, useSearch, useRouter } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ArrowDownUp, ChevronDown, HardDrive, KeyRound } from "lucide-react";
import {
  Avatar,
  Button,
  Chip,
  EmptyState,
  FilterBar,
  icon,
  IconButton,
  ListGroup,
  ListRowButton,
  LoadingState,
  Menu,
  MenuContent,
  MenuItem,
  MenuTrigger,
  Page,
  SearchField,
  SecretInput,
  Sheet,
  Tab,
  TabList,
  Tabs,
  Text,
  Tooltip,
} from "@lettuceai/crisp";
import type { HfBrowserLinks, HfModelSummary, HfSort } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { appStatusQuery } from "@/api/app-status-query";
import { openExternalUrl } from "@/api/external-links";
import { FailureState } from "@/shared/ui/FailureState";
import { useProviderData, useSettingsAction, useSettingsApi, useSettingsLayout } from "../data";
import {
  browserKey,
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
  const client = useQueryClient();
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
  const destinationName = account?.label ?? t("hf.local");
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
  const root = () =>
    update({
      view: undefined,
      repo: undefined,
      author: undefined,
      tab: undefined,
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
  const [token, setToken] = useState("");
  const auth = useQuery({
    queryKey: [...browserKey, "auth"],
    queryFn: () => api.call("hfAuthStatus"),
  });
  const action = useSettingsAction();
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
  const downloadedCount = account
    ? serverModels.data?.models.length
    : localFiles.data?.files.length;
  const downloadedLabel =
    downloadedCount === undefined
      ? t("hf.downloadedLabel")
      : t("hf.downloaded", { count: downloadedCount });
  const title = search.repo
    ? search.repo.split("/").at(-1)
    : (search.author ??
      (search.view === "downloaded"
        ? account
          ? t("hf.onServer", { name: account.label })
          : downloadedLabel
        : t("hf.title")));
  return (
    <Page
      title={title}
      meta={phone ? undefined : t("hf.source")}
      onBack={back}
      backLabel={t("sections.models.title")}
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
          <Tooltip placement="left" content={t("hf.tokenHint")}>
            <IconButton label={t("hf.token")} onClick={() => setTokenOpen(true)}>
              <KeyRound size={icon.lg} />
            </IconButton>
          </Tooltip>
        </>
      }
      filters={
        !phone && (
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
      ) : search.kind === "image" && !mobile ? (
        <ImageBrowser search={search} update={update} links={links.data} />
      ) : (
        <>
          <div className={phone ? "w-full" : "flex justify-end"}>
            {phone ? (
              <>
                <ListGroup>
                  <ListRowButton
                    title={account ? t("hf.pullTo", { name: account.label }) : destinationName}
                    description={account?.base_url ?? undefined}
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
                        title={
                          hasSprout(server)
                            ? server.label
                            : t("hf.noSprout", { name: server.label })
                        }
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
                    block={phone}
                    variant="secondary"
                    trailing={<ChevronDown size={icon.sm} />}
                  >
                    {account ? t("hf.pullTo", { name: account.label }) : destinationName}
                  </Button>
                </MenuTrigger>
                <MenuContent align="end">
                  {!mobile && (
                    <MenuItem
                      selected={!account}
                      onSelect={() => update({ destination: undefined })}
                    >
                      {t("hf.local")}
                    </MenuItem>
                  )}
                  {servers.map((server) => (
                    <MenuItem
                      key={server.id}
                      selected={server.id === destination}
                      onSelect={() => update({ destination: server.id })}
                    >
                      {hasSprout(server) ? server.label : t("hf.noSprout", { name: server.label })}
                    </MenuItem>
                  ))}
                </MenuContent>
              </Menu>
            )}
          </div>
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
              onRepo={(repo) => update({ repo, tab: "info" })}
              onClearAuthor={root}
            />
          )}
        </>
      )}
      <Sheet
        open={tokenOpen}
        onClose={() => setTokenOpen(false)}
        title={t("hf.token")}
        description={t("hf.tokenHint")}
      >
        <div className="space-y-3">
          {auth.isPending ? (
            <LoadingState />
          ) : auth.isError ? (
            <FailureState error={auth.error} onRetry={() => void auth.refetch()} />
          ) : (
            <Text>
              {auth.data.type === "valid"
                ? t("hf.tokenValid", { name: auth.data.username })
                : auth.data.type === "missing"
                  ? t("hf.tokenMissing")
                  : auth.data.type === "invalid"
                    ? t("hf.tokenInvalid")
                    : t("hf.tokenUnknown")}
            </Text>
          )}
          <SecretInput aria-label={t("hf.token")} value={token} onChange={setToken} />
          <div className="flex gap-2">
            <Button
              variant="primary"
              disabled={!token.trim()}
              loading={action.isPending}
              onClick={() =>
                action.mutate(async () => {
                  await api.call("hfAuthSave", { token: token.trim() });
                  setToken("");
                  await Promise.all([
                    auth.refetch(),
                    client.invalidateQueries({ queryKey: browserKey }),
                    client.invalidateQueries({
                      queryKey: queryKeys.localImages.all,
                    }),
                  ]);
                })
              }
            >
              {t("hf.saveToken")}
            </Button>
            <Button
              variant="ghost"
              disabled={action.isPending}
              onClick={() =>
                action.mutate(async () => {
                  await api.call("hfAuthClear");
                  await Promise.all([
                    auth.refetch(),
                    client.invalidateQueries({ queryKey: browserKey }),
                    client.invalidateQueries({
                      queryKey: queryKeys.localImages.all,
                    }),
                  ]);
                })
              }
            >
              {t("hf.removeToken")}
            </Button>
          </div>
          {action.isError && <FailureState error={action.error} size="sm" />}
        </div>
      </Sheet>
    </Page>
  );
}

export function SortMenu({ value, onChange }: { value: HfSort; onChange: (sort: HfSort) => void }) {
  const { t } = useTranslation("settings");
  const { phone } = useSettingsLayout();
  return (
    <Menu>
      <MenuTrigger>
        {phone ? (
          <IconButton label={t("hf.sort")} variant="secondary">
            <ArrowDownUp size={icon.md} />
          </IconButton>
        ) : (
          <Button variant="secondary" leading={<ArrowDownUp size={icon.sm} />}>
            {t(`hf.${value}`)}
          </Button>
        )}
      </MenuTrigger>
      <MenuContent align="end">
        {(["trending_score", "downloads", "likes", "last_modified"] as const).map((sort) => (
          <MenuItem key={sort} selected={sort === value} onSelect={() => onChange(sort)}>
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
  const authors = [...new Set(models.map((item) => item.author))].sort();
  const avatars = useQuery({
    queryKey: [...browserKey, "avatars", authors],
    queryFn: () => api.call("hfAvatars", { authors }),
    enabled: authors.length > 0,
  });
  const number = new Intl.NumberFormat(i18n.language, { notation: "compact" });
  return (
    <div className="@container">
      <div className="grid gap-2 @2xl:grid-cols-2 @5xl:grid-cols-3">
        {models.map((model) => (
          <ListGroup key={model.model_id}>
            <ListRowButton
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
              title={model.model_id}
              description={
                <span className="block space-y-1">
                  <span className="block truncate">
                    {[
                      model.pipeline_tag,
                      parameterBillions(model.model_id) === null
                        ? null
                        : t("hf.parameters", {
                            count: parameterBillions(model.model_id)!,
                          }),
                      model.last_modified
                        ? new Intl.DateTimeFormat(i18n.language, {
                            dateStyle: "medium",
                          }).format(new Date(model.last_modified))
                        : null,
                    ]
                      .filter(Boolean)
                      .join(" · ")}
                  </span>
                  <span className="block">
                    {t("hf.stats", {
                      downloads: number.format(model.downloads),
                      likes: number.format(model.likes),
                    })}
                  </span>
                  {recommended?.includes(model.model_id) && (
                    <Text variant="hint">{t("hf.recommended")}</Text>
                  )}
                </span>
              }
              onClick={() => onRepo(model.model_id)}
            />
          </ListGroup>
        ))}
      </div>
    </div>
  );
}

function TextSearch({
  author,
  links,
  accountId,
  destination,
  onRepo,
  onClearAuthor,
}: {
  author: string | undefined;
  links: HfBrowserLinks;
  accountId: string | undefined;
  destination: string | undefined;
  onRepo: (repo: string) => void;
  onClearAuthor: () => void;
}) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
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
  const pasted = repoFromLink(input, links.repository_base);
  const authorProfile =
    profile.data?.profile.type === "found" ? profile.data.profile.overview : null;
  return (
    <div className="space-y-4">
      {author && (
        <ListGroup>
          <ListRowButton
            title={authorProfile?.fullname ?? author}
            description={
              authorProfile
                ? t("hf.authorModels", {
                    count: authorProfile.num_models,
                    followers: authorProfile.num_followers,
                  })
                : author
            }
            icon={<Avatar name={author} src={authorProfile?.avatar_url ?? null} />}
            onClick={onClearAuthor}
          />
        </ListGroup>
      )}
      {profile.isError && (
        <FailureState error={profile.error} onRetry={() => void profile.refetch()} size="sm" />
      )}
      <div className="flex gap-2">
        <SearchField
          value={input}
          onChange={setInput}
          placeholder={t("hf.search")}
          aria-label={t("hf.search")}
          className="min-w-0 flex-1"
        />
        <SortMenu value={sort} onChange={setSort} />
      </div>
      <div className="flex gap-2">
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
        <Chip selected={vision} onClick={() => setVision(!vision)}>
          {t("hf.vision")}
        </Chip>
      </div>
      <BrowserDownloads accountId={accountId} destination={destination} />
      {pasted && (
        <Button variant="secondary" onClick={() => onRepo(pasted)}>
          {t("hf.openRepo", { repo: pasted })}
        </Button>
      )}
      {result.isPending ? (
        <LoadingState />
      ) : result.isError && all.length === 0 ? (
        <FailureState error={result.error} onRetry={() => void result.refetch()} />
      ) : (
        <>
          <RepoCards models={models} onRepo={onRepo} />
          {models.length === 0 && (
            <EmptyState
              title={t("hf.empty")}
              description={t("hf.emptyBody")}
              action={
                <Button
                  variant="secondary"
                  onClick={() => {
                    setSize("all");
                    setVision(false);
                  }}
                >
                  {t("hf.clearFilters")}
                </Button>
              }
            />
          )}
          {result.isError && (
            <FailureState error={result.error} onRetry={() => void result.fetchNextPage()} />
          )}
          {result.hasNextPage && (
            <Button
              variant="ghost"
              loading={result.isFetchingNextPage}
              onClick={() => void result.fetchNextPage()}
            >
              {t("hf.more")}
            </Button>
          )}
        </>
      )}
    </div>
  );
}

export function ExternalButton({ url, children }: { url: string; children: React.ReactNode }) {
  const action = useSettingsAction();
  return (
    <>
      <Button variant="ghost" onClick={() => action.mutate(() => openExternalUrl(url))}>
        {children}
      </Button>
      {action.isError && <FailureState error={action.error} size="sm" />}
    </>
  );
}
