import { useQuery } from "@tanstack/react-query";
import { useParams, useRouteContext } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { object, useChatLook } from "@/entities/preferences";
import { metricsQuery, usageQuery } from "../data/queries";
import { statsOf } from "../data/messages";

export function ReplyDetails({ messageId }: { messageId: string }) {
  const api = useRouteContext({ from: "__root__", select: (context) => context.api });
  const { conversationId } = useParams({ strict: false });
  const look = useChatLook();
  const { t, i18n } = useTranslation("customization");
  const metric = useQuery({
    ...metricsQuery(api, conversationId ?? "", messageId),
    enabled: Boolean(conversationId) && look.details.length > 0,
  });
  const usage = useQuery({
    ...usageQuery(api, conversationId ?? "", messageId),
    enabled: Boolean(conversationId) && look.details.length > 0,
  });
  const stats = statsOf(metric.data?.summary, usage.data);
  if (!stats) return null;
  const format = new Intl.NumberFormat(i18n.language, { maximumFractionDigits: 1 });
  const summary = object(metric.data?.summary);
  const mtp = object(summary.mtpStats);
  const total =
    stats.promptTokens !== null && stats.completionTokens !== null
      ? stats.promptTokens + stats.completionTokens
      : (usage.data?.total_tokens ?? null);
  const values = {
    model: stats.model,
    input:
      stats.promptTokens === null
        ? null
        : t("metricInput", { value: format.format(stats.promptTokens) }),
    output:
      stats.completionTokens === null
        ? null
        : t("metricOutput", { value: format.format(stats.completionTokens) }),
    total: total === null ? null : t("metricTotal", { value: format.format(total) }),
    ttft:
      stats.firstTokenSeconds === null
        ? null
        : t("metricTtft", { value: format.format(stats.firstTokenSeconds) }),
    tps:
      stats.tokensPerSecond === null
        ? null
        : t("metricSpeed", { value: format.format(stats.tokensPerSecond) }),
    mtp:
      typeof mtp.draftAcceptance === "number"
        ? t("metricDraft", { value: format.format(mtp.draftAcceptance * 100) })
        : null,
  };
  return (
    <>
      {look.details.flatMap((key) =>
        values[key] === null ? [] : [<span key={key}>{values[key]}</span>],
      )}
    </>
  );
}
