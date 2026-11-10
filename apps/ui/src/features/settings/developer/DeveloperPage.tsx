import { useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ListGroup, ListRowButton, Page, Spinner, toast } from "@lettuceai/crisp";
import { isApiFailure } from "@/api/client";
import { useSettingsApi, useSettingsLayout } from "../data";
import { runTemplate, templateIds, type TemplateId } from "./templates";

export function DeveloperPage() {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const api = useSettingsApi();
  const queryClient = useQueryClient();
  const navigate = useNavigate();
  const { back } = useSettingsLayout();
  const [running, setRunning] = useState<TemplateId | null>(null);

  const run = async (id: TemplateId) => {
    if (running) return;
    setRunning(id);
    try {
      const conversationId = await runTemplate(api, queryClient, id);
      await navigate({ to: "/chat/$conversationId", params: { conversationId } });
    } catch (error) {
      toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setRunning(null);
    }
  };

  return (
    <Page title={t("sections.developer.title")} {...back}>
      <ListGroup label={t("developer.group")}>
        {templateIds.map((id) => (
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
        ))}
      </ListGroup>
    </Page>
  );
}
