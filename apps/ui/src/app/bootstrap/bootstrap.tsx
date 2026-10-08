import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import type { i18n } from "i18next";
import { createI18n } from "@/shared/i18n";
import { FatalErrorScreen } from "../error-boundaries/FatalErrorScreen";
import { ShellProviders } from "../providers/AppProviders";
import { App } from "./App";
import { createAppRuntime } from "./runtime";

const lastResortMessage = "LettuceAI could not start. Restart the app; if this keeps happening, reinstall it.";

/** Used only when localization itself failed, so the text cannot come from a locale file. */
export function renderLastResort(container: HTMLElement, error: unknown): void {
  console.error("app bootstrap failed before localization", error);
  container.replaceChildren();
  container.textContent = lastResortMessage;
}

/** Starts the app in `container`; a failure to reach the backend renders the fatal screen. */
export async function bootstrap(container: HTMLElement): Promise<void> {
  let localization: i18n;
  try {
    localization = await createI18n();
  } catch (error) {
    renderLastResort(container, error);
    return;
  }
  const root = createRoot(container);
  try {
    const runtime = await createAppRuntime();
    root.render(
      <StrictMode>
        <App runtime={runtime} />
      </StrictMode>,
    );
  } catch (error) {
    console.error("app bootstrap failed", error);
    root.render(
      <StrictMode>
        <ShellProviders i18n={localization}>
          <FatalErrorScreen error={error} />
        </ShellProviders>
      </StrictMode>,
    );
  }
}
