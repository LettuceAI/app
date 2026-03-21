import { useState, useEffect } from "react";
import { useParams } from "react-router-dom";
import { Loader2, X, Download, Bookmark, CheckCircle } from "lucide-react";
import { motion } from "framer-motion";
import { usePersonaFormController } from "./hooks/usePersonaFormController";
import {
  exportPersona,
  downloadJson,
  generateExportFilename,
} from "../../../core/storage/personaTransfer";
import { AvatarPicker } from "../../components/AvatarPicker";
import { DesignReferenceEditor } from "../../components/DesignReferenceEditor";
import { useI18n } from "../../../core/i18n/context";
import { typography, radius, spacing, interactive, cn } from "../../design-tokens";

const wordCount = (text: string) => {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return trimmed.split(/\s+/).length;
};

export function EditPersonaPage() {
  const { t } = useI18n();
  const { personaId } = useParams();
  const {
    state: {
      loading,
      saving,
      error,
      title,
      description,
      nickname,
      isDefault,
      avatarPath,
      avatarCrop,
      avatarRoundPath,
      designDescription,
      designReferenceImageIds,
    },
    setTitle,
    setDescription,
    setNickname,
    setIsDefault,
    setAvatarPath,
    setAvatarCrop,
    setAvatarRoundPath,
    setDesignDescription,
    setDesignReferenceImageIds,
    handleSave,
    resetToInitial,
    canSave,
  } = usePersonaFormController(personaId);

  const [exporting, setExporting] = useState(false);

  useEffect(() => {
    const globalWindow = window as any;
    globalWindow.__savePersona = handleSave;
    globalWindow.__savePersonaCanSave = canSave;
    globalWindow.__savePersonaSaving = saving;
    return () => {
      delete globalWindow.__savePersona;
      delete globalWindow.__savePersonaCanSave;
      delete globalWindow.__savePersonaSaving;
    };
  }, [handleSave, canSave, saving]);

  useEffect(() => {
    const handleDiscard = () => resetToInitial();
    window.addEventListener("unsaved:discard", handleDiscard);
    return () => window.removeEventListener("unsaved:discard", handleDiscard);
  }, [resetToInitial]);

  const handleExport = async () => {
    if (!personaId) return;

    try {
      setExporting(true);
      const exportJson = await exportPersona(personaId);
      const filename = generateExportFilename(title || "persona");
      await downloadJson(exportJson, filename);
    } catch (err: any) {
      console.error("Failed to export persona:", err);
      alert(err?.message || "Failed to export persona");
    } finally {
      setExporting(false);
    }
  };

  const handleAvatarChange = (newPath: string) => {
    setAvatarPath(newPath || null);
  };

  if (loading) {
    return (
      <div className="flex h-full items-center justify-center">
        <div className="h-8 w-8 animate-spin rounded-full border-2 border-fg/10 border-t-white/60" />
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col pb-16 text-fg/80">
      <main className="flex-1 overflow-y-auto px-4 pt-4">
        <motion.div
          initial={{ opacity: 0, y: 16 }}
          animate={{ opacity: 1, y: 0 }}
          transition={{ duration: 0.2, ease: "easeOut" }}
          className="space-y-6"
        >
          {/* Error Message */}
          {error && (
            <div className="rounded-xl border border-danger/30 bg-danger/10 px-4 py-3">
              <p className="text-sm text-danger">{error}</p>
            </div>
          )}

          <div className="grid gap-8 xl:grid-cols-[minmax(0,480px)_minmax(0,1fr)] xl:items-start">
            <div className="space-y-5">
              <div className="flex flex-col items-center py-4 xl:py-0">
                <div className="relative">
                  <AvatarPicker
                    currentAvatarPath={avatarPath ?? ""}
                    onAvatarChange={handleAvatarChange}
                    promptSubjectName={title}
                    promptSubjectDescription={description}
                    avatarCrop={avatarCrop}
                    onAvatarCropChange={setAvatarCrop}
                    avatarRoundPath={avatarRoundPath}
                    onAvatarRoundChange={setAvatarRoundPath}
                    placeholder={title.trim().charAt(0).toUpperCase() || "?"}
                  />

                  {avatarPath && (
                    <button
                      type="button"
                      onClick={() => {
                        setAvatarPath(null);
                        setAvatarCrop(null);
                        setAvatarRoundPath(null);
                      }}
                      className="absolute -top-1 -left-1 z-10 flex h-10 w-10 items-center justify-center rounded-full border border-fg/10 bg-surface-el text-fg/60 transition hover:bg-danger/80 hover:border-danger/50 hover:text-fg active:scale-95"
                    >
                      <X size={14} strokeWidth={2.5} />
                    </button>
                  )}
                </div>
                <p className="mt-3 text-center text-xs text-fg/40">
                  {t("personas.edit.avatarHint")}
                </p>
              </div>

              <div className={spacing.field}>
                <label
                  className={cn(
                    typography.label.size,
                    typography.label.weight,
                    typography.label.tracking,
                    "uppercase text-fg/70",
                  )}
                >
                  {t("personas.edit.nameLabel")}
                </label>
                <div className="relative">
                  <input
                    value={title}
                    onChange={(e) => setTitle(e.target.value)}
                    placeholder={t("personas.edit.namePlaceholder")}
                    className={cn(
                      "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                      radius.md,
                      typography.body.size,
                      interactive.transition.default,
                      "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                      title.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10",
                    )}
                  />
                  {title.trim() && (
                    <motion.div
                      initial={{ scale: 0, opacity: 0 }}
                      animate={{ scale: 1, opacity: 1 }}
                      className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2"
                    >
                      <div
                        className={cn(
                          "flex h-6 w-6 items-center justify-center",
                          radius.full,
                          "bg-accent/20",
                        )}
                      >
                        <CheckCircle className="h-3 w-3 text-accent/80" />
                      </div>
                    </motion.div>
                  )}
                </div>
                <p className={cn(typography.bodySmall.size, "text-fg/40")}>
                  {t("personas.edit.nameHint")}
                </p>
              </div>

              <div className={spacing.field}>
                <label
                  className={cn(
                    typography.label.size,
                    typography.label.weight,
                    typography.label.tracking,
                    "uppercase text-fg/70",
                  )}
                >
                  {t("personas.edit.nicknameLabel")}
                </label>
                <input
                  value={nickname}
                  onChange={(e) => setNickname(e.target.value)}
                  placeholder={t("personas.edit.nicknamePlaceholder")}
                  className={cn(
                    "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                    radius.md,
                    typography.body.size,
                    interactive.transition.default,
                    "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                    nickname.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10",
                  )}
                />
                <p className={cn(typography.bodySmall.size, "text-fg/40")}>
                  {t("personas.edit.nicknameHint")}
                </p>
              </div>

              <div className={spacing.field}>
                <button
                  type="button"
                  onClick={() => setIsDefault(!isDefault)}
                  className={cn(
                    "flex w-full items-center justify-between border border-fg/10 bg-surface-el/20 px-4 py-3 backdrop-blur-xl",
                    radius.md,
                    interactive.transition.default,
                    "hover:border-fg/25 hover:bg-fg/5 active:scale-[0.99]",
                  )}
                >
                  <div className="flex items-center gap-3">
                    <div
                      className={cn(
                        "p-2 transition",
                        radius.lg,
                        isDefault ? "bg-accent/20 text-accent/80" : "bg-fg/10 text-fg/50",
                      )}
                    >
                      <Bookmark size={16} />
                    </div>
                    <div className="text-left">
                      <h3 className="text-sm font-medium text-fg">
                        {t("personas.edit.setAsDefault")}
                      </h3>
                      <p className="text-xs text-fg/40">{t("personas.edit.defaultDescription")}</p>
                    </div>
                  </div>

                  <div className="relative">
                    <div
                      className={cn(
                        "h-6 w-11 transition-colors",
                        radius.full,
                        isDefault ? "bg-accent/40" : "bg-fg/20",
                      )}
                    >
                      <div
                        className={cn(
                          "absolute top-0.5 h-5 w-5 transition-transform",
                          radius.full,
                          isDefault ? "translate-x-5 bg-accent" : "translate-x-0.5 bg-fg",
                        )}
                      />
                    </div>
                  </div>
                </button>
              </div>

              <div className={spacing.field}>
                <motion.button
                  onClick={handleExport}
                  disabled={!personaId || exporting}
                  whileTap={{ scale: exporting ? 1 : 0.98 }}
                  className={cn(
                    "w-full border border-info/40 bg-info/20 px-4 py-3.5 text-sm font-semibold text-info transition hover:bg-info/30 disabled:opacity-50",
                    radius.md,
                  )}
                >
                  {exporting ? (
                    <span className="flex items-center justify-center gap-2">
                      <Loader2 className="h-4 w-4 animate-spin" />
                      {t("common.buttons.exporting")}
                    </span>
                  ) : (
                    <span className="flex items-center justify-center gap-2">
                      <Download className="h-4 w-4" />
                      {t("personas.edit.exportButton")}
                    </span>
                  )}
                </motion.button>
              </div>
            </div>

            <div className="space-y-5">
              <div className={spacing.field}>
                <label
                  className={cn(
                    typography.label.size,
                    typography.label.weight,
                    typography.label.tracking,
                    "uppercase text-fg/70",
                  )}
                >
                  {t("personas.edit.descriptionLabel")}
                </label>
                <textarea
                  value={description}
                  onChange={(e) => setDescription(e.target.value)}
                  rows={10}
                  placeholder={t("personas.edit.descriptionPlaceholder")}
                  className={cn(
                    "min-h-35 max-h-96 w-full resize-y border bg-surface-el/20 px-4 py-3.5 text-sm leading-relaxed text-fg placeholder-fg/40 backdrop-blur-xl xl:min-h-72",
                    radius.md,
                    interactive.transition.default,
                    "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                    description.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10",
                  )}
                />
                <div className="flex justify-end text-[11px] text-fg/40">
                  {wordCount(description)} {t("personas.edit.wordCount")}
                </div>
                <p className={cn(typography.bodySmall.size, "text-fg/40")}>
                  {t("personas.edit.descriptionHint")}
                </p>
              </div>

              <DesignReferenceEditor
                designDescription={designDescription}
                onDesignDescriptionChange={setDesignDescription}
                referenceImages={designReferenceImageIds}
                onReferenceImagesChange={setDesignReferenceImageIds}
                title="Design references"
                description="Attach a few stable image references and one concise design note for scene generation."
              />
            </div>
          </div>
        </motion.div>
      </main>
    </div>
  );
}
