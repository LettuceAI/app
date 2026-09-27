import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { createI18n } from "@/shared/i18n";
import { FatalErrorScreen } from "../error-boundaries/FatalErrorScreen";
import { ShellProviders } from "../providers/AppProviders";
import { App } from "./App";
import { createAppRuntime } from "./runtime";

/** Starts the app in `container`; a failure to reach the backend renders the fatal screen. */
export async function bootstrap(container: HTMLElement): Promise<void> {
  const root = createRoot(container);
  const i18n = await createI18n();
  try {
    const runtime = await createAppRuntime({ i18n });
    root.render(
      <StrictMode>
        <App runtime={runtime} />
      </StrictMode>,
    );
  } catch (error) {
    console.error("app bootstrap failed", error);
    root.render(
      <StrictMode>
        <ShellProviders i18n={i18n}>
          <FatalErrorScreen error={error} />
        </ShellProviders>
      </StrictMode>,
    );
  }
}
