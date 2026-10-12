import { ArrowRight } from "lucide-react";
import { Trans, useTranslation } from "react-i18next";
import { Button, cn, icon } from "@lettuceai/crisp";
import { lettuce } from "../assets";
import { useOnboarding } from "../flow/context";

export function WelcomeScreen({ onOtherOptions }: { onOtherOptions: () => void }) {
  const { t } = useTranslation("onboarding");
  const { t: common } = useTranslation("common");
  const { phone, dispatch } = useOnboarding();
  const brand = { brand: <span className="text-brand" /> };
  return (
    <div className={cn("flex h-full flex-col justify-end", !phone && "max-w-2xl")}>
      <span className="mb-6 flex items-center gap-2.5">
        <img src={lettuce} alt="" className="h-8 w-8" />
        <span className="text-lg font-semibold tracking-tight text-fg">{common("app.name")}</span>
      </span>
      <h1 className={cn("font-bold tracking-[-0.03em] text-fg", phone ? "text-[2.5rem] leading-[1.08]" : "text-7xl leading-[1.02]")}>
        <span className="block whitespace-nowrap">
          <Trans t={t} i18nKey="welcome.line1" components={brand} />
        </span>
        <span className="block whitespace-nowrap">
          <Trans t={t} i18nKey="welcome.line2" components={brand} />
        </span>
      </h1>
      <p className="mt-5 max-w-md text-lg leading-relaxed text-fg-2">{t("welcome.pitch")}</p>
      <div className={cn("mt-9 flex items-center gap-4", phone && "flex-col items-stretch gap-3")}>
        <Button
          variant="primary"
          size="lg"
          block={phone}
          trailing={<ArrowRight size={icon.md} />}
          onClick={() => dispatch({ type: "go", step: "connect" })}
        >
          {t("welcome.start")}
        </Button>
        <Button variant="ghost" size="lg" block={phone} onClick={onOtherOptions}>
          {t("welcome.other")}
        </Button>
      </div>
    </div>
  );
}
