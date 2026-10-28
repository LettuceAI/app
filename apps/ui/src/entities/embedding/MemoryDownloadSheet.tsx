import { useTranslation } from "react-i18next";
import { ModelDownloadSheet } from "@/entities/download";
import type { InstallState } from "./use-embedding-install";

export interface MemoryDownloadSheetProps {
  open: boolean;
  onClose: () => void;
  install: InstallState;
  onStart: () => void;
  onManual: () => void;
  mode?: "enable" | "install";
  done?: { label: string; onClick: () => void; loading: boolean; error: unknown };
}

export function MemoryDownloadSheet({ mode, onManual, ...props }: MemoryDownloadSheetProps) {
  const { t } = useTranslation("common");
  const installOnly = mode === "install";
  return (
    <ModelDownloadSheet
      {...props}
      copy={{
        name: t("memoryDownload.modelName"),
        title: t("memoryDownload.title"),
        titleDone: t("memoryDownload.titleDone"),
        body: t(installOnly ? "memoryDownload.bodyInstall" : "memoryDownload.body"),
        bodyDone: t("memoryDownload.bodyDone"),
        download: t(installOnly ? "memoryDownload.downloadOnly" : "memoryDownload.download"),
      }}
      secondary={installOnly ? { label: t("actions.cancel"), onClick: props.onClose } : { label: t("memoryDownload.useManual"), onClick: onManual }}
    />
  );
}
