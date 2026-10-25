import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import {
  Activity,
  Calculator,
  Clock,
  Download,
  Gauge,
  List,
  MoreHorizontal,
  RefreshCw,
  Trash2,
} from "lucide-react";
import {
  Alert,
  Button,
  CrispStringsProvider,
  icon,
  IconButton,
  Menu,
  MenuContent,
  MenuItem,
  MenuSeparator,
  MenuTrigger,
  Page,
  Panel,
  Progress,
  Skeleton,
  Tab,
  TabList,
  Tabs,
  Text,
} from "@lettuceai/crisp";
import type { UsageRow } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { ActionError } from "@/shared/ui/ActionError";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { AppTime } from "./AppTime";
import { ClearRecords } from "./ClearRecords";
import {
  clockQuery,
  currentInstant,
  detailQuery,
  rangeDates,
  requestFilters,
  type UsageTab,
} from "./data";
import { NanoGpt } from "./NanoGpt";
import { Overview } from "./Overview";
import { RequestDetail } from "./RequestDetail";
import { Requests } from "./Requests";
import { useNano } from "./use-nano";
import { useUsageActions } from "./use-actions";

function UsageContent() {
  const { t, i18n } = useTranslation("usage");
  const api = useSettingsApi();
  const client = useQueryClient();
  const { phone, back } = useSettingsLayout();
  const search = useSearch({ from: "/settings/$section" });
  const navigate = useNavigate();
  const router = useRouter();
  const main = useRef<HTMLElement>(null);
  const pushed = useLocation({ select: (location) => location.state.usagePushed });
  const tab = search.usageTab ?? "overview";
  const range = search.usageRange ?? "30d";
  const provider = search.usageProvider ?? "all";
  const status = search.usageStatus ?? "all";
  const operation = search.usageOperation ?? "all";
  const model = search.usageModel ?? "all";
  const clock = useQuery(clockQuery());
  const [initialNow] = useState(currentInstant);
  const now = clock.data ?? initialNow;
  const accounts = useQuery({
    queryKey: queryKeys.providers.accounts(),
    queryFn: () => api.call("providerAccountsList"),
  });
  const nanoAccounts = (accounts.data ?? []).filter((item) => item.provider_kind === "nanogpt");
  const accountId = search.usageAccount ?? nanoAccounts[0]?.id ?? "";
  const filters =
    tab === "requests"
      ? requestFilters(status, operation, model)
      : {
          ...requestFilters("all", "all", "all"),
          range: rangeDates(range, now),
          provider_kind: provider === "all" ? null : provider,
        };
  const actions = useUsageActions(filters);
  const nano = useNano(
    nanoAccounts.find((item) => item.id === accountId),
    tab === "nanogpt",
  );
  const change = (patch: { [K in keyof typeof search]?: (typeof search)[K] | undefined }) =>
    void navigate({
      to: "/settings/$section",
      params: { section: "usage" },
      search: Object.fromEntries(
        Object.entries({ ...search, ...patch }).filter(([, value]) => value !== undefined),
      ) as typeof search,
      replace: true,
      resetScroll: false,
    });
  const close = () => {
    if (pushed) router.history.back();
    else change({ usagePanel: undefined, usageRequest: undefined });
  };
  const open = (row?: UsageRow) => {
    if (row) client.setQueryData(detailQuery(api, row.id).queryKey, row);
    void navigate({
      to: "/settings/$section",
      params: { section: "usage" },
      search: {
        ...search,
        usagePanel: row ? "request" : "clear",
        ...(row ? { usageRequest: row.id } : {}),
      },
      state: { usagePushed: true },
      resetScroll: false,
    });
  };
  const progress = actions.job?.progress;
  return (
    <CrispStringsProvider
      locale={i18n.language}
      strings={{
        sheetClose: t("close"),
        selectMenuClear: t("clearSelection"),
        selectMenuEmpty: t("noOptions"),
      }}
    >
      <Page
        mainRef={main}
        title={t("title")}
        meta={phone ? undefined : t("meta")}
        {...back}
        contentClassName="w-full space-y-5"
        actions={
          <div className="flex items-center gap-1">
            {!phone && (
              <IconButton
                label={t("exportCsv")}
                disabled={actions.exporting || !actions.exportReady}
                onClick={actions.exportCsv}
              >
                <Download size={icon.lg} />
              </IconButton>
            )}
            <IconButton
              label={t("refresh")}
              disabled={tab === "nanogpt" && (nano.busy || !nano.ready)}
              onClick={tab === "nanogpt" ? nano.refresh : actions.refresh}
            >
              <RefreshCw size={icon.lg} />
            </IconButton>
            <Menu>
              <MenuTrigger>
                <IconButton label={t("more")}>
                  <MoreHorizontal size={icon.lg} />
                </IconButton>
              </MenuTrigger>
              <MenuContent align="end">
                {phone && (
                  <MenuItem
                    icon={<Download size={icon.md} />}
                    disabled={actions.exporting || !actions.exportReady}
                    onSelect={actions.exportCsv}
                  >
                    {t("exportCsv")}
                  </MenuItem>
                )}
                <MenuItem
                  icon={<Calculator size={icon.md} />}
                  disabled={
                    actions.starting ||
                    actions.active ||
                    actions.recovered.isPending ||
                    actions.recovered.isError
                  }
                  onSelect={actions.start}
                >
                  {t("prices.action")}
                </MenuItem>
                <MenuSeparator />
                <MenuItem danger icon={<Trash2 size={icon.md} />} onSelect={() => open()}>
                  {t("clear.title")}
                </MenuItem>
              </MenuContent>
            </Menu>
          </div>
        }
        filters={
          <Tabs
            value={tab}
            onChange={(next) => {
              change({
                usageTab: next as UsageTab,
                usagePanel: undefined,
                usageRequest: undefined,
              });
              main.current?.scrollTo({ top: 0 });
            }}
          >
            <TabList aria-label={t("sections")}>
              <Tab value="overview" icon={phone ? undefined : <Activity size={icon.md} />}>
                {t("tabs.overview")}
              </Tab>
              <Tab value="requests" icon={phone ? undefined : <List size={icon.md} />}>
                {t("tabs.requests")}
              </Tab>
              <Tab value="time" icon={phone ? undefined : <Clock size={icon.md} />}>
                {t(phone ? "tabs.timeShort" : "tabs.time")}
              </Tab>
              {nanoAccounts.length > 0 && (
                <Tab value="nanogpt" icon={phone ? undefined : <Gauge size={icon.md} />}>
                  {t("tabs.nanogpt")}
                </Tab>
              )}
            </TabList>
          </Tabs>
        }
      >
        <ActionError
          error={actions.error}
          busy={actions.exporting || actions.starting || actions.cancelling}
          onRetry={actions.retry}
        />
        {actions.recovered.isError && (
          <FailureState
            error={actions.recovered.error}
            onRetry={() => void actions.recovered.refetch()}
            size="sm"
          />
        )}
        {actions.watchError !== null && (
          <FailureState error={actions.watchError} onRetry={actions.retryWatch} size="sm" />
        )}
        {actions.active && (
          <Panel
            title={t("prices.running")}
            action={
              <Button
                size="sm"
                variant="ghost"
                disabled={
                  actions.cancelling ||
                  actions.job?.state === "cancellation_requested" ||
                  actions.job?.state === "cleaning_up"
                }
                onClick={actions.cancel}
              >
                {t("cancel")}
              </Button>
            }
          >
            <Progress
              label={t("prices.running")}
              {...(progress?.total
                ? { value: Math.min(100, (progress.current / progress.total) * 100) }
                : {})}
            />
            <Text variant="hint">
              {progress?.total
                ? t("prices.progress", { current: progress.current, total: progress.total })
                : t("prices.background")}
            </Text>
          </Panel>
        )}
        {actions.job && ["failed", "interrupted"].includes(actions.job.state) && (
          <Alert
            tone="danger"
            title={t("prices.failed")}
            description={t(`priceFailures.${actions.job.failure?.reason ?? "unknown"}`, {
              defaultValue: t("priceFailures.unknown"),
            })}
            action={
              <Button size="sm" variant="secondary" onClick={actions.start}>
                {t("retry")}
              </Button>
            }
          />
        )}
        {tab === "overview" ? (
          <Overview
            range={range}
            provider={provider}
            now={now}
            phone={phone}
            onRange={(usageRange) => change({ usageRange })}
            onProvider={(usageProvider) => change({ usageProvider })}
            onOpen={open}
            onRequests={() => change({ usageTab: "requests" })}
          />
        ) : tab === "requests" ? (
          <Requests
            status={status}
            operation={operation}
            model={model}
            phone={phone}
            now={now}
            onFilter={change}
            onOpen={open}
          />
        ) : tab === "time" ? (
          <AppTime
            range={range}
            now={now}
            phone={phone}
            onRange={(usageRange) => change({ usageRange })}
          />
        ) : accounts.isPending ? (
          <Skeleton className="h-64 w-full" />
        ) : accounts.isError ? (
          <FailureState error={accounts.error} onRetry={() => void accounts.refetch()} />
        ) : (
          <NanoGpt
            key={accountId}
            controller={nano}
            accounts={nanoAccounts}
            accountId={accountId}
            onAccount={(usageAccount) => change({ usageAccount })}
          />
        )}
        {accounts.isError && tab !== "nanogpt" && (
          <FailureState error={accounts.error} onRetry={() => void accounts.refetch()} size="sm" />
        )}
        {search.usagePanel === "request" && (
          <RequestDetail id={search.usageRequest ?? ""} phone={phone} onClose={close} />
        )}
        {search.usagePanel === "clear" && <ClearRecords phone={phone} onClose={close} />}
      </Page>
    </CrispStringsProvider>
  );
}

export function UsagePage() {
  const api = useSettingsApi();
  const client = useQueryClient();
  const navigate = useNavigate();
  const [databaseVersion, setDatabaseVersion] = useState(0);
  const [subscriptionError, setSubscriptionError] = useState<unknown>(null);
  const [attempt, setAttempt] = useState(0);
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void api
      .subscribe((event) => {
        if (!disposed && event.type === "database_reloaded") {
          void client.cancelQueries({ queryKey: queryKeys.usage.all });
          client.removeQueries({ queryKey: queryKeys.usage.all });
          const jobsKey = [...queryKeys.jobs.lists(), "usage-prices"];
          void client.cancelQueries({ queryKey: jobsKey });
          client.removeQueries({ queryKey: jobsKey });
          setDatabaseVersion((value) => value + 1);
          void navigate({
            to: "/settings/$section",
            params: { section: "usage" },
            search: (previous) => {
              const next = { ...previous };
              delete next.usagePanel;
              delete next.usageRequest;
              return next;
            },
            replace: true,
            resetScroll: false,
          });
        }
      })
      .then((unsubscribe) => {
        if (disposed) unsubscribe();
        else stop = unsubscribe;
      })
      .catch((error: unknown) => {
        if (!disposed) setSubscriptionError(error);
      });
    return () => {
      disposed = true;
      stop?.();
    };
  }, [api, client, navigate, attempt]);
  return (
    <>
      {subscriptionError !== null && (
        <FailureState
          error={subscriptionError}
          onRetry={() => {
            setSubscriptionError(null);
            setAttempt((value) => value + 1);
          }}
          size="sm"
        />
      )}
      <UsageContent key={databaseVersion} />
    </>
  );
}
