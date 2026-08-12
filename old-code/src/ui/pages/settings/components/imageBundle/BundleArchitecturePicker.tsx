import { useNavigate } from "react-router-dom";
import { ExternalLink, ShieldCheck } from "lucide-react";

import { useI18n } from "../../../../../core/i18n/context";
import { Routes } from "../../../../navigation";
import type { ImageBundleController } from "./useImageBundle";

export function BundleArchitecturePicker({ bundle }: { bundle: ImageBundleController }) {
  const { t } = useI18n();
  const navigate = useNavigate();

  return (
    <div className="mx-auto w-full max-w-4xl px-4 py-6">
      <div className="mb-5">
        <div className="mb-1 flex items-center gap-2 text-[10px] font-semibold uppercase tracking-[0.18em] text-accent">
          <ShieldCheck size={12} /> {t("hfBrowser.bundle.verifiedInstaller")}
        </div>
        <h1 className="text-[22px] font-semibold tracking-tight text-fg">
          {t("hfBrowser.bundle.title")}
        </h1>
        <p className="mt-1 max-w-2xl text-[12.5px] leading-relaxed text-fg/50">
          {t("hfBrowser.bundle.subtitle")}
        </p>
      </div>

      {!bundle.runtime && (
        <button
          onClick={() => navigate(Routes.settingsImageGenerationLocal)}
          className="mb-4 flex w-full items-center justify-between rounded-xl border border-amber-400/15 bg-amber-400/5 px-3 py-2.5 text-left text-[11.5px] text-amber-200/80"
        >
          <span>{t("hfBrowser.bundle.runtimeRequired")}</span>
          <ExternalLink size={12} />
        </button>
      )}

      <h2 className="mb-3 text-[13px] font-semibold text-fg">
        {t("hfBrowser.bundle.chooseArchitecture")}
      </h2>
      <div className="grid gap-2 sm:grid-cols-2">
        {bundle.profiles.map((item, index) => (
          <button
            key={item.id}
            onClick={() => bundle.chooseProfile(item)}
            className="group relative overflow-hidden rounded-2xl border border-fg/9 bg-fg/[0.025] p-4 text-left transition hover:-translate-y-0.5 hover:border-accent/35 hover:bg-accent/[0.045]"
          >
            <span className="absolute right-3 top-2 font-mono text-[10px] text-fg/15">
              {String(index + 1).padStart(2, "0")}
            </span>
            <div className="pr-6 text-[14px] font-semibold text-fg">{item.displayName}</div>
            <div className="mt-1 text-[11px] font-medium uppercase tracking-[0.12em] text-accent/70">
              {item.family}
            </div>
            <p className="mt-3 text-[12px] leading-relaxed text-fg/50">{item.description}</p>
            <div className="mt-4 flex items-center gap-2 text-[10.5px] text-fg/40">
              {item.supportsTextToImage && <span>{t("hfBrowser.bundle.textToImage")}</span>}
              {item.supportsImageEdit && <span>• {t("hfBrowser.bundle.imageEditing")}</span>}
            </div>
          </button>
        ))}
      </div>
    </div>
  );
}
