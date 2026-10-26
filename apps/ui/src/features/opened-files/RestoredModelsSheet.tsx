import { useEffect, useState, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { Download } from "lucide-react";
import { Button, icon, Sheet, Text } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { RequiredModel } from "@/api/generated/bindings";
import { DownloadCard } from "@/entities/download";
import { modelsNeededAfterMove, useModelInstalls } from "@/features/welcome-back";
import { restoredModelChecks, subscribeRestoredModels } from "./restored-models";

function Required({ api, models, onDone }: { api: ApiClient; models: RequiredModel[]; onDone: () => void }) {
  const { t } = useTranslation("opened-files");
  const { t: welcomeBack } = useTranslation("welcome-back");
  const installs = useModelInstalls(api, models);
  const done = installs.installs.every((entry) => entry.phase === "done");
  const failed = installs.installs.some((entry) => entry.phase === "failed");
  const started = installs.running || installs.installs.some((entry) => entry.phase !== "queued");
  const body = models.length > 1 ? "both" : models[0] === "embedding" ? "memory" : "emotion";

  return (
    <Sheet
      open
      dismissible={false}
      onClose={() => { if (done) onDone(); }}
      size="sm"
      title={t("restore.models.title")}
      footer={
        done
          ? <Button variant="primary" block onClick={onDone}>{welcomeBack("models.continue")}</Button>
          : <Button variant="primary" block loading={installs.running} leading={<Download size={icon.md} />} onClick={() => void installs.start()}>{failed ? welcomeBack("models.retry") : welcomeBack("models.download")}</Button>
      }
    >
      <div className="space-y-4 pb-1">
        <Text variant="body">{welcomeBack(`models.bodyPending_${body}`)}</Text>
        <Text variant="hint">{t("restore.models.required")}</Text>
        <div className="space-y-3">
          {installs.installs.map((entry) => (
            <DownloadCard
              key={entry.model}
              job={entry.job}
              name={welcomeBack(`models.names.${entry.model}`)}
              {...(!started ? { detail: welcomeBack(`models.why.${entry.model}`) } : {})}
              {...(entry.job === null ? { status: entry.phase === "failed" ? ("failed" as const) : ("queued" as const) } : {})}
            />
          ))}
        </div>
        {installs.running && <Text variant="hint">{welcomeBack("models.keepOpen")}</Text>}
      </div>
    </Sheet>
  );
}

/** After a restore, holds the screen until the models the restored data needs are downloaded. */
export function RestoredModelsSheet({ api }: { api: ApiClient }) {
  const requested = useSyncExternalStore(subscribeRestoredModels, restoredModelChecks);
  const [models, setModels] = useState<RequiredModel[]>([]);

  useEffect(() => {
    if (requested === 0) return;
    let active = true;
    modelsNeededAfterMove(api).then(
      (needed) => { if (active) setModels(needed); },
      () => undefined,
    );
    return () => { active = false; };
  }, [api, requested]);

  if (models.length === 0) return null;
  return <Required key={requested} api={api} models={models} onDone={() => setModels([])} />;
}
