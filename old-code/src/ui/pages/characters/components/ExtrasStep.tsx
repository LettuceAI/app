import { motion, AnimatePresence } from "framer-motion";
import { User, PenTool, Tag, FileText, Globe, AlertCircle } from "lucide-react";
import { radius, interactive, typography, spacing, shadows, cn } from "../../../design-tokens";
import { useI18n } from "../../../../core/i18n/context";
import { ActiveLorebooksSelector } from "./ActiveLorebooksSelector";

interface ExtrasStepProps {
  nickname: string;
  onNicknameChange: (value: string) => void;
  creator: string;
  onCreatorChange: (value: string) => void;
  creatorNotes: string;
  onCreatorNotesChange: (value: string) => void;
  creatorNotesMultilingualText: string;
  onCreatorNotesMultilingualTextChange: (value: string) => void;
  tagsText: string;
  onTagsTextChange: (value: string) => void;
  activeLorebookIds: string[];
  onActiveLorebookIdsChange: (ids: string[]) => void;
  onSave: () => void;
  saving: boolean;
  error: string | null;
}

export function ExtrasStep({
  nickname,
  onNicknameChange,
  creator,
  onCreatorChange,
  creatorNotes,
  onCreatorNotesChange,
  creatorNotesMultilingualText,
  onCreatorNotesMultilingualTextChange,
  tagsText,
  onTagsTextChange,
  activeLorebookIds,
  onActiveLorebookIdsChange,
  onSave,
  saving,
  error,
}: ExtrasStepProps) {
  const { t } = useI18n();

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -16 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className={spacing.section}
    >
      <div className={spacing.tight}>
        <h2 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
          {t("characters.extras.title")}
        </h2>
        <p className={cn(typography.body.size, "text-fg/50")}>{t("characters.extras.subtitle")}</p>
      </div>

      {/* Nickname */}
      <div className={spacing.field}>
        <label
          className={cn(
            typography.label.size,
            typography.label.weight,
            typography.label.tracking,
            "uppercase text-fg/70",
          )}
        >
          {t("characters.extras.nickname")}
        </label>
        <div className="relative">
          <input
            value={nickname}
            onChange={(e) => onNicknameChange(e.target.value)}
            placeholder={t("characters.extras.nicknamePlaceholder")}
            className={cn(
              "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
              radius.md,
              typography.body.size,
              interactive.transition.default,
              "focus:bg-surface-el/30 focus:outline-none",
              nickname.trim()
                ? "border-fg/20 focus:border-fg/40"
                : "border-fg/10 focus:border-fg/30",
            )}
          />
          {nickname.trim() && (
            <motion.div
              initial={{ scale: 0, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2"
            >
              <User className="h-4 w-4 text-fg/30" />
            </motion.div>
          )}
        </div>
        <p className={cn(typography.bodySmall.size, "text-fg/40")}>
          {t("characters.extras.nicknameDesc")}
        </p>
      </div>

      {/* Creator + Tags */}
      <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
        <div className={spacing.field}>
          <label
            className={cn(
              typography.label.size,
              typography.label.weight,
              typography.label.tracking,
              "uppercase text-fg/70",
            )}
          >
            {t("characters.extras.creator")}
          </label>
          <div className="relative">
            <input
              value={creator}
              onChange={(e) => onCreatorChange(e.target.value)}
              placeholder={t("characters.extras.creatorPlaceholder")}
              className={cn(
                "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                radius.md,
                typography.body.size,
                interactive.transition.default,
                "focus:bg-surface-el/30 focus:outline-none",
                creator.trim()
                  ? "border-fg/20 focus:border-fg/40"
                  : "border-fg/10 focus:border-fg/30",
              )}
            />
            {creator.trim() && (
              <motion.div
                initial={{ scale: 0, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2"
              >
                <PenTool className="h-3.5 w-3.5 text-fg/30" />
              </motion.div>
            )}
          </div>
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
            {t("characters.extras.tags")}
          </label>
          <div className="relative">
            <input
              value={tagsText}
              onChange={(e) => onTagsTextChange(e.target.value)}
              placeholder={t("characters.extras.tagsPlaceholder")}
              className={cn(
                "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                radius.md,
                typography.body.size,
                interactive.transition.default,
                "focus:bg-surface-el/30 focus:outline-none",
                tagsText.trim()
                  ? "border-fg/20 focus:border-fg/40"
                  : "border-fg/10 focus:border-fg/30",
              )}
            />
            {tagsText.trim() && (
              <motion.div
                initial={{ scale: 0, opacity: 0 }}
                animate={{ scale: 1, opacity: 1 }}
                className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2"
              >
                <Tag className="h-3.5 w-3.5 text-fg/30" />
              </motion.div>
            )}
          </div>
          <p className={cn(typography.bodySmall.size, "text-fg/40")}>
            {t("characters.extras.tagsDesc")}
          </p>
        </div>
      </div>

      {/* Creator Notes */}
      <div className="grid grid-cols-1 gap-4 lg:grid-cols-2">
        <div className={spacing.field}>
          <div className="flex items-center gap-2">
            <FileText className="h-3.5 w-3.5 text-fg/40" />
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              {t("characters.extras.creatorNotes")}
            </label>
          </div>
          <textarea
            value={creatorNotes}
            onChange={(e) => onCreatorNotesChange(e.target.value)}
            rows={4}
            placeholder={t("characters.extras.creatorNotesPlaceholder")}
            className={cn(
              "w-full resize-none border bg-surface-el/20 px-4 py-3 leading-relaxed text-fg placeholder-fg/40 backdrop-blur-xl",
              radius.md,
              typography.body.size,
              interactive.transition.default,
              "focus:bg-surface-el/30 focus:outline-none",
              creatorNotes.trim()
                ? "border-fg/20 focus:border-fg/40"
                : "border-fg/10 focus:border-fg/30",
            )}
          />
        </div>

        <div className={spacing.field}>
          <div className="flex items-center gap-2">
            <Globe className="h-3.5 w-3.5 text-fg/40" />
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              {t("characters.extras.multilingualNotes")}
            </label>
          </div>
          <textarea
            value={creatorNotesMultilingualText}
            onChange={(e) => onCreatorNotesMultilingualTextChange(e.target.value)}
            rows={4}
            placeholder='{"en": "English note", "ja": "メモ"}'
            className={cn(
              "w-full resize-none border bg-surface-el/20 px-4 py-3 text-sm leading-relaxed text-fg placeholder-fg/40 backdrop-blur-xl",
              radius.md,
              interactive.transition.default,
              "font-mono focus:bg-surface-el/30 focus:outline-none",
              creatorNotesMultilingualText.trim()
                ? "border-fg/20 focus:border-fg/40"
                : "border-fg/10 focus:border-fg/30",
            )}
          />
          <p className={cn(typography.bodySmall.size, "text-fg/40")}>
            {t("characters.extras.multilingualNotesHint")}
          </p>
        </div>
      </div>

      <ActiveLorebooksSelector
        selectedIds={activeLorebookIds}
        onChange={onActiveLorebookIdsChange}
        disabled={saving}
      />

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
            <div className="flex items-start gap-3">
              <AlertCircle className="mt-0.5 h-5 w-5 shrink-0 text-danger" />
              <p className={cn(typography.body.size, "text-danger")}>{error}</p>
            </div>
          </motion.div>
        )}
      </AnimatePresence>

      <div className="pt-2">
        <motion.button
          disabled={saving}
          onClick={onSave}
          whileTap={{ scale: saving ? 1 : 0.97 }}
          className={cn(
            "w-full py-4 text-base font-semibold",
            radius.md,
            interactive.transition.fast,
            saving
              ? "cursor-not-allowed border border-fg/5 bg-fg/5 text-fg/30"
              : cn(
                  "border border-accent/40 bg-accent/20 text-accent",
                  shadows.glow,
                  "active:border-accent/60 active:bg-accent/30",
                ),
          )}
        >
          {saving ? (
            <div className="flex items-center justify-center gap-2">
              <div className="h-4 w-4 animate-spin rounded-full border-2 border-accent/30 border-t-accent/80" />
              <span>{t("characters.extras.creatingCharacter")}</span>
            </div>
          ) : (
            t("characters.identity.title")
          )}
        </motion.button>
      </div>
    </motion.div>
  );
}
