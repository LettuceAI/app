import { motion, AnimatePresence } from "framer-motion";
import { X, Camera, Image, Upload, Sparkles, Loader2, AlertCircle, FolderOpen } from "lucide-react";
import { typography, radius, spacing, interactive, shadows, cn } from "../../../design-tokens";
import { AvatarPicker } from "../../../components/AvatarPicker";
import { Switch } from "../../../components/Switch";
import type { AvatarCrop, CharacterCardType } from "../../../../core/storage/schemas";
import { useI18n } from "../../../../core/i18n/context";

interface IdentityStepProps {
  name: string;
  onNameChange: (value: string) => void;
  avatarPath: string;
  onAvatarChange: (value: string) => void;
  onBeforeChooseAvatarFromLibrary?: () => void;
  avatarCrop?: AvatarCrop | null;
  onAvatarCropChange?: (value: AvatarCrop | null) => void;
  avatarRoundPath?: string | null;
  onAvatarRoundChange?: (value: string | null) => void;
  bannerPath?: string | null;
  onBannerChange?: (value: string | null) => void;
  bannerCrop?: AvatarCrop | null;
  onBannerCropChange?: (value: AvatarCrop | null) => void;
  cardType: CharacterCardType;
  onCardTypeChange: (value: CharacterCardType) => void;
  backgroundImagePath: string;
  onBackgroundImageChange: (value: string) => void;
  onBackgroundImageUpload: (event: React.ChangeEvent<HTMLInputElement>) => void;
  onChooseBackgroundFromLibrary: () => void;
  disableAvatarGradient: boolean;
  onDisableAvatarGradientChange: (value: boolean) => void;
  onContinue: () => void;
  canContinue: boolean;
  importingAvatar?: boolean;
  avatarImportError?: string | null;
  onImport?: (event: React.ChangeEvent<HTMLInputElement>) => void;
}

export function IdentityStep({
  name,
  onNameChange,
  avatarPath,
  onAvatarChange,
  onBeforeChooseAvatarFromLibrary,
  avatarCrop,
  onAvatarCropChange,
  avatarRoundPath,
  onAvatarRoundChange,
  bannerPath,
  onBannerChange,
  bannerCrop,
  onBannerCropChange,
  cardType,
  onCardTypeChange,
  backgroundImagePath,
  onBackgroundImageChange,
  onBackgroundImageUpload,
  onChooseBackgroundFromLibrary,
  disableAvatarGradient,
  onDisableAvatarGradientChange,
  onContinue,
  canContinue,
  importingAvatar = false,
  avatarImportError = null,
  onImport,
}: IdentityStepProps) {
  const { t } = useI18n();

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -16 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className={spacing.section}
    >
      {/* Title */}
      <div className={spacing.tight}>
        <h2 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
          {t("characters.identity.title")}
        </h2>
        <p className={cn(typography.body.size, "text-fg/50")}>
          {t("characters.identity.subtitle")}
        </p>
      </div>

      {/* Desktop: Two-column layout / Mobile: stacked */}
      <div className="flex flex-col gap-6 lg:gap-8">
        <div className="grid gap-6 lg:grid-cols-[minmax(0,320px)_minmax(0,1fr)] lg:items-start">
          <div className="space-y-5 py-4 lg:shrink-0">
            <div className="flex flex-col items-center">
              <div className="relative">
                <AvatarPicker
                  currentAvatarPath={avatarPath}
                  onAvatarChange={onAvatarChange}
                  onBeforeChooseFromLibrary={onBeforeChooseAvatarFromLibrary}
                  promptSubjectName={name}
                  avatarCrop={avatarCrop}
                  onAvatarCropChange={onAvatarCropChange}
                  avatarRoundPath={avatarRoundPath}
                  onAvatarRoundChange={onAvatarRoundChange}
                  avatarPreview={
                    importingAvatar ? (
                      <div className="flex h-full w-full items-center justify-center">
                        <Loader2 className="animate-spin text-fg/50" size={34} />
                      </div>
                    ) : avatarPath ? undefined : (
                      <div className="flex h-full w-full items-center justify-center">
                        <Camera className="text-fg/30" size={36} />
                      </div>
                    )
                  }
                />

                {avatarPath && (
                  <button
                    onClick={() => {
                      onAvatarChange("");
                      onAvatarCropChange?.(null);
                      onAvatarRoundChange?.(null);
                      onBannerChange?.(null);
                      onBannerCropChange?.(null);
                    }}
                    className="absolute -top-1 -left-1 z-30 flex h-12 w-12 items-center justify-center rounded-full border border-fg/10 bg-surface-el text-fg/60 transition hover:bg-danger/80 hover:border-danger/50 hover:text-fg active:scale-95"
                  >
                    <X size={14} strokeWidth={2.5} />
                  </button>
                )}
              </div>
              <p className="mt-3 text-xs text-fg/40">{t("characters.identity.tapCameraToAdd")}</p>
              {importingAvatar && (
                <p className="mt-1 text-xs text-accent/80">
                  {t("characters.identity.importingAvatar")}
                </p>
              )}
              {avatarImportError && (
                <div className="mt-2 flex items-start gap-1.5 text-xs text-danger">
                  <AlertCircle size={12} className="mt-0.5 shrink-0" />
                  <span className="max-w-[320px] whitespace-pre-line leading-4">
                    {avatarImportError}
                  </span>
                </div>
              )}
            </div>

            <div className="space-y-3">
              <div>
                <p className={cn(typography.label.size, typography.label.weight, "text-fg/70")}>
                  {t("characters.identity.cardTypeLabel")}
                </p>
                <div className="mt-2 grid grid-cols-2 gap-2">
                  {(["circle", "banner"] as const).map((value) => (
                    <button
                      key={value}
                      type="button"
                      onClick={() => onCardTypeChange(value)}
                      className={cn(
                        "rounded-xl border px-3 py-2 text-sm font-medium transition",
                        cardType === value
                          ? "border-accent/50 bg-accent/10 text-fg"
                          : "border-fg/10 bg-surface-el/20 text-fg/60 hover:border-fg/20 hover:text-fg",
                      )}
                    >
                      {value === "circle"
                        ? t("characters.identity.cardTypeCircle")
                        : t("characters.identity.cardTypeBanner")}
                    </button>
                  ))}
                </div>
              </div>

              <div className="rounded-2xl border border-fg/10 bg-surface-el/10 p-4">
                <div className="mb-3">
                  <p className="text-sm font-medium text-fg">{t("characters.identity.bannerImageLabel")}</p>
                  <p className="text-xs text-fg/45">
                    {t("characters.identity.bannerImageHint")}
                  </p>
                </div>

                <div className="flex flex-col items-center gap-3">
                  <AvatarPicker
                    currentAvatarPath={bannerPath || ""}
                    onAvatarChange={(value) => onBannerChange?.(value)}
                    librarySelectionScope="character-banner"
                    onBeforeChooseFromLibrary={onBeforeChooseAvatarFromLibrary}
                    promptSubjectName={name}
                    avatarCrop={bannerCrop}
                    onAvatarCropChange={onBannerCropChange}
                    shape="banner"
                    size="md"
                    avatarPreview={
                      bannerPath ? undefined : (
                        <div className="flex h-full w-full items-center justify-center">
                          <Image className="text-fg/30" size={24} />
                        </div>
                      )
                    }
                  />
                  {bannerPath ? (
                    <button
                      type="button"
                      onClick={() => {
                        onBannerChange?.(null);
                        onBannerCropChange?.(null);
                      }}
                      className="text-xs text-fg/45 transition hover:text-danger"
                    >
                      {t("characters.identity.bannerImageRemove")}
                    </button>
                  ) : null}
                </div>
              </div>
            </div>
          </div>

          <div className="lg:flex-1 space-y-6">
          {/* Name Input */}
          <div className={spacing.field}>
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              {t("characters.identity.characterName")}
            </label>
            <div className="relative">
              <input
                value={name}
                onChange={(e) => onNameChange(e.target.value)}
                placeholder={t("characters.identity.characterNamePlaceholder")}
                inputMode="text"
                className={cn(
                  "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
                  radius.md,
                  typography.body.size,
                  interactive.transition.default,
                  "focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none",
                  name.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10",
                )}
              />
              {name.trim() && (
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
                    <Sparkles className="h-3 w-3 text-accent/80" />
                  </div>
                </motion.div>
              )}
            </div>
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              {t("characters.identity.characterNameDesc")}
            </p>
          </div>

          {/* Avatar Gradient Toggle (only show if avatar exists) */}
          <AnimatePresence>
            {avatarPath && (
              <motion.div
                initial={{ opacity: 0, height: 0 }}
                animate={{ opacity: 1, height: "auto" }}
                exit={{ opacity: 0, height: 0 }}
                className={spacing.field}
              >
                <div
                  className={cn(
                    "flex items-center justify-between border border-fg/10 bg-surface-el/20 px-4 py-3",
                    radius.md,
                    interactive.transition.default,
                    "active:bg-surface-el/30",
                  )}
                >
                  <div className="flex-1">
                    <div className="flex items-center gap-2">
                      <Sparkles className="h-4 w-4 text-accent" />
                      <span className={cn(typography.body.size, "font-medium text-fg")}>
                        {t("characters.identity.avatarGradient")}
                      </span>
                    </div>
                    <p className={cn(typography.bodySmall.size, "mt-1 text-fg/40")}>
                      {t("characters.identity.avatarGradientDesc")}
                    </p>
                  </div>
                  <div className="ml-3">
                    <Switch
                      checked={!disableAvatarGradient}
                      onChange={(next) => onDisableAvatarGradientChange(!next)}
                    />
                  </div>
                </div>
              </motion.div>
            )}
          </AnimatePresence>

          <div className={spacing.field}>
            <label
              className={cn(
                typography.label.size,
                typography.label.weight,
                typography.label.tracking,
                "uppercase text-fg/70",
              )}
            >
              {t("characters.identity.chatBackgroundLabel")}{" "}
              <span className="text-fg/40">{t("characters.identity.optionalSuffix")}</span>
            </label>
            <div
              className={cn(
                "overflow-hidden border",
                radius.md,
                backgroundImagePath
                  ? "border-secondary/30 bg-secondary/5"
                  : "border-fg/10 bg-surface-el/20",
              )}
            >
              {backgroundImagePath ? (
                <div className="relative">
                  <img
                    src={backgroundImagePath}
                    alt={t("characters.identity.backgroundPreviewAlt")}
                    className="h-24 w-full object-cover"
                  />
                  <div className="absolute inset-0 bg-surface-el/30 flex items-center justify-center">
                    <span
                      className={cn(
                        typography.caption.size,
                        "text-fg/80 bg-surface-el/50 px-2 py-1",
                        radius.sm,
                      )}
                    >
                      {t("characters.identity.backgroundPreviewBadge")}
                    </span>
                  </div>
                  <button
                    onClick={() => onBackgroundImageChange("")}
                    className={cn(
                      "absolute top-2 right-2 flex h-6 w-6 items-center justify-center border border-fg/20 bg-surface-el/50 text-fg/70",
                      radius.full,
                      interactive.transition.fast,
                      "active:scale-95 active:bg-surface-el/70",
                    )}
                  >
                    <X size={12} />
                  </button>
                </div>
              ) : (
                <div className={cn("flex h-24 flex-col items-center justify-center gap-2")}>
                  <div
                    className={cn(
                      "flex h-8 w-8 items-center justify-center border border-fg/10 bg-fg/5",
                      radius.md,
                    )}
                  >
                    <Image size={16} className="text-fg/40" />
                  </div>
                  <div className="text-center">
                    <p className={cn(typography.bodySmall.size, "text-fg/70")}>
                      {t("characters.identity.addBackground")}
                    </p>
                    <p className={cn(typography.caption.size, "text-fg/40")}>
                      {t("characters.identity.addBackgroundHint")}
                    </p>
                  </div>
                </div>
              )}
            </div>
            <div className="grid grid-cols-2 gap-2">
              <label
                className={cn(
                  "flex cursor-pointer items-center justify-center gap-2 border bg-surface-el/20 px-3 py-3 text-sm text-fg/75 backdrop-blur-xl",
                  radius.md,
                  interactive.transition.default,
                  "border-fg/10 hover:bg-surface-el/30",
                )}
              >
                <Upload size={14} />
                {t("characters.identity.uploadImage")}
                <input
                  type="file"
                  accept="image/*"
                  onChange={onBackgroundImageUpload}
                  className="hidden"
                />
              </label>
              <button
                type="button"
                onClick={onChooseBackgroundFromLibrary}
                className={cn(
                  "flex items-center justify-center gap-2 border bg-surface-el/20 px-3 py-3 text-sm text-fg/75 backdrop-blur-xl",
                  radius.md,
                  interactive.transition.default,
                  "border-fg/10 hover:bg-surface-el/30",
                )}
              >
                <FolderOpen size={14} />
                {t("characters.identity.chooseFromLibrary")}
              </button>
            </div>
            <p className={cn(typography.bodySmall.size, "text-fg/40")}>
              {t("characters.identity.backgroundDesc")}
            </p>
          </div>
          </div>
        </div>
      </div>

      {/* Continue Button */}
      <div className="pt-2">
        <motion.button
          disabled={!canContinue}
          onClick={onContinue}
          whileTap={{ scale: canContinue ? 0.97 : 1 }}
          className={cn(
            "w-full py-4 text-base font-semibold",
            radius.md,
            interactive.transition.fast,
            canContinue
              ? cn(
                  "border border-accent/40 bg-accent/20 text-accent",
                  shadows.glow,
                  "active:border-accent/60 active:bg-accent/30",
                )
              : "cursor-not-allowed border border-fg/5 bg-fg/5 text-fg/30",
          )}
        >
          {t("characters.identity.continueToDescription")}
        </motion.button>
      </div>

      {/* Import Character Button */}
      {onImport && (
        <div className="pt-2">
          <label
            className={cn(
              "flex w-full cursor-pointer items-center justify-center gap-2 border border-info/40 bg-info/20 py-3.5 text-sm font-semibold text-info",
              radius.md,
              interactive.transition.fast,
              "active:scale-[0.97] active:bg-info/30",
            )}
          >
            <Upload className="h-4 w-4" />
            {t("characters.identity.importCharacterFromFile")}
            <input type="file" onChange={onImport} className="hidden" />
          </label>
          <p className={cn(typography.bodySmall.size, "mt-2 text-center text-fg/40")}>
            {t("characters.identity.importCharacterDesc")}
          </p>
        </div>
      )}
    </motion.div>
  );
}
