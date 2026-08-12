import { useNavigate } from "react-router-dom";
import { useState } from "react";
import { Trash2, Edit2, Sparkles } from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";
import type { Character } from "../../../core/storage/schemas";
import { BottomMenu } from "../../components";
import { NoModelMenu } from "../../components/CreateMenus/NoModelMenu";
import { hasConfiguredModel } from "../../../core/storage/repo";
import { AvatarImage } from "../../components/AvatarImage";
import { typography, radius, interactive, cn } from "../../design-tokens";
import { useCharactersController } from "../characters/hooks/useCharactersController";
import { useAvatar } from "../../hooks/useAvatar";
import { useMultipleAvatarGradients } from "../../hooks/useAvatarGradient";
import { useI18n } from "../../../core/i18n/context";
import { isRenderableImageUrl } from "../../../core/utils/image";

const CharacterSkeleton = () => (
  <div className="space-y-3">
    {[1, 2, 3].map((i) => (
      <div key={i} className="rounded-xl border border-fg/10 bg-surface/90 px-4 py-3">
        <div className="flex items-center gap-3">
          <div className="h-12 w-12 animate-pulse rounded-full bg-fg/10" />
          <div className="flex-1 space-y-2">
            <div className="h-4 w-32 animate-pulse rounded bg-fg/10" />
            <div className="h-3 w-48 animate-pulse rounded bg-fg/10" />
          </div>
        </div>
      </div>
    ))}
  </div>
);

const EmptyState = ({ onCreate }: { onCreate: () => void }) => {
  const { t } = useI18n();
  return (
    <div className="flex h-64 flex-col items-center justify-center">
      <Sparkles className="mb-3 h-12 w-12 text-fg/20" />
      <h3 className="mb-1 text-lg font-medium text-fg">{t("characters.empty.title")}</h3>
      <p className="mb-4 text-center text-sm text-fg/50">{t("characters.empty.description")}</p>
      <button
        onClick={onCreate}
        className="rounded-full border border-accent/40 bg-accent/20 px-6 py-2 text-sm font-medium text-accent/90 transition hover:bg-accent/30 active:scale-[0.99]"
      >
        {t("characters.empty.createButton")}
      </button>
    </div>
  );
};

function isImageLike(s?: string) {
  return isRenderableImageUrl(s);
}

function CharacterAvatar({ character }: { character: Character }) {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");

  if (avatarUrl && isImageLike(avatarUrl)) {
    return (
      <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
    );
  }

  const initials = character.name
    .split(" ")
    .map((n) => n[0])
    .join("")
    .toUpperCase()
    .slice(0, 2);
  return <span className="text-sm font-semibold">{initials}</span>;
}

export function CharactersPage() {
  const navigate = useNavigate();
  const { t } = useI18n();
  const [showNoModel, setShowNoModel] = useState(false);
  const {
    state: { characters, loading, selectedCharacter, showDeleteConfirm, deleting },
    setSelectedCharacter,
    setShowDeleteConfirm,
    handleDelete,
  } = useCharactersController();

  const handleCreateCharacter = () => {
    void hasConfiguredModel().then((ok) => {
      if (ok) navigate("/create/character");
      else setShowNoModel(true);
    });
  };

  // Get gradients for all characters at once (follows React rules of hooks)
  const { getGradientCss, hasGradient, getTextColor, getTextSecondary } =
    useMultipleAvatarGradients(
      characters.map((c) => ({
        type: "character" as const,
        id: c.id,
        avatarPath: c.avatarPath,
        disableGradient: c.disableAvatarGradient,
        source: c.avatarGradientSource ?? "base",
      })),
    );

  const handleEditCharacter = (character: Character) => {
    navigate(`/settings/characters/${character.id}/edit`);
  };

  return (
    <div className="flex h-full flex-col text-fg/90">
      <main className="flex-1 overflow-y-auto px-4 pt-4 pb-6">
        {loading ? (
          <CharacterSkeleton />
        ) : characters.length === 0 ? (
          <EmptyState onCreate={handleCreateCharacter} />
        ) : (
          <div className="space-y-3">
            {/* Characters List */}
            <AnimatePresence>
              {characters.map((character) => {
                const descriptionPreview =
                  (character.description || character.definition || "").trim() ||
                  t("common.labels.noDescriptionYet");
                const gradientCss = getGradientCss(character.id);
                const hasGrad = hasGradient(character.id);
                const textColor = getTextColor(character.id);
                const textSecondary = getTextSecondary(character.id);

                return (
                  <motion.div
                    key={character.id}
                    className={cn(
                      "group relative flex w-full items-center gap-3 overflow-hidden px-4 py-3 text-left",
                      radius.md,
                      hasGrad ? "" : "border border-fg/10 bg-surface/90",
                      interactive.transition.default,
                      "hover:border-fg/25 hover:bg-surface/95",
                      interactive.active.scale,
                    )}
                    style={
                      hasGrad
                        ? {
                            background: gradientCss,
                          }
                        : {}
                    }
                  >
                    {/* Hover gradient effect */}
                    <div
                      className={cn(
                        "absolute inset-y-0 right-0 w-1/4 transition",
                        hasGrad
                          ? "bg-linear-to-l from-fg/10 via-transparent to-transparent opacity-0 group-hover:opacity-100"
                          : "bg-linear-to-l from-secondary/10 via-transparent to-transparent opacity-0 group-hover:opacity-100",
                      )}
                    />

                    {/* Avatar */}
                    <div
                      className={cn(
                        "relative flex h-12 w-12 shrink-0 items-center justify-center overflow-hidden",
                        radius.lg,
                        "border border-fg/15 bg-fg/8",
                        typography.body.size,
                        typography.body.weight,
                        "text-fg",
                      )}
                    >
                      <CharacterAvatar character={character} />
                    </div>

                    {/* Content */}
                    <div className="relative min-w-0 flex-1">
                      <h3
                        className={cn("truncate", typography.body.size, typography.h3.weight)}
                        style={hasGrad ? { color: textColor } : {}}
                      >
                        {character.name}
                      </h3>
                      <p
                        className={cn(typography.bodySmall.size, "line-clamp-1")}
                        style={hasGrad ? { color: textSecondary } : {}}
                      >
                        {descriptionPreview}
                      </p>
                    </div>

                    {/* Actions */}
                    <div className="flex shrink-0 items-center gap-2">
                      <button
                        onClick={() => handleEditCharacter(character)}
                        className={cn(
                          "relative flex items-center justify-center",
                          radius.full,
                          "border border-fg/10 bg-fg/25 text-fg/70",
                          "transition-all hover:border-fg/50 hover:text-fg",
                        )}
                        aria-label={t("characters.actions.editCharacter")}
                      >
                        <Edit2 size={12} />
                      </button>
                      <button
                        onClick={() => {
                          setSelectedCharacter(character);
                          setShowDeleteConfirm(true);
                        }}
                        className={cn(
                          "relative flex items-center justify-center",
                          radius.full,
                          "border border-danger/30 bg-danger/70 text-danger/80",
                          "transition-all hover:border-danger/90 hover:bg-danger/20",
                        )}
                        aria-label={t("characters.actions.deleteCharacter")}
                      >
                        <Trash2 size={12} />
                      </button>
                    </div>
                  </motion.div>
                );
              })}
            </AnimatePresence>
          </div>
        )}
      </main>

      {/* Delete Confirmation */}
      <BottomMenu
        isOpen={showDeleteConfirm}
        onClose={() => setShowDeleteConfirm(false)}
        title={t("characters.delete.title")}
      >
        <div className="space-y-4">
          <p className="text-sm text-fg/70">
            {t("characters.delete.confirmMessage", { name: selectedCharacter?.name ?? "" })}
          </p>
          <div className="flex gap-3">
            <button
              onClick={() => setShowDeleteConfirm(false)}
              disabled={deleting}
              className="flex-1 rounded-xl border border-fg/10 bg-fg/5 py-3 text-sm font-medium text-fg transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50"
            >
              {t("common.buttons.cancel")}
            </button>
            <button
              onClick={() => void handleDelete()}
              disabled={deleting}
              className="flex-1 rounded-xl border border-danger/30 bg-danger/20 py-3 text-sm font-medium text-danger/80 transition hover:bg-danger/30 disabled:opacity-50"
            >
              {deleting ? t("common.buttons.deleting") : t("common.buttons.delete")}
            </button>
          </div>
        </div>
      </BottomMenu>

      <NoModelMenu isOpen={showNoModel} onClose={() => setShowNoModel(false)} />
    </div>
  );
}
