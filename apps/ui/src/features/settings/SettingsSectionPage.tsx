import { useParams } from "@tanstack/react-router";
import { ProvidersPage } from "./ProvidersPage";
import { ModelsPage } from "./ModelsPage";
import { AboutPage } from "./AboutPage";
import { MemoryPage } from "./memory/MemoryPage";

/** The page of one settings section; the route only lets through sections that have one. */
export function SettingsSectionPage() {
  const { section } = useParams({ from: "/settings/$section" });
  if (section === "providers") return <ProvidersPage />;
  if (section === "models") return <ModelsPage />;
  if (section === "memory") return <MemoryPage />;
  return section === "about" ? <AboutPage /> : null;
}
