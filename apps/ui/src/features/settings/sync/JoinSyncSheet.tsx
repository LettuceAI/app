import { useEffect, useRef } from "react";
import { useQuery } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Sheet } from "@lettuceai/crisp";
import { useSettingsApi } from "../data";
import { SyncSession } from "./SyncSession";
import { syncStatusQuery } from "./sync-data";

/**
 * The join half of the sync session in a sheet, for setting up a fresh install from another device. `onCompleted`
 * runs once when a sync that began while the sheet was open finishes.
 */
export function JoinSyncSheet({ open, onClose, onCompleted }: { open: boolean; onClose: () => void; onCompleted: () => void }) {
  const { t } = useTranslation("onboarding");
  const api = useSettingsApi();
  const type = useQuery(syncStatusQuery(api)).data?.type;
  const seen = useRef(type);
  const completed = useRef(onCompleted);
  useEffect(() => {
    completed.current = onCompleted;
  });
  useEffect(() => {
    if (type === "completed" && seen.current !== "completed") completed.current();
    seen.current = type;
  }, [type]);
  return (
    <Sheet open={open} onClose={onClose} size="md" title={t("sync.title")} description={t("sync.body")} dismissible={type !== "connecting" && type !== "syncing"}>
      <div className="space-y-4 pb-1">
        <SyncSession joinOnly />
        <p className="text-sm text-fg-3">{t("sync.hint")}</p>
      </div>
    </Sheet>
  );
}
