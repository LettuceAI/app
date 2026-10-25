import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import {
  Alert,
  Badge,
  Button,
  EmptyState,
  Field,
  icon,
  ListGroup,
  ListRow,
  Meter,
  SelectMenu,
  Skeleton,
  Text,
} from "@lettuceai/crisp";
import type { ProviderAccountView } from "@/api/generated/bindings";
import { ActionError } from "@/shared/ui/ActionError";
import { FailureState } from "@/shared/ui/FailureState";
import type { useNano } from "./use-nano";
import { number, quotaDate, timestamp } from "./format";

export function NanoGpt({
  accounts,
  accountId,
  onAccount,
  controller,
}: {
  accounts: ProviderAccountView[];
  accountId: string;
  onAccount: (id: string) => void;
  controller: ReturnType<typeof useNano>;
}) {
  const { t, i18n } = useTranslation("usage");
  const navigate = useNavigate();
  const account = accounts.find((item) => item.id === accountId);
  const { query, busy, error, refresh } = controller;
  const openProviders = () =>
    void navigate({ to: "/settings/$section", params: { section: "providers" } });
  const data = query.data;
  return (
    <div className="space-y-7">
      {accounts.length > 1 && (
        <Field label={t("nano.account")}>
          <SelectMenu
            className="w-full"
            value={accountId}
            onChange={(value) => value && onAccount(value)}
            options={accounts.map((item) => ({ value: item.id, label: item.label }))}
          />
        </Field>
      )}
      {!account ? (
        <EmptyState
          title={t("nano.noAccount")}
          action={<Button onClick={openProviders}>{t("nano.openProviders")}</Button>}
        />
      ) : !account.api_key_set || !account.enabled ? (
        <Alert
          tone="warning"
          title={t(account.enabled ? "nano.noKey" : "nano.disabled")}
          description={t("nano.configure")}
          action={
            <Button size="sm" variant="secondary" onClick={openProviders}>
              {t("nano.openProviders")}
            </Button>
          }
        />
      ) : (
        <>
          {query.isError ? (
            <FailureState error={query.error} onRetry={() => void query.refetch()} />
          ) : !data ? (
            <div role="status" aria-label={t("nano.loading")} className="space-y-5">
              <Skeleton className="h-16 w-full" />
              <Skeleton className="h-64 w-full" />
            </div>
          ) : (
            <>
              <ListGroup>
                <ListRow
                  title={account.label}
                  description={[
                    data.state,
                    quotaDate(data.current_period_end, i18n.language)
                      ? t("nano.periodEnd", {
                          value: quotaDate(data.current_period_end, i18n.language),
                        })
                      : null,
                    quotaDate(data.grace_until, i18n.language)
                      ? t("nano.graceEnd", { value: quotaDate(data.grace_until, i18n.language) })
                      : null,
                  ]
                    .filter(Boolean)
                    .join(" · ")}
                  trailing={
                    <Badge tone={data.active === true ? "accent" : "neutral"}>
                      {t(
                        data.active === true
                          ? "nano.active"
                          : data.active === false
                            ? "nano.inactive"
                            : "notReported",
                      )}
                    </Badge>
                  }
                />
              </ListGroup>
              {data.daily || data.weekly || data.monthly ? (
                <ListGroup label={t("nano.planLimits")}>
                  {(["daily", "weekly", "monthly"] as const).flatMap((key) => {
                    const window = data[key];
                    if (!window) return [];
                    const ratio =
                      window.used !== null && window.limit !== null && window.limit > 0
                        ? window.used / window.limit
                        : window.percent_used;
                    const unit = window.unit ?? t("nano.units");
                    const value =
                      window.used !== null && window.limit !== null
                        ? t("nano.usedOf", {
                            used: number(window.used, i18n.language),
                            limit: number(window.limit, i18n.language),
                            unit,
                          })
                        : window.used !== null
                          ? t("nano.used", { value: number(window.used, i18n.language), unit })
                          : t("notReported");
                    const reset = quotaDate(window.reset_at, i18n.language);
                    return (
                      <div key={key} className="space-y-2 px-4 py-3">
                        {ratio !== null ? (
                          <Meter
                            value={Math.max(0, Math.min(1, ratio))}
                            max={1}
                            label={t(`nano.windows.${key}`)}
                            valueLabel={value}
                          />
                        ) : (
                          <div className="flex flex-wrap justify-between gap-2 text-sm">
                            <span className="text-fg">{t(`nano.windows.${key}`)}</span>
                            <span className="text-fg-2">{value}</span>
                          </div>
                        )}
                        <Text variant="hint">
                          {[
                            reset ? t("nano.resets", { value: reset }) : t("nano.resetUnknown"),
                            window.remaining !== null
                              ? t("nano.remaining", {
                                  value: number(window.remaining, i18n.language),
                                  unit,
                                })
                              : null,
                          ]
                            .filter(Boolean)
                            .join(" · ")}
                        </Text>
                      </div>
                    );
                  })}
                </ListGroup>
              ) : (
                <EmptyState title={t("nano.noLimits")} />
              )}
              <div className="flex flex-wrap items-center justify-between gap-3 px-1">
                <Text variant="hint">
                  {t("nano.fetched", { value: timestamp(data.fetched_at, i18n.language) })}
                </Text>
                <Button
                  size="sm"
                  variant="ghost"
                  loading={busy}
                  leading={<RefreshCw size={icon.sm} />}
                  onClick={() => void refresh()}
                >
                  {t("refresh")}
                </Button>
              </div>
            </>
          )}
          <ActionError error={error} busy={busy} onRetry={() => void refresh()} />
          <Text variant="hint" className="px-1">
            {t("nano.hint")}
          </Text>
        </>
      )}
    </div>
  );
}
