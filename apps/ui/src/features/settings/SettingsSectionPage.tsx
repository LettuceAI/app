import { useParams } from "@tanstack/react-router";
import { ProvidersPage } from "./ProvidersPage";
import { ModelsPage } from "./ModelsPage";
import { AboutPage } from "./AboutPage";

/** The page of one settings section; the route only lets through sections that have one. */
export function SettingsSectionPage() {
  const { section } = useParams({ from: "/settings/$section" });
  if (section === "providers") return <ProvidersPage />;
  if (section === "models") return <ModelsPage />;
  return section === "about" ? <AboutPage /> : null;
}
