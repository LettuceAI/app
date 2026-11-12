import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { useQuery } from "@tanstack/react-query";
import { Badge, ListGroup, ListRowButton, Page } from "@lettuceai/crisp";
import { builtSettingsSections, settingsGroups } from "./sections";
import { useDeveloperMode, useMobilePlatform, useSettingsApi, useSettingsLayout } from "./data";
import { openConflictCount, syncConflictsQuery } from "./sync/sync-data";
import { settingsSectionAvailable, settingsSectionVisible, useSettingsNavigation } from "./use-settings-navigation";
import { ArrowUpRight } from "lucide-react";
import { icon } from "@lettuceai/crisp";

/** Every settings section, in the groups the app files them under. */
export function SettingsIndexPage() {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const { standalone } = useSettingsLayout();
  const developerMode = useDeveloperMode();
  const mobile = useMobilePlatform();
  const conflictCount = openConflictCount(useQuery(syncConflictsQuery(useSettingsApi())).data);
  const navigate = useNavigate();
  const navigation = useSettingsNavigation(
    (id) => void navigate({ to: "/settings/$section", params: { section: id } }),
  );
  return (
    <Page
      title={t("title")}
      {...(standalone
        ? { onBack: () => void navigate({ to: "/" }), backLabel: common("navigation.backToApp") }
        : {})}
      contentClassName="space-y-6"
    >
      {settingsGroups.map((group) => (
        <ListGroup key={group.id} label={t(`groups.${group.id}`)}>
          {group.sections.filter((section) => settingsSectionVisible(section, developerMode, mobile)).map((section) => {
            const built = settingsSectionAvailable(section, builtSettingsSections.has(section.id));
            return (
              <ListRowButton
                key={section.id}
                icon={section.icon}
                tone={section.danger ? "danger" : "neutral"}
                title={t(`sections.${section.id}.title`)}
                description={built ? t(`sections.${section.id}.subtitle`) : t("notYet")}
                disabled={!built || (!!section.externalUrl && navigation.externalPending)}
                {...(section.externalUrl
                  ? {
                      chevron: false,
                      trailing: <ArrowUpRight size={icon.md} className="text-fg-4" />,
                    }
                  : section.id === "sync" && conflictCount > 0
                    ? { trailing: <Badge tone="warning">{conflictCount}</Badge> }
                    : {})}
                onClick={() => navigation.open(section)}
              />
            );
          })}
        </ListGroup>
      ))}
    </Page>
  );
}
