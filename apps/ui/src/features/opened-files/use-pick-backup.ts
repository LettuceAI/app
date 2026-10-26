import { useState } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { isApiFailure } from "@/api/client";
import type { BackupHeaderView, FileSource } from "@/api/generated/bindings";

export type PickedBackup = { source: FileSource; header: BackupHeaderView };

const notBackup: ReadonlySet<string> = new Set(["unsupported", "malformed", "invalid_input"]);

export function usePickBackup(api: ApiClient, onPicked: (picked: PickedBackup) => void) {
  const { t } = useTranslation("opened-files");
  const { t: common } = useTranslation();
  const [picking, setPicking] = useState(false);

  const pick = async () => {
    if (picking) return;
    setPicking(true);
    try {
      const picked = await api.call("filesPickOpen", { kinds: ["backup"], multiple: false });
      const source = picked.sources[0];
      if (!source) return;
      const header = await api.call("backupInspect", { source });
      onPicked({ source, header });
    } catch (error) {
      toast.error(isApiFailure(error) && notBackup.has(error.code) ? t("restore.notBackup") : common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`));
    } finally {
      setPicking(false);
    }
  };

  return { picking, pick };
}
