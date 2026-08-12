import {
  AlertTriangle,
  Check,
  Cpu,
  Download,
  ExternalLink,
  Loader,
  MemoryStick,
  RefreshCw,
  Sparkles,
  X,
} from "lucide-react";

import { useI18n } from "../../../../../core/i18n/context";
import { openExternalUrl } from "../../../../../core/utils/openExternal";
import { cn } from "../../../../design-tokens";
import { InlineDownloadCards } from "../DownloadQueueBar";
import { formatBundleBytes, type BundleRole } from "./types";
import type { ImageBundleController } from "./useImageBundle";

export function BundleReview({
  bundle,
  onOpenModel,
  onRedoRole,
}: {
  bundle: ImageBundleController;
  onOpenModel: (modelId: string) => void;
  onRedoRole: (role: BundleRole) => void;
}) {
  const { t } = useI18n();
  const { estimate, bundleStatus } = bundle;
  const blocked =
    !bundle.runtime ||
    estimate?.status === "notInstalled" ||
    estimate?.status === "incompatibleRuntime";

  const planLabel = (planMode: string) => {
    if (planMode === "concurrent") return t("hfBrowser.bundle.planConcurrent");
    if (planMode === "timeShare") return t("hfBrowser.bundle.planTimeShare");
    return t("hfBrowser.bundle.planDefaultBackend");
  };

  const componentLabel = (component: string) => {
    if (component === "DiT") return t("hfBrowser.bundle.componentDit");
    if (component === "VAE") return t("hfBrowser.bundle.componentVae");
    if (component === "Conditioner") return t("hfBrowser.bundle.componentConditioner");
    return component;
  };

  if (bundleStatus?.registrationState === "registered") {
    return (
      <div className="mx-auto flex max-w-xl flex-col items-center px-6 py-16 text-center">
        <div className="mb-5 flex h-16 w-16 items-center justify-center rounded-[1.4rem] border border-emerald-400/30 bg-emerald-400/10 text-emerald-300 shadow-[0_0_50px_rgba(52,211,153,0.12)]">
          <Sparkles size={27} />
        </div>
        <h2 className="text-xl font-semibold text-fg">{t("hfBrowser.bundle.completeTitle")}</h2>
        <p className="mt-2 max-w-md text-[13px] leading-relaxed text-fg/55">
          {t("hfBrowser.bundle.completeBody", { name: bundle.draft.displayName })}
        </p>
        <button
          onClick={() => bundleStatus.modelId && onOpenModel(bundleStatus.modelId)}
          className="mt-6 rounded-xl bg-accent px-5 py-2.5 text-[13px] font-semibold text-white transition hover:brightness-110 active:scale-[0.98]"
        >
          {t("hfBrowser.bundle.openModel")}
        </button>
      </div>
    );
  }

  return (
    <div className="mx-auto w-full max-w-5xl px-4 py-5">
      {bundle.draft.bundleId && (
        <div className="mb-4 space-y-2">
          <InlineDownloadCards
            filter={(item) => item.installId === bundle.draft.bundleId}
          />
          {((bundle.downloadFailed && bundle.downloadsSettled) ||
            bundle.downloadsInterrupted) && (
            <button
              onClick={() => void bundle.retryDownloads()}
              disabled={bundle.busy}
              className="flex items-center gap-2 rounded-lg border border-amber-400/20 px-3 py-2 text-[11.5px] font-semibold text-amber-200"
            >
              <RefreshCw size={12} />
              {t("hfBrowser.bundle.retryDownloads")}
            </button>
          )}
        </div>
      )}
      <div className="flex items-start justify-between gap-3">
        <div className="min-w-0">
          <h1 className="text-[18px] font-semibold tracking-tight text-fg">
            {t("hfBrowser.bundle.reviewTitle")}
          </h1>
          {bundle.profile && (
            <p className="mt-0.5 text-[11.5px] text-fg/45">
              {bundle.profile.displayName} · {bundle.profile.defaultWidth}×
              {bundle.profile.defaultHeight} · {bundle.profile.defaultSteps}{" "}
              {t("hfBrowser.bundle.steps")} · CFG {bundle.profile.defaultCfg}
            </p>
          )}
        </div>
        <button
          onClick={bundle.resetProfile}
          className="shrink-0 text-[11px] text-fg/45 transition hover:text-fg"
        >
          {t("hfBrowser.bundle.changeArchitecture")}
        </button>
      </div>

      <div className="mt-4 grid items-start gap-4 lg:grid-cols-2">
      <div className="min-w-0 space-y-4">
      <div className="overflow-hidden rounded-2xl border border-fg/9 bg-fg/[0.025]">
        <div className="divide-y divide-fg/7">
          {bundle.requiredRoles.map((role) => {
            const asset = bundle.draft.selections[role];
            if (!asset) return null;
            return (
              <div key={role} className="flex items-start gap-3 px-4 py-3">
                <span className="mt-0.5 flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-emerald-400/25 bg-emerald-400/10 text-emerald-300">
                  <Check size={11} />
                </span>
                <div className="min-w-0 flex-1">
                  <div className="flex items-center gap-2">
                    <span className="text-[10.5px] font-medium text-fg/45">
                      {bundle.roleLabel(role)}
                    </span>
                    <button
                      onClick={() =>
                        void openExternalUrl(`https://huggingface.co/${asset.modelId}`)
                      }
                      className="flex min-w-0 items-center gap-1 text-[10.5px] text-fg/55 transition hover:text-accent"
                    >
                      <span className="truncate font-mono">{asset.modelId}</span>
                      <ExternalLink size={9} className="shrink-0" />
                    </button>
                  </div>
                  <div className="mt-1 truncate font-mono text-[11.5px] text-fg">
                    {asset.relativePath.split("/").pop()}
                  </div>
                  <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
                    <span className="rounded-md border border-accent/20 bg-accent/10 px-1.5 py-0.5 text-[9px] font-semibold text-accent/80">
                      {asset.format.toUpperCase()}
                    </span>
                    {asset.quantization && (
                      <span className="rounded-md border border-fg/12 bg-fg/5 px-1.5 py-0.5 text-[9px] font-semibold text-fg/60">
                        {asset.quantization}
                      </span>
                    )}
                    {asset.gated && (
                      <span className="rounded-md bg-amber-400/10 px-1.5 py-0.5 text-[9px] font-semibold text-amber-300">
                        {t("hfBrowser.bundle.gated")}
                      </span>
                    )}
                    <span className="text-[10.5px] text-fg/45">
                      {formatBundleBytes(asset.size)}
                    </span>
                    <span className="font-mono text-[9.5px] text-fg/30">
                      {asset.revision.slice(0, 8)}
                    </span>
                  </div>
                </div>
                {!bundle.draft.bundleId && (
                  <button
                    onClick={() => onRedoRole(role)}
                    aria-label={bundle.roleLabel(role)}
                    className="shrink-0 rounded-lg p-1.5 text-fg/30 transition hover:bg-fg/8 hover:text-fg"
                  >
                    <X size={12} />
                  </button>
                )}
              </div>
            );
          })}
        </div>
      </div>

      <div className="rounded-2xl border border-fg/9 bg-fg/[0.025] p-5">
        <label className="text-[11px] font-semibold uppercase tracking-[0.13em] text-fg/45">
          {t("hfBrowser.bundle.displayName")}
        </label>
        <input
          value={bundle.draft.displayName}
          onChange={(event) => bundle.setDisplayName(event.target.value)}
          disabled={!!bundle.draft.bundleId}
          className="mt-2 h-11 w-full rounded-xl border border-fg/10 bg-surface px-3 text-[14px] text-fg outline-none transition focus:border-accent/40 disabled:opacity-60"
        />
      </div>
      </div>

      <div className="min-w-0 space-y-4">
      <div
        className={cn(
          "rounded-2xl border p-5",
          blocked
            ? "border-red-400/20 bg-red-400/5"
            : estimate?.status === "cpuFallback"
              ? "border-amber-400/20 bg-amber-400/5"
              : "border-emerald-400/20 bg-emerald-400/5",
        )}
      >
        <div className="flex items-start gap-3">
          {bundle.estimateBusy ? (
            <Loader size={18} className="mt-0.5 animate-spin text-accent" />
          ) : blocked || estimate?.status === "inconclusive" ? (
            <AlertTriangle size={18} className="mt-0.5 text-amber-300" />
          ) : (
            <Check size={18} className="mt-0.5 text-emerald-300" />
          )}
          <div className="min-w-0 flex-1">
            <h3 className="text-[13px] font-semibold text-fg">
              {t("hfBrowser.bundle.preDownloadEstimate")}
            </h3>
            <p className="mt-1 text-[12px] leading-relaxed text-fg/55">
              {!bundle.runtime
                ? t("hfBrowser.bundle.runtimeRequired")
                : bundle.estimateBusy
                  ? t("hfBrowser.bundle.calculating")
                  : estimate?.reason}
            </p>

            {estimate?.estimate && (
              <>
                <div className="mt-3 grid gap-2 sm:grid-cols-3">
                  <div className="rounded-lg bg-black/10 px-3 py-2">
                    <div className="text-[9px] uppercase tracking-wider text-fg/35">
                      {t("hfBrowser.bundle.bundleSize")}
                    </div>
                    <div className="mt-1 text-[12px] font-medium text-fg">
                      {formatBundleBytes(estimate.estimate.modelBytes)}
                    </div>
                  </div>
                  <div className="rounded-lg bg-black/10 px-3 py-2">
                    <div className="text-[9px] uppercase tracking-wider text-fg/35">
                      {t("hfBrowser.bundle.placement")}
                    </div>
                    <div className="mt-1 text-[12px] font-medium text-fg">
                      {planLabel(estimate.estimate.planMode)}
                    </div>
                  </div>
                  <div className="rounded-lg bg-black/10 px-3 py-2">
                    <div className="text-[9px] uppercase tracking-wider text-fg/35">
                      {t("hfBrowser.bundle.devices")}
                    </div>
                    <div className="mt-1 truncate text-[12px] font-medium text-fg">
                      {estimate.estimate.devices.map((device) => device.name).join(", ") ||
                        "CPU"}
                    </div>
                  </div>
                </div>

                {estimate.estimate.placements.length > 0 && (
                  <div className="mt-3">
                    <div className="mb-1.5 text-[9px] font-semibold uppercase tracking-wider text-fg/35">
                      {t("hfBrowser.bundle.placementHeading")}
                    </div>
                    <div className="divide-y divide-fg/6 overflow-hidden rounded-lg bg-black/10">
                      {estimate.estimate.placements.map((placement) => (
                        <div
                          key={placement.component}
                          className="flex flex-wrap items-center gap-x-3 gap-y-1 px-3 py-2"
                        >
                          <span className="w-28 shrink-0 text-[11px] font-medium text-fg/80">
                            {componentLabel(placement.component)}
                          </span>
                          <span className="text-[10.5px] tabular-nums text-fg/55">
                            {formatBundleBytes(placement.paramsBytes)}
                            {placement.computeReserveBytes > 0 && (
                              <span className="text-fg/35">
                                {" "}
                                +{formatBundleBytes(placement.computeReserveBytes)}{" "}
                                {t("hfBrowser.bundle.reserve")}
                              </span>
                            )}
                          </span>
                          <span className="min-w-0 flex-1 truncate text-right font-mono text-[10.5px] text-fg/60">
                            {placement.cpu ? "CPU" : placement.targets.join(", ")}
                          </span>
                          {placement.split && (
                            <span className="rounded-md border border-blue-400/20 bg-blue-400/10 px-1.5 py-0.5 text-[9px] font-semibold text-blue-300">
                              {t("hfBrowser.bundle.splitAcross")}
                            </span>
                          )}
                          {placement.cpu && (
                            <span className="rounded-md border border-amber-400/20 bg-amber-400/10 px-1.5 py-0.5 text-[9px] font-semibold text-amber-300">
                              CPU
                            </span>
                          )}
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {estimate.estimate.devices.length > 0 && (
                  <div className="mt-3">
                    <div className="mb-1.5 text-[9px] font-semibold uppercase tracking-wider text-fg/35">
                      {t("hfBrowser.bundle.devices")}
                    </div>
                    <div className="divide-y divide-fg/6 overflow-hidden rounded-lg bg-black/10">
                      {estimate.estimate.devices.map((device) => (
                        <div
                          key={device.id}
                          className="flex flex-wrap items-center gap-x-3 gap-y-1 px-3 py-2"
                        >
                          <Cpu size={12} className="shrink-0 text-fg/40" />
                          <span className="min-w-0 flex-1 truncate text-[11px] text-fg/80">
                            <span className="font-medium">{device.name}</span>
                            {device.description && (
                              <span className="text-fg/45"> · {device.description}</span>
                            )}
                          </span>
                          <span className="shrink-0 text-[10.5px] tabular-nums text-fg/55">
                            {t("hfBrowser.bundle.deviceUsable", {
                              free: formatBundleBytes(device.freeBytes),
                              budget: formatBundleBytes(device.budgetBytes),
                              total: formatBundleBytes(device.totalBytes),
                            })}
                          </span>
                        </div>
                      ))}
                    </div>
                  </div>
                )}

                {estimate.estimate.availableRamBytes != null && (
                  <div className="mt-2.5 flex items-center gap-1.5 text-[10.5px] text-fg/45">
                    <MemoryStick size={11} />
                    {t("hfBrowser.bundle.availableRam", {
                      value: formatBundleBytes(estimate.estimate.availableRamBytes),
                    })}
                  </div>
                )}
              </>
            )}
          </div>
        </div>
      </div>

      {bundleStatus?.registrationState === "setupFailed" ? (
        <div className="rounded-2xl border border-red-400/20 bg-red-400/5 p-4">
          <div className="text-[13px] font-semibold text-red-300">
            {t("hfBrowser.bundle.registrationFailed")}
          </div>
          <p className="mt-1 text-[12px] text-fg/55">{bundleStatus.setupError}</p>
          <button
            onClick={() => void bundle.retryRegistration()}
            disabled={bundle.busy}
            className="mt-3 flex items-center gap-2 rounded-xl border border-red-400/25 px-4 py-2 text-[12px] font-semibold text-red-200"
          >
            <RefreshCw size={13} />
            {t("hfBrowser.bundle.retryRegistration")}
          </button>
        </div>
      ) : bundle.draft.bundleId ? null : (
        <button
          onClick={() => void bundle.startDownload()}
          disabled={
            bundle.busy ||
            blocked ||
            !estimate ||
            bundle.estimateBusy ||
            !bundle.draft.displayName.trim()
          }
          className="flex h-12 w-full items-center justify-center gap-2 rounded-xl bg-accent text-[13px] font-semibold text-white shadow-[0_12px_35px_rgba(var(--accent-rgb),0.18)] transition hover:brightness-110 active:scale-[0.99] disabled:cursor-not-allowed disabled:opacity-35"
        >
          {bundle.busy ? <Loader size={16} className="animate-spin" /> : <Download size={16} />}
          {t("hfBrowser.bundle.downloadCreate")}
        </button>
      )}
      </div>
      </div>
    </div>
  );
}
