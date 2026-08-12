import {
  Download,
  Upload,
  Trash2,
  FileArchive,
  Loader2,
  CheckCircle,
  AlertTriangle,
  Eye,
  EyeOff,
  Lock,
  HardDrive,
} from "lucide-react";
import { interactive, radius, cn } from "../../design-tokens";
import { BottomMenu } from "../../components/BottomMenu";
import { useBackupRestore, type BackupInfo } from "./hooks/useBackupRestore";
import { useState } from "react";
import { useNavigate } from "react-router-dom";
import { storageBridge } from "../../../core/storage/files";
import { useI18n } from "../../../core/i18n/context";

function formatDate(timestamp: number) {
  const date = new Date(timestamp);
  return `${date.toLocaleDateString(undefined, { month: "short", day: "numeric" })}, ${date.toLocaleTimeString(undefined, { hour: "numeric", minute: "2-digit" })}`;
}

export function BackupRestorePage() {
  const { state, actions } = useBackupRestore();
  const [showEmbeddingPrompt, setShowEmbeddingPrompt] = useState(false);
  const navigate = useNavigate();
  const { t } = useI18n();

  const handleRestoreClick = async () => {
    const result = await actions.handleImport();
    if (result?.needsEmbeddingModel) {
      setShowEmbeddingPrompt(true);
    } else if (result?.success) {
      // Import complete - navigate to chat, data is ready in DB
      navigate("/");
    }
  };

  const handleDownloadModel = () => {
    setShowEmbeddingPrompt(false);
    actions.closeModal();
    navigate("/settings/embedding-download?returnTo=/");
  };

  const handleDisableAndContinue = async () => {
    setShowEmbeddingPrompt(false);

    // Import is already done, just disable dynamic memory
    try {
      await storageBridge.backupDisableDynamicMemory();
    } catch (error) {
      console.error("Failed to disable dynamic memory:", error);
      // Don't show error to user - the import was successful
    }
    // Navigate to chat after successful import
    navigate("/");
  };

  return (
    <div className="flex h-full flex-col">
      <section className="flex-1 overflow-y-auto px-3 pt-3 pb-6 space-y-4">
        {/* Success Message */}
        {state.exportSuccess && (
          <div className="flex items-center gap-3 rounded-xl border border-accent/20 bg-accent/10 p-3">
            <CheckCircle className="h-5 w-5 shrink-0 text-accent" />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium text-accent/80">{t("backup.extra.successMessage")}</p>
              <p className="text-xs text-accent/80/60">{t("backup.extra.savedLocation")}</p>
            </div>
            <button
              onClick={actions.clearExportSuccess}
              className="text-accent/60 hover:text-accent/80 text-lg px-1"
            >
              ×
            </button>
          </div>
        )}

        {/* Create Backup */}
        <div>
          <h2 className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
            {t("backup.tabs.create")}
          </h2>
          <button
            onClick={actions.openExportModal}
            disabled={state.exporting}
            className={cn(
              "w-full rounded-xl border border-fg/10 bg-fg/5 p-4 text-left",
              interactive.transition.default,
              "hover:border-fg/20 hover:bg-fg/8",
              "active:scale-[0.99]",
              "disabled:opacity-50",
            )}
          >
            <div className="flex items-center gap-3">
              <div className="flex h-10 w-10 items-center justify-center rounded-full border border-accent/30 bg-accent/15">
                {state.exporting ? (
                  <Loader2 className="h-5 w-5 animate-spin text-accent/80" />
                ) : (
                  <Download className="h-5 w-5 text-accent/80" />
                )}
              </div>
              <div className="flex-1">
                <p className="text-sm font-medium text-fg">{t("backup.create.newBackupButton")}</p>
                <p className="text-[11px] text-fg/50">{t("backup.create.exportDescription")}</p>
              </div>
            </div>
          </button>
        </div>

        {/* Backups List */}
        <div>
          <div className="flex items-center justify-between mb-2 px-1">
            <h2 className="text-[10px] font-semibold uppercase tracking-[0.25em] text-fg/35">
              {t("backup.restore.availableBackups")}
            </h2>
            <button
              onClick={actions.handleBrowseForBackup}
              className="text-[10px] font-medium text-info hover:text-info/80"
            >
              {t("backup.restore.browseFiles")}
            </button>
          </div>

          {state.loading ? (
            <div className="flex items-center justify-center py-12">
              <Loader2 className="h-6 w-6 animate-spin text-fg/30" />
            </div>
          ) : state.backups.length === 0 ? (
            <div className="rounded-xl border border-fg/10 bg-fg/5 p-6 text-center">
              <FileArchive className="mx-auto h-8 w-8 text-fg/20" />
              <p className="mt-3 text-sm text-fg/40">{t("backup.restore.noBackupsFound")}</p>
              <p className="mt-1 text-xs text-fg/30">
                {t("backup.restore.noBackupsDesc")}
              </p>
              <button
                onClick={actions.handleBrowseForBackup}
                className={cn(
                  "mt-4 inline-flex items-center gap-2 px-4 py-2 rounded-lg",
                  "border border-info/30 bg-info/10",
                  "text-sm text-info/80 font-medium",
                  "hover:bg-info/20 active:scale-[0.98]",
                  interactive.transition.default,
                )}
              >
                <Upload className="h-4 w-4" />
                {t("backup.restore.browseDesc")}
              </button>
            </div>
          ) : (
            <div className="space-y-2">
              {state.backups.map((backup) => (
                <BackupItem
                  key={backup.path}
                  backup={backup}
                  onRestore={() => actions.openImportModal(backup)}
                  onDelete={() => actions.openDeleteModal(backup)}
                />
              ))}
            </div>
          )}
        </div>

        <p className="px-1 text-[11px] text-fg/30">{t("backup.footer.note")}</p>
      </section>

      {/* Export Modal */}
      <BottomMenu
        isOpen={state.activeModal === "export"}
        onClose={actions.closeModal}
        title={t("backup.create.createButton")}
      >
        <div className="space-y-4">
          <p className="text-sm text-fg/50">{t("backup.export.passwordPrompt")}</p>

          {state.error && (
            <div className="flex items-center gap-2 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger/80">
              <AlertTriangle className="h-4 w-4 shrink-0" />
              {state.error}
            </div>
          )}

          <div className="space-y-3">
            <div>
              <label className="mb-1.5 block text-xs font-medium text-fg/50">{t("backup.export.passwordLabel")}</label>
              <div className="relative">
                <input
                  type={state.showExportPassword ? "text" : "password"}
                  value={state.exportPassword}
                  onChange={(e) => actions.setExportPassword(e.target.value)}
                  placeholder={t("backup.export.passwordPlaceholder")}
                  className={cn(
                    "w-full border border-fg/10 bg-fg/5 px-4 py-3 pr-12 text-fg placeholder-fg/30",
                    radius.lg,
                    "focus:border-fg/20 focus:outline-none",
                  )}
                />
                <button
                  type="button"
                  onClick={actions.toggleShowExportPassword}
                  className="absolute right-3 top-1/2 -translate-y-1/2 text-fg/40 hover:text-fg/60"
                >
                  {state.showExportPassword ? (
                    <EyeOff className="h-5 w-5" />
                  ) : (
                    <Eye className="h-5 w-5" />
                  )}
                </button>
              </div>
            </div>

            <div>
              <label className="mb-1.5 block text-xs font-medium text-fg/50">
                {t("backup.export.confirmPasswordLabel")}
              </label>
              <input
                type={state.showExportPassword ? "text" : "password"}
                value={state.confirmPassword}
                onChange={(e) => actions.setConfirmPassword(e.target.value)}
                placeholder={t("backup.export.confirmPasswordPlaceholder")}
                className={cn(
                  "w-full border border-fg/10 bg-fg/5 px-4 py-3 text-fg placeholder-fg/30",
                  radius.lg,
                  "focus:border-fg/20 focus:outline-none",
                )}
              />
            </div>
          </div>

          <div className="flex gap-3 pt-2">
            <button
              onClick={actions.closeModal}
              className={cn(
                "flex-1 border border-fg/10 bg-fg/5 px-4 py-3 text-sm font-medium text-fg/70",
                radius.lg,
                "hover:bg-fg/10",
              )}
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={actions.handleExport}
              disabled={
                state.exporting ||
                state.exportPassword.length < 6 ||
                state.exportPassword !== state.confirmPassword
              }
              className={cn(
                "flex flex-1 items-center justify-center gap-2 bg-accent px-4 py-3 text-sm font-medium text-fg",
                radius.lg,
                "hover:bg-accent/80",
                "disabled:opacity-50",
              )}
            >
              {state.exporting ? (
                <>
                  <Loader2 className="h-4 w-4 animate-spin" />
                  {t("common.buttons.creating")}
                </>
              ) : (
                <>
                  <Download className="h-4 w-4" />
                  {t("common.buttons.create")}
                </>
              )}
            </button>
          </div>
        </div>
      </BottomMenu>

      {/* Import Modal */}
      <BottomMenu
        isOpen={state.activeModal === "import"}
        onClose={actions.closeModal}
        title={t("backup.restore.restoreDialogTitle")}
      >
        <div className="space-y-4">
          {state.selectedBackup && (
            <>
              <div className={cn("border border-fg/10 bg-fg/5 p-3", radius.lg)}>
                <div className="flex items-center gap-3">
                  <FileArchive className="h-6 w-6 text-fg/40" />
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-sm font-medium text-fg">
                      {state.selectedBackup.filename}
                    </p>
                    <p className="text-xs text-fg/40">
                      {formatDate(state.selectedBackup.createdAt)} · v
                      {state.selectedBackup.appVersion}
                    </p>
                  </div>
                </div>
              </div>

              <div className="flex items-start gap-2 rounded-lg border border-warning/30 bg-warning/10 px-3 py-2 text-xs text-warning/80">
                <AlertTriangle className="h-4 w-4 shrink-0 mt-0.5" />
                <span>{t("backup.import.replaceWarning")}</span>
              </div>

              {state.error && (
                <div className="flex items-center gap-2 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2 text-sm text-danger/80">
                  <AlertTriangle className="h-4 w-4 shrink-0" />
                  {state.error}
                </div>
              )}

              {state.selectedBackup.encrypted && (
                <div>
                  <label className="mb-1.5 block text-xs font-medium text-fg/50">
                    {t("backup.import.passwordLabel")}
                  </label>
                  <div className="relative">
                    <input
                      type={state.showImportPassword ? "text" : "password"}
                      value={state.importPassword}
                      onChange={(e) => actions.setImportPassword(e.target.value)}
                      placeholder={t("backup.import.passwordPlaceholder")}
                      className={cn(
                        "w-full border border-fg/10 bg-fg/5 px-4 py-3 pr-12 text-fg placeholder-fg/30",
                        radius.lg,
                        "focus:border-fg/20 focus:outline-none",
                      )}
                    />
                    <button
                      type="button"
                      onClick={actions.toggleShowImportPassword}
                      className="absolute right-3 top-1/2 -translate-y-1/2 text-fg/40 hover:text-fg/60"
                    >
                      {state.showImportPassword ? (
                        <EyeOff className="h-5 w-5" />
                      ) : (
                        <Eye className="h-5 w-5" />
                      )}
                    </button>
                  </div>
                </div>
              )}

              <div className="flex gap-3 pt-2">
                <button
                  onClick={actions.closeModal}
                  className={cn(
                    "flex-1 border border-fg/10 bg-fg/5 px-4 py-3 text-sm font-medium text-fg/70",
                    radius.lg,
                    "hover:bg-fg/10",
                  )}
                >
                  {t("common.buttons.cancel")}
                </button>
                <button
                  onClick={handleRestoreClick}
                  disabled={
                    state.importing ||
                    (state.selectedBackup.encrypted && state.importPassword.length < 1)
                  }
                  className={cn(
                    "flex flex-1 items-center justify-center gap-2 bg-info px-4 py-3 text-sm font-medium text-fg",
                    radius.lg,
                    "hover:bg-info/80",
                    "disabled:opacity-50",
                  )}
                >
                  {state.importing ? (
                    <>
                      <Loader2 className="h-4 w-4 animate-spin" />
                      {t("common.buttons.importing")}
                    </>
                  ) : (
                    <>
                      <Upload className="h-4 w-4" />
                      {t("common.buttons.import")}
                    </>
                  )}
                </button>
              </div>
            </>
          )}
        </div>
      </BottomMenu>

      {/* Delete Modal */}
      <BottomMenu
        isOpen={state.activeModal === "delete"}
        onClose={actions.closeModal}
        title={t("backup.restore.deleteDialogTitle")}
      >
        <div className="space-y-4">
          {state.selectedBackup && (
            <>
              <p className="text-sm text-fg/50">{t("backup.delete.confirmPrompt")}</p>

              <div className={cn("border border-fg/10 bg-fg/5 p-3", radius.lg)}>
                <p className="truncate text-sm font-medium text-fg">
                  {state.selectedBackup.filename}
                </p>
                <p className="text-xs text-fg/40">
                  {formatDate(state.selectedBackup.createdAt)}
                </p>
              </div>

              <div className="flex gap-3 pt-2">
                <button
                  onClick={actions.closeModal}
                  className={cn(
                    "flex-1 border border-fg/10 bg-fg/5 px-4 py-3 text-sm font-medium text-fg/70",
                    radius.lg,
                    "hover:bg-fg/10",
                  )}
                >
                  {t("common.buttons.cancel")}
                </button>
                <button
                  onClick={actions.handleDelete}
                  className={cn(
                    "flex flex-1 items-center justify-center gap-2 bg-danger px-4 py-3 text-sm font-medium text-fg",
                    radius.lg,
                    "hover:bg-danger/80",
                  )}
                >
                  <Trash2 className="h-4 w-4" />
                  {t("common.buttons.delete")}
                </button>
              </div>
            </>
          )}
        </div>
      </BottomMenu>

      {/* Dynamic Memory Model Required Modal */}
      <BottomMenu
        isOpen={showEmbeddingPrompt}
        onClose={() => setShowEmbeddingPrompt(false)}
        title={t("backup.embedding.modalTitle")}
      >
        <div className="space-y-4">
          <div className="flex items-start gap-3 rounded-xl border border-warning/20 bg-warning/10 p-3">
            <HardDrive className="h-5 w-5 shrink-0 text-warning mt-0.5" />
            <div className="min-w-0 flex-1">
              <p className="text-sm font-medium text-warning/80">{t("backup.embedding.detectedTitle")}</p>
              <p className="mt-1 text-xs text-warning/70">
                {t("backup.embedding.detectedDescription")}
              </p>
            </div>
          </div>

          <p className="text-sm text-fg/60">
            {t("backup.embedding.choiceDescription")}
          </p>

          <div className="flex flex-col gap-2 pt-2">
            <button
              onClick={handleDownloadModel}
              className={cn(
                "flex items-center justify-center gap-2 bg-info px-4 py-3 text-sm font-medium text-fg",
                radius.lg,
                "hover:bg-info/80",
              )}
            >
              <Download className="h-4 w-4" />
              {t("backup.embedding.downloadButton")}
            </button>
            <button
              onClick={handleDisableAndContinue}
              className={cn(
                "border border-fg/10 bg-fg/5 px-4 py-3 text-sm font-medium text-fg/70",
                radius.lg,
                "hover:bg-fg/10",
              )}
            >
              {t("backup.embedding.continueButton")}
            </button>
          </div>

          <p className="text-xs text-fg/40 text-center">
            {t("backup.embedding.reenableNote")}
          </p>
        </div>
      </BottomMenu>
    </div>
  );
}

// Extracted component for backup items
function BackupItem({
  backup,
  onRestore,
  onDelete,
}: {
  backup: BackupInfo;
  onRestore: () => void;
  onDelete: () => void;
}) {
  const { t } = useI18n();
  return (
    <button
      onClick={onRestore}
      className={cn(
        "group w-full rounded-xl border border-fg/10 bg-fg/5 p-3 text-left",
        interactive.transition.default,
        "hover:border-fg/20 hover:bg-fg/8",
        "active:scale-[0.99]",
      )}
    >
      <div className="flex items-center gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/10">
          <FileArchive className="h-4 w-4 text-fg/60" />
        </div>
        <div className="min-w-0 flex-1">
          <div className="flex items-center gap-2">
            <p className="truncate text-sm font-medium text-fg">{backup.filename}</p>
            {backup.encrypted && <Lock className="h-3 w-3 shrink-0 text-warning/70" />}
          </div>
          <p className="mt-0.5 text-[11px] text-fg/40">
            {formatDate(backup.createdAt)} · v{backup.appVersion}
            {backup.totalFiles > 0 && ` · ${t("backup.item.filesCount", { count: backup.totalFiles })}`}
          </p>
        </div>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onDelete();
          }}
          className={cn(
            "shrink-0 rounded-lg p-2 text-fg/20",
            "opacity-100",
            "hover:bg-danger/20 hover:text-danger",
          )}
        >
          <Trash2 className="h-4 w-4" />
        </button>
      </div>
    </button>
  );
}
