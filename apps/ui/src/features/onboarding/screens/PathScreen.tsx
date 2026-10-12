import { ArrowRight, Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, cn, icon } from "@lettuceai/crisp";
import { ProviderMark } from "@/entities/provider";
import { guidedRoutes } from "../content/slides";
import { useOnboarding } from "../flow/context";
import { ScreenTitle } from "../components/ui";

export function PathScreen() {
  const { t } = useTranslation("onboarding");
  const { phone, dispatch } = useOnboarding();
  return (
    <>
      <ScreenTitle title={t("path.title")} lead={t("path.lead")} />
      <div className={cn("grid gap-x-12 gap-y-8", !phone && "grid-cols-2")}>
        {guidedRoutes.map((route) => (
          <div key={route} className="flex flex-col border-t border-line pt-5">
            <div className="flex items-center gap-3">
              <span className="flex h-10 w-10 items-center justify-center rounded-xl bg-fill-2">
                <ProviderMark kind={route} name={t(`path.${route}.subtitle`)} />
              </span>
              <div>
                <p className="text-2xl font-semibold text-fg">{t(`path.${route}.title`)}</p>
                <p className="text-sm text-fg-3">{t(`path.${route}.subtitle`)}</p>
              </div>
            </div>
            <ul className="mt-5 space-y-2 text-base text-fg-2">
              {(["pro1", "pro2", "pro3"] as const).map((point) => (
                <li key={point} className="flex gap-3">
                  <Check size={icon.md} className="mt-1 shrink-0 text-accent" />
                  {t(`path.${route}.${point}`)}
                </li>
              ))}
            </ul>
            <ul className="mt-3 space-y-2 text-base text-fg-3">
              {(["con1", "con2"] as const).map((point) => (
                <li key={point} className="flex gap-3">
                  <span className="mt-3 h-px w-4 shrink-0 bg-fg-4" />
                  {t(`path.${route}.${point}`)}
                </li>
              ))}
            </ul>
            <div className="flex-1" />
            <div className="mt-6">
              <Button
                variant={route === "gemini" ? "primary" : "secondary"}
                size="lg"
                block={phone}
                trailing={<ArrowRight size={icon.md} />}
                onClick={() => dispatch({ type: "chooseRoute", route })}
              >
                {t(`path.${route}.cta`)}
              </Button>
            </div>
          </div>
        ))}
      </div>
      <p className="mt-8 text-base text-fg-3">{t("path.footer")}</p>
    </>
  );
}
