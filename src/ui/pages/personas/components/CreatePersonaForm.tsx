import { Dispatch } from "react";
import { Bookmark, Camera, X, Upload, Loader2, CheckCircle } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import { PersonaFormState, PersonaFormAction } from "../hooks/createPersonaReducer";
import { AvatarPicker } from "../../../components/AvatarPicker";
import { DesignReferenceEditor } from "../../../components/DesignReferenceEditor";
import { typography, radius, spacing, interactive, cn } from "../../../design-tokens";
import type { AvatarCrop } from "../../../../core/storage/schemas";
import { useI18n } from "../../../../core/i18n/context";

const wordCount = (text: string) => {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return trimmed.split(/\s+/).length;
};

interface CreatePersonaFormProps {
  state: PersonaFormState;
  dispatch: Dispatch<PersonaFormAction>;
  canSave: boolean;
  onSave: () => void;
  onImport: () => void;
}

export function CreatePersonaForm({
  state,
  dispatch,
  canSave,
  onSave,
  onImport,
}: CreatePersonaFormProps) {
  const { t } = useI18n();
  const {
    title,
    description,
    nickname,
    avatarPath,
    avatarCrop,
    avatarRoundPath,
    designDescription,
    designReferenceImageIds,
    isDefault,
    saving,
    importing,
    error,
  } = state;

  const handleAvatarChange = (newPath: string) => {
    dispatch({ type: "set_avatar_path", value: newPath || null });
  };

  const handleAvatarCropChange = (crop: AvatarCrop | null) => {
    dispatch({ type: "set_avatar_crop", value: crop });
  };

  const handleAvatarRoundChange = (roundPath: string | null) => {
    dispatch({ type: "set_avatar_round_path", value: roundPath });
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -16 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className="space-y-6"
    >
      {/* Title */}
      <div className="space-y-1.5 text-left">
        <h2 className="text-xl font-semibold text-fg">{t("personas.empty.createButton")}</h2>
        <p className="text-sm text-fg/50">{t("components.createMenu.personaDesc")}</p>
      </div>

      <div className="grid gap-8 xl:grid-cols-[minmax(0,480px)_minmax(0,1fr)] xl:items-start">
        <div className="space-y-5">
          {/* Avatar Section */}
          <div className="flex flex-col items-center py-4 xl:items-start xl:py-0">
            <div className="relative">
              <AvatarPicker
                currentAvatarPath={avatarPath ?? ""}
                onAvatarChange={handleAvatarChange}
                promptSubjectName={title}
                promptSubjectDescription={description}
                avatarCrop={avatarCrop}
                onAvatarCropChange={handleAvatarCropChange}
                avatarRoundPath={avatarRoundPath}
                onAvatarRoundChange={handleAvatarRoundChange}
                size={"lg"}
                avatarPreview={
                  avatarPath ? undefined : (
                    <div className="flex h-full w-full items-center justify-center">
                      <Camera className="text-fg/30" size={36} />
                    </div>
                  )
                }
              />

              {avatarPath && (
                <button
                  onClick={() => {
                    dispatch({ type: "set_avatar_path", value: null });
                    dispatch({ type: "set_avatar_crop", value: null });
                    dispatch({ type: "set_avatar_round_path", value: null });
                  }}
                  className="absolute -top-1 -left-1 z-30 flex h-12 w-12 items-center justify-center rounded-full border border-fg/10 bg-surface-el text-fg/60 transition hover:bg-danger/80 hover:border-danger/50 hover:text-fg active:scale-95"
                >
                  <X size={14} strokeWidth={2.5} />
                </button>
              )}
            </div>
            <p className="mt-3 text-center text-xs text-fg/40 xl:text-left">
              {t("personas.edit.avatarHint")}
            </p>
          </div>

          {/* Title Input */}
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
                onChange={(e) => dispatch({ type: "set_title", value: e.target.value })}
                placeholder="Professional Writer"
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

          {/* Nickname Input */}
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
            <div className="relative">
              <input
                value={nickname}
                onChange={(e) => dispatch({ type: "set_nickname", value: e.target.value })}
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
            </div>
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              {t("personas.edit.nicknameHint")}
            </p>
          </div>

          {/* Default Option */}
          <div className={spacing.field}>
            <button
              onClick={() => dispatch({ type: "set_default", value: !isDefault })}
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
                  <h3 className="text-sm font-medium text-fg">{t("personas.edit.setAsDefault")}</h3>
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
        </div>

        <div className="space-y-5">
          {/* Description Textarea */}
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
              onChange={(e) => dispatch({ type: "set_description", value: e.target.value })}
              rows={10}
              placeholder="Write in a professional, clear, and concise style. Use formal language and focus on delivering information effectively..."
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
            onDesignDescriptionChange={(value) =>
              dispatch({ type: "set_design_description", value })
            }
            referenceImages={designReferenceImageIds}
            onReferenceImagesChange={(value) =>
              dispatch({ type: "set_design_reference_image_ids", value })
            }
            title="Design references"
            description="Attach a few stable image references and one concise design note for scene generation."
          />
        </div>
      </div>

      {/* Error Display */}
      <AnimatePresence>
        {error && (
          <motion.div
            initial={{ opacity: 0, height: 0 }}
            animate={{ opacity: 1, height: "auto" }}
            exit={{ opacity: 0, height: 0 }}
            className={cn(
              "overflow-hidden border border-danger/20 bg-danger/10 px-4 py-3 backdrop-blur-xl",
              radius.md,
            )}
          >
            <p className="text-sm text-danger">{error}</p>
          </motion.div>
        )}
      </AnimatePresence>

      <div className="flex flex-col gap-3 border-t border-fg/10 pt-4 xl:flex-row xl:items-center xl:justify-between">
        <motion.button
          onClick={onImport}
          disabled={importing}
          whileTap={{ scale: importing ? 1 : 0.98 }}
          className={cn(
            "w-full border border-info/40 bg-info/20 px-4 py-3.5 text-sm font-semibold text-info transition hover:bg-info/30 disabled:opacity-50 xl:w-auto xl:min-w-56",
            radius.md,
          )}
        >
          {importing ? (
            <span className="flex items-center justify-center gap-2">
              <Loader2 className="h-4 w-4 animate-spin" />
              {t("common.buttons.importing")}
            </span>
          ) : (
            <span className="flex items-center justify-center gap-2">
              <Upload className="h-4 w-4" />
              {t("common.buttons.import")} {t("common.nav.personas")}
            </span>
          )}
        </motion.button>

        <motion.button
          disabled={!canSave}
          onClick={onSave}
          whileTap={{ scale: canSave ? 0.98 : 1 }}
          className={cn(
            "w-full py-3.5 text-sm font-semibold transition xl:w-auto xl:min-w-64",
            radius.md,
            canSave
              ? "border border-accent/40 bg-accent/20 text-accent/70 shadow-[0_8px_24px_rgba(52,211,153,0.15)] hover:border-accent/60 hover:bg-accent/30"
              : "cursor-not-allowed border border-fg/5 bg-fg/5 text-fg/30",
          )}
        >
          {saving ? (
            <div className="flex items-center justify-center gap-2">
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-accent/30 border-t-accent/80" />
              <span>{t("common.buttons.creating")}</span>
            </div>
          ) : (
            t("personas.empty.createButton")
          )}
        </motion.button>
      </div>
    </motion.div>
  );
}
