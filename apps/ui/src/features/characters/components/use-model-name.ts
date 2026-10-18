import { useTranslation } from "react-i18next";
import type { ApiClient } from "@/api/client";
import { useChatModels } from "@/features/conversations";

export function useModelName(api: ApiClient, modelId: string | null): string {
  const { t } = useTranslation("characters");
  const chat = useChatModels(api);
  return modelId === null ? t("details.modelDefault") : (chat.find(modelId)?.name ?? (chat.models.length > 0 ? t("details.modelMissing") : ""));
}
