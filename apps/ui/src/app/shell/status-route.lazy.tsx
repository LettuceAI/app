import { createLazyRoute } from "@tanstack/react-router";
import { StatusPage } from "./StatusPage";

export const Route = createLazyRoute("/")({
  component: StatusPage,
});
