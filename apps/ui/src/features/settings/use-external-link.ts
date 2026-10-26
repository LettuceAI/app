import { useMutation } from "@tanstack/react-query";
import { toast } from "@lettuceai/crisp";
import { useTranslation } from "react-i18next";
import { isApiFailure } from "@/api/client";
import { openExternalUrl } from "@/api/external-links";

export function useExternalLink() {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation();
  const mutation = useMutation({
    mutationFn: (url: string) => openExternalUrl(url),
    onSuccess: (_, url) => toast.dismiss(`external-link:${url}`),
    onError(error, url): void {
      toast.error(
        t("about.openLinkFailed"),
        common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`),
        {
          id: `external-link:${url}`,
          action: { label: common("actions.retry"), onClick: () => mutation.mutate(url) },
        },
      );
    },
  });
  return mutation;
}
