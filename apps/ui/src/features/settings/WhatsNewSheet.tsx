import { useEffect, useRef } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ArrowUpRight } from "lucide-react";
import { Alert, Button, icon, Sheet, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { appStatusQuery } from "@/api/app-status-query";
import { projectLinks } from "@/shared/links";
import updates from "@/shared/i18n/locales/en/updates.json";
import { useSettingsApi, useSettingsLayout } from "./data";
import { useExternalLink } from "./use-external-link";

const changeTypes = ["feature", "improvement", "bugfix", "breaking"] as const;
const changes = [
  { type: "feature", id: "contextUsage" },
  { type: "feature", id: "groupContextUsage" },
  { type: "improvement", id: "responseBudget" },
  { type: "improvement", id: "developerBreakdown" },
  { type: "bugfix", id: "memoryRetrieval" },
  { type: "bugfix", id: "macEmbeddings" },
] as const;

export function WhatsNewSheet() {
  const { t, i18n } = useTranslation("updates");
  const { t: common } = useTranslation();
  const api = useSettingsApi();
  const { phone } = useSettingsLayout();
  const status = useQuery(appStatusQuery(api));
  const queryClient = useQueryClient();
  const location = useLocation();
  const router = useRouter();
  const navigate = useNavigate();
  const external = useExternalLink();
  const open = location.search.settingsPanel === "whatsNew";
  const version = updates.release.version;
  const date = new Intl.DateTimeFormat(i18n.resolvedLanguage ?? "en", {
    year: "numeric",
    month: "long",
    day: "numeric",
    timeZone: "UTC",
  }).format(new Date(`${updates.release.date}T00:00:00Z`));
  const markSeen = useMutation({
    mutationFn: (seenVersion: string) =>
      api.call("appUiStateUpdate", { patch: { lastSeenAppVersion: seenVersion } }),
    onSuccess: () => {
      toast.dismiss("updates-mark-seen");
      return queryClient.invalidateQueries({ queryKey: appStatusQuery(api).queryKey });
    },
    onError(error, seenVersion): void {
      toast.error(
        t("markFailed"),
        common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`),
        {
          id: "updates-mark-seen",
          action: { label: common("actions.retry"), onClick: () => markSeen.mutate(seenVersion) },
        },
      );
    },
  });
  const viewed = useRef<{ open: boolean; version?: string }>({ open: false });
  const closingKey = useRef<string | undefined>(undefined);
  const { mutate: saveSeen } = markSeen;
  useEffect(() => {
    if (open) {
      if (!viewed.current.open) closingKey.current = undefined;
      viewed.current = { open: true, ...(status.data ? { version: status.data.version } : {}) };
    } else if (viewed.current.open) {
      if (viewed.current.version) saveSeen(viewed.current.version);
      viewed.current = { open: false };
    }
  }, [open, status.data, saveSeen]);
  const close = () => {
    if (
      !open ||
      router.state.location.state.key !== location.state.key ||
      closingKey.current === location.state.key
    )
      return;
    closingKey.current = location.state.key;
    if (location.state.settingsPanelPushed && router.history.canGoBack()) router.history.back();
    else
      void navigate({
        to: ".",
        search: (previous) => {
          const { settingsPanel: _, ...rest } = previous;
          return rest;
        },
        replace: true,
        resetScroll: false,
      });
  };
  return (
    <Sheet
      open={open}
      onClose={close}
      title={t("title")}
      description={status.data ? t("installed", { version: status.data.version }) : undefined}
      placement={phone ? "bottom" : "side"}
      {...(phone ? { detents: [1], defaultDetent: 0 } : {})}
      size="md"
      footer={
        <div className="flex items-center justify-between gap-3">
          <Button
            variant="ghost"
            disabled={external.isPending}
            trailing={<ArrowUpRight size={icon.md} />}
            onClick={() => external.mutate(projectLinks.changelog)}
          >
            {t("fullChangelog")}
          </Button>
          <Button onClick={close}>{t("continue")}</Button>
        </div>
      }
    >
      <div className="space-y-6">
        <header className="space-y-2">
          <p className="text-xs text-fg-3">{date}</p>
          <h2 className="text-xl font-semibold text-fg">{t("release.title")}</h2>
          <p className="text-sm leading-relaxed text-fg-2">{t("release.description")}</p>
        </header>
        {status.data && status.data.version !== version && (
          <Alert
            tone="info"
            title={t("releaseMismatch", { version, installed: status.data.version })}
          />
        )}
        {changeTypes.map((type) => {
          const items = changes.filter((change) => change.type === type);
          return (
            items.length > 0 && (
              <section key={type} className="space-y-3">
                <h3 className="text-2xs font-medium uppercase tracking-[0.18em] text-fg-3">
                  {t(`types.${type}`)}
                </h3>
                <ul className="list-disc space-y-3 pl-4 text-sm leading-relaxed text-fg-2">
                  {items.map((change) => (
                    <li key={change.id}>{t(`release.changes.${change.id}`)}</li>
                  ))}
                </ul>
              </section>
            )
          );
        })}
      </div>
    </Sheet>
  );
}
