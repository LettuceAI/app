import { useState } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, Field, Input, LoadingState, Select, Sheet, Text } from "@lettuceai/crisp";
import type { SettingsLlamaDefaultKvCacheType, SettingsView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsAction, useSettingsApi } from "./data";
import { LoraLibraryView } from "./image/LoraLibraryView";

export function ModelTools({
  open,
  tool,
  onClose,
}: {
  open: boolean;
  tool: "runtime" | "loras";
  onClose: () => void;
}) {
  const { t } = useTranslation("settings");
  return (
    <Sheet open={open} onClose={onClose} title={t(`models.${tool}`)}>
      {tool === "runtime" ? <RuntimeDefaults /> : <LoraLibraryView open={open} />}
    </Sheet>
  );
}

function RuntimeDefaults() {
  const api = useSettingsApi();
  const settings = useQuery({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
  });
  if (settings.isPending) return <LoadingState />;
  if (settings.isError)
    return <FailureState error={settings.error} onRetry={() => void settings.refetch()} />;
  return <RuntimeForm key={settings.data.revision} settings={settings.data} />;
}

function RuntimeForm({ settings }: { settings: SettingsView }) {
  const { t } = useTranslation("settings");
  const api = useSettingsApi();
  const action = useSettingsAction();
  const [context, setContext] = useState(
    String(settings.global.llama_default_context_length ?? ""),
  );
  const [kv, setKv] = useState<SettingsLlamaDefaultKvCacheType | "">(
    settings.global.llama_default_kv_cache_type ?? "",
  );
  const invalid = context !== "" && (!Number.isSafeInteger(Number(context)) || Number(context) < 1);
  const save = () =>
    action.mutate(() =>
      api.call("settingsUpdate", {
        expected_revision: settings.revision,
        patch: {
          section: "local_runtime",
          context_length: context === "" ? null : Number(context),
          kv_cache_type: kv || null,
        },
      }),
    );
  return (
    <div className="space-y-4">
      {action.isError && <FailureState error={action.error} size="sm" />}
      <Field label={t("tools.context")} hint={t("tools.defaultHint")}>
        <Input
          type="number"
          min={1}
          step={1}
          value={context}
          onChange={(e) => setContext(e.target.value)}
        />
      </Field>
      <Field label={t("tools.kv")}>
        <Select
          value={kv}
          options={[
            { value: "", label: t("tools.inherit") },
            ...(["auto", "f16", "q8_0", "q4_0"] as const).map((value) => ({
              value,
              label: value,
            })),
          ]}
          onChange={(e) => setKv(e.target.value as typeof kv)}
        />
      </Field>
      <Text variant="hint">{t("tools.runtimeHint")}</Text>
      <Button loading={action.isPending} disabled={invalid} onClick={save}>
        {t("manage.save")}
      </Button>
    </div>
  );
}
