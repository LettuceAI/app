import { useEffect } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import type { ApiClient } from "@/api/client";
import { refreshEmotionModel, useEmotionInstall } from "./emotion-install";
import { ModelDownloadSheet } from "./ModelDownloadSheet";

export function EmotionDownloadSheet({ api, open, onClose }: { api: ApiClient; open: boolean; onClose: () => void }) {
  const { t } = useTranslation("common");
  const { t: companions } = useTranslation("companions");
  const client = useQueryClient();
  const install = useEmotionInstall(api);
  const { reset } = install;
  const done = install.state.phase === "done";

  useEffect(() => {
    if (done) void refreshEmotionModel(client);
  }, [done, client]);
  useEffect(() => {
    if (!open) reset();
  }, [open, reset]);

  return (
    <ModelDownloadSheet
      open={open}
      onClose={onClose}
      install={install.state}
      onStart={() => void install.start()}
      copy={{
        name: companions("modelName"),
        title: t("emotionDownload.title"),
        titleDone: t("emotionDownload.titleDone"),
        body: t("emotionDownload.body"),
        bodyDone: t("emotionDownload.bodyDone"),
        download: t("emotionDownload.download"),
      }}
      secondary={{ label: t("emotionDownload.close"), onClick: onClose }}
      done={{ label: t("emotionDownload.finish"), onClick: onClose, loading: false, error: null }}
    />
  );
}
