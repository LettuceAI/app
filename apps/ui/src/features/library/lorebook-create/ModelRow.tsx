import { useQuery } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { Globe } from "lucide-react";
import { icon, ListGroup, ListRowButton, Skeleton } from "@lettuceai/crisp";
import { queryKeys } from "@/api/query-keys";
import { useApi } from "@/api/use-api";
import { ModelMark } from "@/entities/provider";
import { FailureState } from "@/shared/ui/FailureState";

export function ModelRow() {
  const { t } = useTranslation("lorebook-create");
  const api = useApi();
  const navigate = useNavigate();
  const settings = useQuery({
    queryKey: queryKeys.settings.detail(),
    queryFn: () => api.call("settingsGet"),
  });
  const models = useQuery({
    queryKey: queryKeys.models.list(),
    queryFn: () => api.call("modelsList"),
  });
  const accounts = useQuery({
    queryKey: queryKeys.providers.accounts(),
    queryFn: () => api.call("providerAccountsList"),
  });
  const selected = settings.data?.global.lorebook_generator.selection.model_profile_id;
  const model = models.data?.models.find((item) => item.id === selected);
  const account = accounts.data?.find((item) => item.id === model?.provider_account_id);
  const pending = settings.isPending || models.isPending || accounts.isPending;
  return (
    <div className="space-y-2">
      <ListGroup label={t("model")}>
        <ListRowButton
          icon={
            model ? (
              <ModelMark
                name={model.display_name}
                modelId={model.external_model_id}
                providerKind={account?.provider_kind ?? ""}
              />
            ) : (
              <Globe size={icon.lg} />
            )
          }
          title={
            pending ? (
              <Skeleton className="h-5 w-40" />
            ) : model ? (
              `${account?.label ?? t("unknownProvider")} · ${model.display_name}`
            ) : (
              t(selected ? "missingModel" : "automaticModel")
            )
          }
          description={t("modelSettings")}
          onClick={() =>
            void navigate({
              to: "/settings/$section",
              params: { section: "lorebooks" },
              search: { loreTab: "lorebook" },
            })
          }
        />
      </ListGroup>
      {[settings, models, accounts].map(
        (query, index) =>
          query.isError && (
            <FailureState
              key={index}
              error={query.error}
              onRetry={() => void query.refetch()}
              size="sm"
            />
          ),
      )}
    </div>
  );
}
