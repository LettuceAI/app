import { motion, AnimatePresence } from "framer-motion";
import { User, PenTool, Tag, FileText, Globe, AlertCircle } from "lucide-react";
import { radius, interactive, typography, spacing, shadows, cn } from "../../../design-tokens";

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
  onSave,
  saving,
  error,
}: ExtrasStepProps) {
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
          Extra Details
        </h2>
        <p className={cn(typography.body.size, "text-fg/50")}>All fields are optional</p>
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
          Nickname
        </label>
        <div className="relative">
          <input
            value={nickname}
            onChange={(e) => onNicknameChange(e.target.value)}
            placeholder="What should the user call this character?"
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
          Alternative display name used in conversations
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
            Creator
          </label>
          <div className="relative">
            <input
              value={creator}
              onChange={(e) => onCreatorChange(e.target.value)}
              placeholder="Creator name..."
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
            Tags
          </label>
          <div className="relative">
            <input
              value={tagsText}
              onChange={(e) => onTagsTextChange(e.target.value)}
              placeholder="fantasy, sci-fi, romance..."
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
            Comma-separated list for filtering and organization
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
              Creator Notes
            </label>
          </div>
          <textarea
            value={creatorNotes}
            onChange={(e) => onCreatorNotesChange(e.target.value)}
            rows={4}
            placeholder="Usage tips, lore context, or instructions for other users..."
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
              Multilingual Notes
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
            JSON object with language codes as keys
          </p>
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
              <span>Creating Character...</span>
            </div>
          ) : (
            "Create Character"
          )}
        </motion.button>
      </div>
    </motion.div>
  );
}
