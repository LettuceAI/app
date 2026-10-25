import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import type { QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { invalidationsFor } from "@/api/query-keys";

export interface AppEventBridgeProps {
  api: ApiClient;
  queryClient: QueryClient;
  onDatabaseReload?: () => void;
}

/**
 * Subscribes once to the application event stream and invalidates the queries each event affects.
 * Once the subscription is live it invalidates every query, covering events that fired before it.
 */
export function AppEventBridge({ api, queryClient, onDatabaseReload }: AppEventBridgeProps) {
  const { t } = useTranslation("common");
  const [failure, setFailure] = useState<{ error: unknown } | null>(null);

  useEffect(() => {
    let active = true;
    let unsubscribe: (() => void) | undefined;
    api
      .subscribe((event) => {
        if (event.type === "database_reloaded") onDatabaseReload?.();
        if (event.type === "onnx_models_unloaded") {
          const names = { eidos: "Eidos", thymos: "Thymos", kokoro: "Kokoro" };
          toast.warning(
            t("onnxModelsUnloaded", {
              models: event.models.map((model) => names[model]).join(", "),
            }),
            undefined,
            { id: "onnx-memory-pressure" },
          );
        }
        for (const queryKey of invalidationsFor(event))
          void queryClient.invalidateQueries({ queryKey });
      })
      .then((stop) => {
        if (!active) {
          stop();
          return;
        }
        unsubscribe = stop;
        void queryClient.invalidateQueries();
      })
      .catch((error: unknown) => {
        if (active) setFailure({ error });
      });
    return () => {
      active = false;
      unsubscribe?.();
    };
  }, [api, queryClient, t, onDatabaseReload]);

  if (failure) throw failure.error;
  return null;
}
