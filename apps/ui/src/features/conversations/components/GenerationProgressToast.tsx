import { useEffect, useId, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { toast } from "@lettuceai/crisp";
import type { GenerationStore } from "../data/generation-store";

export function GenerationProgressToast({ store }: { store: GenerationStore }) {
  const { t } = useTranslation("conversations");
  const id = useId();
  const { loading, imagePrompt } = useSyncExternalStore(store.subscribeText, store.getText);

  useEffect(() => {
    if (loading) {
      toast.progress({
        id,
        title: t(loading.status === "retrying" ? "loading.toast.retrying" : "loading.toast.loading"),
        description: loading.model,
        progress: loading.percent / 100,
      });
    } else if (imagePrompt) {
      toast.progress({ id, title: t("streaming.imagePrompt") });
    } else {
      toast.dismiss(id);
    }
  }, [id, loading, imagePrompt, t]);

  useEffect(() => () => { toast.dismiss(id); }, [id]);

  return null;
}
