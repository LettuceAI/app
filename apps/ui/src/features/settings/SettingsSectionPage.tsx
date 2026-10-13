import { useParams } from "@tanstack/react-router";
import { AboutPage } from "./AboutPage";

/** The page of one settings section; the route only lets through sections that have one. */
export function SettingsSectionPage() {
  const { section } = useParams({ from: "/settings/$section" });
  return section === "about" ? <AboutPage /> : null;
}
