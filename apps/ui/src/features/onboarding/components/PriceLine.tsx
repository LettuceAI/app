import { useTranslation } from "react-i18next";
import type { RemoteModelContract } from "@/api/generated/bindings";
import { formatPrice } from "../data/format";
import { priceOf } from "../data/models";

const separator = " · ";

export function PriceLine({ model }: { model: RemoteModelContract }) {
  const { t, i18n } = useTranslation("onboarding");
  const price = priceOf(model);
  if (!price) return null;
  return (
    <>
      {formatPrice(price.input, i18n.language)} <span className="text-fg-4">{t("model.priceIn")}</span>
      {separator}
      {formatPrice(price.output, i18n.language)} <span className="text-fg-4">{t("model.priceOut")}</span>
    </>
  );
}
