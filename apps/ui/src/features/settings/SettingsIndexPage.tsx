import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { ListGroup, ListRowButton, Page } from "@lettuceai/crisp";
import { builtSettingsSections, settingsGroups } from "./sections";

/** Every settings section, in the groups the app files them under. */
export function SettingsIndexPage() {
  const { t } = useTranslation("settings");
  const navigate = useNavigate();
  return (
    <Page title={t("title")} contentClassName="space-y-6">
      {settingsGroups.map((group) => (
        <ListGroup key={group.id} label={t(`groups.${group.id}`)}>
          {group.sections.map((section) => {
            const built = builtSettingsSections.has(section.id);
            return (
              <ListRowButton
                key={section.id}
                icon={section.icon}
                tone={section.danger ? "danger" : "neutral"}
                title={t(`sections.${section.id}.title`)}
                description={built ? t(`sections.${section.id}.subtitle`) : t("notYet")}
                disabled={!built}
                onClick={() => void navigate({ to: "/settings/$section", params: { section: section.id } })}
              />
            );
          })}
        </ListGroup>
      ))}
    </Page>
  );
}
