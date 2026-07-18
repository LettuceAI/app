import { AlertTriangle, ExternalLink, HardDrive, KeyRound, Loader, Search } from "lucide-react";

import { useI18n } from "../../../../../core/i18n/context";
import { openExternalUrl } from "../../../../../core/utils/openExternal";
import { formatBundleBytes, type ValidatedAsset } from "./types";
import type { ImageBundleController } from "./useImageBundle";

export function BundleFilePicker({
  bundle,
  modelId,
  onSelect,
  onAddToken,
}: {
  bundle: ImageBundleController;
  modelId: string;
  onSelect: (asset: ValidatedAsset) => void;
  onAddToken: () => void;
}) {
  const { t } = useI18n();
  const role = bundle.currentRole;
  const authShaped = bundle.roleFilesError
    ? /token|license|gated|access/i.test(bundle.roleFilesError)
    : false;

  return (
    <div className="flex min-h-0 flex-1 flex-col">
      <div className="border-b border-fg/8 px-3 py-3">
        <div className="text-[10px] font-semibold uppercase tracking-[0.14em] text-accent">
          {role
            ? t("hfBrowser.bundle.componentStep", {
                current: bundle.requiredRoles.indexOf(role) + 1,
                total: bundle.requiredRoles.length,
              })
            : null}
        </div>
        <div className="mt-1 text-[13px] font-semibold text-fg">
          {role
            ? t("hfBrowser.bundle.selectFileHeading", { role: bundle.roleLabel(role) })
            : null}
        </div>
        {bundle.roleFiles.length > 0 && (
          <div className="mt-0.5 text-[10.5px] text-fg/40">
            {t("hfBrowser.bundle.compatibleFiles", { count: bundle.roleFiles.length })}
          </div>
        )}
      </div>

      <div className="flex-1 space-y-2 overflow-y-auto px-3 py-3 pb-6">
        {bundle.roleFilesLoading && (
          <div className="flex items-center justify-center gap-2 py-10 text-fg/50">
            <Loader size={16} className="animate-spin" />
          </div>
        )}

        {!bundle.roleFilesLoading && bundle.roleFilesError && (
          <div className="rounded-xl border border-amber-400/20 bg-amber-400/5 p-3">
            <div className="flex items-start gap-2">
              <AlertTriangle size={14} className="mt-0.5 shrink-0 text-amber-300" />
              <p className="text-[11.5px] leading-relaxed text-fg/60">{bundle.roleFilesError}</p>
            </div>
            {authShaped && (
              <p className="mt-2 text-[11px] text-fg/45">{t("hfBrowser.bundle.authHint")}</p>
            )}
            <div className="mt-3 flex flex-wrap gap-2">
              {authShaped && (
                <button
                  onClick={onAddToken}
                  className="flex items-center gap-1.5 rounded-lg border border-amber-400/25 px-3 py-1.5 text-[11px] font-semibold text-amber-200"
                >
                  <KeyRound size={11} />
                  {t("hfBrowser.bundle.addToken")}
                </button>
              )}
              <button
                onClick={() => void openExternalUrl(`https://huggingface.co/${modelId}`)}
                className="flex items-center gap-1.5 rounded-lg border border-fg/12 px-3 py-1.5 text-[11px] font-medium text-fg/60 hover:text-fg"
              >
                <ExternalLink size={11} />
                {t("hfBrowser.browseOnHuggingFace")}
              </button>
            </div>
          </div>
        )}

        {!bundle.roleFilesLoading && !bundle.roleFilesError && bundle.roleFiles.length === 0 && (
          <div className="flex flex-col items-center gap-2 px-2 py-10 text-center">
            <Search size={20} className="text-fg/20" />
            <p className="text-[11.5px] leading-relaxed text-fg/45">
              {role
                ? t("hfBrowser.bundle.noCompatibleFiles", { role: bundle.roleLabel(role) })
                : null}
            </p>
          </div>
        )}

        {bundle.roleFiles.map((file) => (
          <div
            key={file.selectionId}
            className="rounded-xl border border-fg/10 bg-fg/3 px-3 py-2.5"
          >
            <div className="flex items-center gap-2">
              <HardDrive size={13} className="shrink-0 text-fg/40" />
              <p className="min-w-0 flex-1 truncate font-mono text-[11.5px] text-fg" title={file.relativePath}>
                {file.relativePath}
              </p>
            </div>
            <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
              <span className="rounded-md border border-accent/20 bg-accent/10 px-1.5 py-0.5 text-[9px] font-semibold text-accent/80">
                {file.format.toUpperCase()}
              </span>
              {file.quantization && (
                <span className="rounded-md border border-fg/12 bg-fg/5 px-1.5 py-0.5 text-[9px] font-semibold text-fg/60">
                  {file.quantization}
                </span>
              )}
              {file.gated && (
                <span className="rounded-md bg-amber-400/10 px-1.5 py-0.5 text-[9px] font-semibold text-amber-300">
                  {t("hfBrowser.bundle.gated")}
                </span>
              )}
              <span className="text-[11px] text-fg/45">{formatBundleBytes(file.size)}</span>
              <span className="font-mono text-[9.5px] text-fg/30">{file.revision.slice(0, 8)}</span>
            </div>
            <button
              onClick={() => onSelect(file)}
              className="mt-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-accent/30 bg-accent/15 py-1.5 text-[11px] font-semibold text-accent transition hover:bg-accent/25 active:scale-[0.97]"
            >
              {t("hfBrowser.bundle.selectFile")}
            </button>
          </div>
        ))}
      </div>
    </div>
  );
}
