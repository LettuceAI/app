import { useLocation, useNavigate } from "@tanstack/react-router";
import type { SettingsSection, SettingsSectionId } from "./sections";
import { useExternalLink } from "./use-external-link";

export function useSettingsNavigation(onOpen: (id: SettingsSectionId) => void) {
  const navigate = useNavigate();
  const location = useLocation();
  const external = useExternalLink();
  const openWhatsNew = () => {
    if (location.search.settingsPanel !== "whatsNew") {
      void navigate({
        to: ".",
        search: (previous) => ({ ...previous, settingsPanel: "whatsNew" }),
        state: (previous) => ({ ...previous, settingsPanelPushed: true }),
        resetScroll: false,
      });
    }
  };
  const openSetupGuide = () =>
    void navigate({ to: "/welcome", search: { setup: true }, state: { setupReplayPushed: true } });
  const open = (section: SettingsSection) => {
    if (section.externalUrl) {
      if (!external.isPending) external.mutate(section.externalUrl);
    } else if (section.id === "whatsNew") {
      openWhatsNew();
    } else if (section.id === "guide") {
      openSetupGuide();
    } else {
      onOpen(section.id);
    }
  };
  return { open, openWhatsNew, openSetupGuide, externalPending: external.isPending };
}

export function settingsSectionAvailable(section: SettingsSection, built: boolean): boolean {
  return built || !!section.externalUrl || section.id === "whatsNew" || section.id === "guide";
}
