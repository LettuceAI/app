import { useRef } from "react";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { HeartPulse, Wand2 } from "lucide-react";
import {
  CrispStringsProvider,
  icon,
  ListGroup,
  ListRow,
  Page,
  Skeleton,
  Tab,
  TabList,
  Tabs,
} from "@lettuceai/crisp";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi, useSettingsLayout } from "../data";
import { useCompanionSettings } from "./use-companion-settings";
import { WriterTab } from "./WriterTab";
import { EmotionsTab } from "./EmotionsTab";
import type { CompanionPickerId } from "./CompanionPicker";

export function CompanionsPage() {
  const { t, i18n } = useTranslation("companions");
  const api = useSettingsApi();
  const { phone, back } = useSettingsLayout();
  const settings = useCompanionSettings(api);
  const search = useSearch({ from: "/settings/$section" });
  const tab = search.compTab ?? "writer";
  const picker = search.compPicker ?? null;
  const pushed = useLocation({ select: (location) => location.state.companionPickerPushed });
  const router = useRouter();
  const navigate = useNavigate();
  const main = useRef<HTMLElement>(null);
  const open = (compPicker: CompanionPickerId) =>
    void navigate({
      to: "/settings/$section",
      params: { section: "companions" },
      search: { compTab: tab, compPicker },
      state: { companionPickerPushed: true },
      resetScroll: false,
    });
  const close = () => {
    if (pushed) router.history.back();
    else
      void navigate({
        to: "/settings/$section",
        params: { section: "companions" },
        search: { compTab: tab },
        replace: true,
        resetScroll: false,
      });
  };
  const writer = settings.view?.global.companion_soul_writer;
  return (
    <CrispStringsProvider
      locale={i18n.language}
      strings={{ sheetClose: t("close"), searchFieldClear: t("clearSearch") }}
    >
      <Page
        mainRef={main}
        title={t("title")}
        meta={phone ? undefined : t("meta")}
        {...back}
        contentClassName="max-w-3xl space-y-7"
        filters={
          <Tabs
            value={tab}
            onChange={(next) => {
              void navigate({
                to: "/settings/$section",
                params: { section: "companions" },
                search: { compTab: next as "writer" | "emotions" },
                replace: true,
                resetScroll: false,
              });
              main.current?.scrollTo({ top: 0 });
            }}
          >
            <TabList aria-label={t("sections")}>
              <Tab value="writer" icon={phone ? undefined : <Wand2 size={icon.md} />}>
                {t("writerTab")}
              </Tab>
              <Tab value="emotions" icon={phone ? undefined : <HeartPulse size={icon.md} />}>
                {t("emotionsTab")}
              </Tab>
            </TabList>
          </Tabs>
        }
      >
        {settings.error !== null && (
          <FailureState error={settings.error} onRetry={settings.retry} size="sm" />
        )}
        {!writer ? (
          settings.error === null && (
            <div className="space-y-7" role="status" aria-label={t("loadingSettings")}>
              {[2, 2, 1].map((count, index) => (
                <ListGroup key={index} label={<Skeleton className="h-4 w-24" />}>
                  {Array.from({ length: count }, (_, index) => (
                    <ListRow
                      key={index}
                      title={<Skeleton className="h-5 w-36" />}
                      description={<Skeleton className="h-4 w-48 max-w-full" />}
                    />
                  ))}
                </ListGroup>
              ))}
            </div>
          )
        ) : tab === "writer" ? (
          <WriterTab
            value={writer}
            defaultModelId={settings.view?.default_model_profile_id ?? null}
            picker={picker}
            onOpen={open}
            onClose={close}
            onChange={settings.changeWriter}
          />
        ) : (
          <EmotionsTab
            key={settings.databaseVersion}
            keepLoaded={settings.view?.device.thymos_keep_model_loaded ?? false}
            onKeepLoaded={settings.setKeepLoaded}
          />
        )}
      </Page>
    </CrispStringsProvider>
  );
}
