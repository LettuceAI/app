import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ListGroup, ListRowButton, Page, Spinner, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { writeTourFlags } from "@/entities/guided-tour";
import { useSettingsApi, useSettingsLayout } from "../data";
import { applyChapterTestSettings, hasChapterSettingsBackup, restoreChapterSettings } from "./chapterSettings";
import { chapterTemplateIds, runTemplate, templateIds, type TemplateId } from "./templates";

type SettingsAction = "apply" | "restore";

export function DeveloperPage() {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const { t: tour } = useTranslation("tour");
  const api = useSettingsApi();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const { back } = useSettingsLayout();
  const [running, setRunning] = useState<TemplateId | null>(null);
  const [resettingTours, setResettingTours] = useState(false);
  const [settingsAction, setSettingsAction] = useState<SettingsAction | null>(null);
  const [hasBackup, setHasBackup] = useState(hasChapterSettingsBackup);

  const fail = (error: unknown) => toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));

  const resetTours = async () => {
    setResettingTours(true);
    try {
      await writeTourFlags(api, queryClient, "reset");
      toast.success(tour("resetDone"));
    } catch (error) {
      fail(error);
    } finally {
      setResettingTours(false);
    }
  };

  const run = async (id: TemplateId) => {
    if (running) return;
    setRunning(id);
    try {
      const conversationId = await runTemplate(api, queryClient, id);
      await navigate({ to: "/chat/$conversationId", params: { conversationId } });
    } catch (error) {
      fail(error);
    } finally {
      setRunning(null);
    }
  };

  const changeSettings = async (action: SettingsAction) => {
    if (settingsAction) return;
    setSettingsAction(action);
    try {
      if (action === "apply") {
        await applyChapterTestSettings(api, queryClient);
        toast.success(t("developer.chapters.applied"));
      } else {
        await restoreChapterSettings(api, queryClient);
        toast.success(t("developer.chapters.restored"));
      }
    } catch (error) {
      fail(error);
    } finally {
      setHasBackup(hasChapterSettingsBackup());
      setSettingsAction(null);
    }
  };

  const templateRow = (id: TemplateId) => (
    <ListRowButton
      key={id}
      title={t(`developer.templates.${id}.title`)}
      description={t(`developer.templates.${id}.description`)}
      chevron={false}
      disabled={running !== null}
      aria-busy={running === id}
      trailing={running === id ? <Spinner size="sm" /> : undefined}
      onClick={() => void run(id)}
    />
  );

  const settingsRow = (action: SettingsAction) => (
    <ListRowButton
      key={action}
      title={t(`developer.chapters.${action}.title`)}
      description={t(`developer.chapters.${action}.description`)}
      chevron={false}
      disabled={settingsAction !== null}
      aria-busy={settingsAction === action}
      trailing={settingsAction === action ? <Spinner size="sm" /> : undefined}
      onClick={() => void changeSettings(action)}
    />
  );

  return (
    <Page title={t("sections.developer.title")} {...back} contentClassName="space-y-6">
      <ListGroup>
        <ListRowButton title={tour("reset")} description={tour("resetDescription")} chevron={false} disabled={resettingTours} onClick={() => void resetTours()} />
      </ListGroup>
      <ListGroup label={t("developer.group")}>{templateIds.map(templateRow)}</ListGroup>
      <ListGroup label={t("developer.chapters.group")}>
        {settingsRow("apply")}
        {hasBackup && settingsRow("restore")}
        {chapterTemplateIds.map(templateRow)}
      </ListGroup>
    </Page>
  );
}
