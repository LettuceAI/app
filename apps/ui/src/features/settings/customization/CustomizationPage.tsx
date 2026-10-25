import { useCallback, useRef } from "react";
import { useLocation, useNavigate, useRouter, useSearch } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Eye } from "lucide-react";
import {
  CrispStringsProvider,
  icon,
  IconButton,
  ListGroup,
  ListRow,
  Page,
  Sheet,
  Skeleton,
  Tab,
  TabList,
  Tabs,
  Text,
} from "@lettuceai/crisp";
import type { SettingsUiChatAppearanceSettings_Deserialize } from "@/api/generated/bindings";
import {
  CHAT_DEFAULTS,
  detailFields,
  object,
  readChatLook,
  usePreferences,
  type ChatLook,
} from "@/entities/preferences";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsLayout } from "../data";
import { ChatPreview, ChatSettings } from "./ChatTab";
import { GeneralTab } from "./GeneralTab";
import { RenameTheme, ThemeTab } from "./ThemeTab";

export function CustomizationPage() {
  const { t, i18n } = useTranslation("customization");
  const { phone, back } = useSettingsLayout();
  const preferences = usePreferences();
  const navigate = useNavigate();
  const router = useRouter();
  const search = useSearch({ strict: false });
  const tab = search.custTab ?? "general";
  const panel = search.custPanel;
  const themeId = search.custTheme;
  const pushed = useLocation({ select: (location) => location.state.customizationPushed });
  const scroller = useRef<HTMLElement | null>(null);
  const look = readChatLook(object(preferences.value.chatAppearance));
  const close = () => {
    if (pushed) router.history.back();
    else
      void navigate({
        to: "/settings/$section",
        params: { section: "customization" },
        search: { custTab: tab },
        replace: true,
        resetScroll: false,
      });
  };
  const open = (custPanel: "preview" | "theme", custTheme?: string) =>
    void navigate({
      to: "/settings/$section",
      params: { section: "customization" },
      search: { custTab: tab, custPanel, ...(custTheme ? { custTheme } : {}) },
      state: { customizationPushed: true },
      resetScroll: false,
    });
  const set = useCallback(
    <K extends keyof ChatLook>(key: K, value: ChatLook[K]) => {
      void preferences.update((latest) => {
        const next: SettingsUiChatAppearanceSettings_Deserialize = {
          ...object(latest.chatAppearance),
        };
        if (key === "details") {
          for (const [detail, field] of Object.entries(detailFields))
            next[field] = (value as ChatLook["details"]).includes(
              detail as ChatLook["details"][number],
            );
        } else if (value === null) delete next[key as keyof typeof next];
        else Object.assign(next, { [key]: value });
        if (key === "userBubbleColor" && value !== "custom") delete next.userBubbleColorHex;
        if (key === "assistantBubbleColor" && value !== "custom")
          delete next.assistantBubbleColorHex;
        return [{ key: "chatAppearance", value: next }];
      }, `chat:${key}`);
    },
    [preferences],
  );
  const reset = () =>
    void preferences.update((latest) => {
      const next = { ...object(latest.chatAppearance) };
      for (const key of [...Object.keys(CHAT_DEFAULTS), ...Object.values(detailFields)])
        delete next[key];
      return [{ key: "chatAppearance", value: next }];
    });
  const changeTab = (next: string) => {
    void navigate({
      to: "/settings/$section",
      params: { section: "customization" },
      search: { custTab: next as typeof tab },
      replace: true,
      resetScroll: false,
    });
    scroller.current?.scrollTo({ top: 0 });
  };
  return (
    <CrispStringsProvider
      locale={i18n.language}
      strings={{
        colorPickerCurrent: (value) => t("controls.currentColor", { value }),
        colorPickerHex: t("controls.hex"),
        colorPickerPick: t("controls.pickColor"),
        numberInputDecrease: t("controls.decrease"),
        numberInputIncrease: t("controls.increase"),
        selectMenuClear: t("controls.clear"),
        selectMenuEmpty: t("controls.empty"),
        selectMenuPlaceholder: t("controls.select"),
        selectMenuSelected: (count) => t("controls.selected", { count }),
        sheetClose: t("controls.close"),
        composerMessage: t("controls.message"),
        composerSend: t("controls.send"),
        messageReasoning: t("controls.reasoning"),
        messageMenuTitle: t("controls.messageActions"),
      }}
    >
      <Page
        mainRef={scroller}
        title={t("title")}
        meta={phone ? undefined : t("meta")}
        {...back}
        actions={
          phone && tab === "chat" ? (
            <IconButton label={t("preview")} onClick={() => open("preview")}>
              <Eye size={icon.lg} />
            </IconButton>
          ) : undefined
        }
        filters={
          <Tabs value={tab} onChange={changeTab}>
            <TabList aria-label={t("sections")}>
              <Tab value="general">{t("general")}</Tab>
              <Tab value="theme">{t("theme")}</Tab>
              <Tab value="chat">{t("chat")}</Tab>
            </TabList>
          </Tabs>
        }
        contentClassName={tab === "chat" ? "max-w-6xl" : "max-w-3xl"}
      >
        {preferences.error !== null && (
          <FailureState error={preferences.error} onRetry={preferences.retry} size="sm" />
        )}
        {!preferences.ready ? (
          preferences.error === null && (
            <div className="space-y-7" role="status" aria-label={t("loading")}>
              {[1, 3, 1].map((count, index) => (
                <ListGroup key={index} label={<Skeleton className="h-4 w-24" />}>
                  {Array.from({ length: count }, (_, row) => (
                    <ListRow
                      key={row}
                      title={<Skeleton className="h-5 w-32" />}
                      description={<Skeleton className="h-4 w-48" />}
                      trailing={<Skeleton className="h-6 w-10" />}
                    />
                  ))}
                </ListGroup>
              ))}
            </div>
          )
        ) : tab === "general" ? (
          <GeneralTab phone={phone} />
        ) : tab === "theme" ? (
          <ThemeTab phone={phone} onRename={(id) => open("theme", id)} />
        ) : phone ? (
          <ChatSettings look={look} set={set} phone onReset={reset} />
        ) : (
          <div className="grid grid-cols-[minmax(0,1fr)_340px] items-start gap-8">
            <ChatSettings look={look} set={set} phone={false} onReset={reset} />
            <div className="sticky top-4 space-y-2">
              <Text variant="label" className="px-1">
                {t("preview")}
              </Text>
              <ChatPreview look={look} />
            </div>
          </div>
        )}
      </Page>
      <Sheet
        open={panel !== undefined}
        onClose={close}
        title={t(panel === "theme" ? "renameTheme" : "preview")}
        description={panel === "preview" ? t("previewDescription") : undefined}
        placement={phone ? "bottom" : "side"}
      >
        {panel === "theme" && themeId ? (
          <RenameTheme key={themeId} id={themeId} onDone={close} />
        ) : (
          <ChatPreview look={look} />
        )}
      </Sheet>
    </CrispStringsProvider>
  );
}
