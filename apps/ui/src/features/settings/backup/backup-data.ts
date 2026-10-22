import { useTranslation } from "react-i18next";
import type { ApiClient } from "@/api/client";
import { isApiFailure } from "@/api/client";
import type { JobView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

export const minPasswordLength = 6;

export const backupsQuery = (api: ApiClient) => ({
  queryKey: queryKeys.backups.list(),
  queryFn: async () => (await api.call("backupsList")).toSorted((a, b) => b.header.created_at - a.header.created_at),
});

export function useBackupFailureText() {
  const { t, i18n } = useTranslation("settings");
  const { t: common } = useTranslation();
  const ofJob = (job: JobView) => {
    const reason = job.failure?.reason;
    return reason && i18n.exists(`backup.failure.${reason}`, { ns: "settings" }) ? t(`backup.failure.${reason}` as never) : t("backup.failure.unknown");
  };
  const ofError = (error: unknown) => common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`);
  return { ofJob, ofError };
}
