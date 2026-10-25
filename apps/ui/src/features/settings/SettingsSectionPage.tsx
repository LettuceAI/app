import { useParams } from "@tanstack/react-router";
import { CompanionsPage } from "./companions/CompanionsPage";
import { SpeechPage } from "./speech/SpeechPage";
import { ProvidersPage } from "./ProvidersPage";
import { ModelsPage } from "./ModelsPage";
import { AboutPage } from "./AboutPage";
import { MemoryPage } from "./memory/MemoryPage";
import { ImageSettingsPage } from "./image/ImageSettingsPage";
import { ReplyHelperPage } from "./reply-helper/ReplyHelperPage";
import { PromptsPage } from "./prompts/PromptsPage";
import { BackupPage } from "./backup/BackupPage";
import { CustomizationPage } from "./customization/CustomizationPage";

/** The page of one settings section; the route only lets through sections that have one. */
export function SettingsSectionPage() {
  const { section } = useParams({ from: "/settings/$section" });
  if (section === "companions") return <CompanionsPage />;
  if (section === "customization") return <CustomizationPage />;
  if (section === "speech") return <SpeechPage />;
  if (section === "providers") return <ProvidersPage />;
  if (section === "models") return <ModelsPage />;
  if (section === "memory") return <MemoryPage />;
  if (section === "imageGeneration") return <ImageSettingsPage />;
  if (section === "replyHelper") return <ReplyHelperPage />;
  if (section === "prompts") return <PromptsPage />;
  if (section === "backup") return <BackupPage />;
  return section === "about" ? <AboutPage /> : null;
}
