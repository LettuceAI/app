import { Camera, Check, Clock3, FolderOpen, MessageSquareText, Upload, X } from "lucide-react";
import type { ChangeEvent, RefObject } from "react";
import type { Lorebook } from "../../core/storage/schemas";
import { useI18n, type TranslationKey } from "../../core/i18n/context";
import { BottomMenu } from "./BottomMenu";
import { LorebookAvatar } from "./LorebookAvatar";

type LorebookMetadataMenuProps = {
  isOpen: boolean;
  onClose: () => void;
  title: string;
  nameValue: string;
  previewName: string;
  namePlaceholder: string;
  avatarPath: string | null;
  avatarInputRef: RefObject<HTMLInputElement | null>;
  onNameChange: (value: string) => void;
  onNameSubmit: () => void;
  onAvatarFileChange: (event: ChangeEvent<HTMLInputElement>) => void;
  onAvatarRemove: () => void;
  onChooseFromLibrary?: () => void;
  keywordDetectionMode: Lorebook["keywordDetectionMode"];
  onKeywordDetectionModeChange: (mode: Lorebook["keywordDetectionMode"]) => void;
  onSave: () => void;
  saveDisabled: boolean;
};

const detectionModes: Array<{
  value: Lorebook["keywordDetectionMode"];
  icon: typeof Clock3;
  titleKey: TranslationKey;
  descriptionKey: TranslationKey;
}> = [
  {
    value: "recentMessageWindow",
    icon: Clock3,
    titleKey: "characters.lorebook.keywordDetectionRecentWindow",
    descriptionKey: "characters.lorebook.keywordDetectionRecentWindowDesc",
  },
  {
    value: "latestUserMessage",
    icon: MessageSquareText,
    titleKey: "characters.lorebook.keywordDetectionLatestUser",
    descriptionKey: "characters.lorebook.keywordDetectionLatestUserDesc",
  },
];

export function LorebookMetadataMenu({
  isOpen,
  onClose,
  title,
  nameValue,
  previewName,
  namePlaceholder,
  avatarPath,
  avatarInputRef,
  onNameChange,
  onNameSubmit,
  onAvatarFileChange,
  onAvatarRemove,
  onChooseFromLibrary,
  keywordDetectionMode,
  onKeywordDetectionModeChange,
  onSave,
  saveDisabled,
}: LorebookMetadataMenuProps) {
  const { t } = useI18n();
  const activeDetectionMode =
    detectionModes.find((mode) => mode.value === keywordDetectionMode) ?? detectionModes[0];
  const displayName = previewName.trim() || namePlaceholder;

  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={title}>
      <div className="space-y-6">
        <div className="flex items-start gap-4">
          <div className="relative shrink-0">
            <button
              type="button"
              onClick={() => avatarInputRef.current?.click()}
              className={`group relative flex h-24 w-24 items-center justify-center overflow-hidden rounded-2xl bg-[#111113] transition ${
                avatarPath
                  ? "border-[3px] border-white/10 hover:border-white/20"
                  : "border-2 border-dashed border-white/15 hover:border-white/25"
              }`}
              aria-label={t("common.buttons.edit")}
            >
              <LorebookAvatar
                avatarPath={avatarPath}
                name={displayName}
                className="text-[28px]"
                fallbackClassName="bg-transparent text-white/30"
              />
              <span className="pointer-events-none absolute inset-0 flex items-center justify-center bg-black/50 opacity-0 transition group-hover:opacity-100">
                <Camera size={18} className="text-white" />
              </span>
            </button>

            {avatarPath && (
              <button
                type="button"
                onClick={onAvatarRemove}
                className="absolute -left-1.5 -top-1.5 z-10 flex h-7 w-7 items-center justify-center rounded-full border border-fg/10 bg-surface-el text-fg/60 transition hover:border-danger/50 hover:bg-danger/80 hover:text-fg active:scale-95"
                aria-label={t("common.buttons.remove")}
              >
                <X size={12} strokeWidth={2.5} />
              </button>
            )}
          </div>

          <div className="min-w-0 flex-1 space-y-2">
            <label className="text-[12px] font-medium uppercase tracking-wide text-fg/70">
              {t("characters.lorebook.nameLabel")}
            </label>
            <input
              value={nameValue}
              onChange={(event) => onNameChange(event.target.value)}
              onKeyDown={(event) => event.key === "Enter" && onNameSubmit()}
              placeholder={namePlaceholder}
              autoFocus
              className={`w-full rounded-md border bg-surface-el/20 px-3.5 py-2.5 text-sm text-fg placeholder-fg/40 backdrop-blur-xl transition focus:border-fg/30 focus:bg-surface-el/30 focus:outline-none ${
                nameValue.trim() ? "border-accent/30 bg-accent/5" : "border-fg/10"
              }`}
            />
            <div className="flex flex-wrap gap-1.5">
              <button
                type="button"
                onClick={() => avatarInputRef.current?.click()}
                className="inline-flex items-center gap-1.5 rounded-md border border-fg/10 bg-fg/[0.04] px-2.5 py-1.5 text-[11px] font-medium text-fg/75 transition hover:border-fg/20 hover:bg-fg/[0.08] hover:text-fg active:scale-[0.97]"
              >
                <Upload size={11} strokeWidth={2.5} />
                <span>{t("components.extra.uploadButton")}</span>
              </button>
              {onChooseFromLibrary && (
                <button
                  type="button"
                  onClick={onChooseFromLibrary}
                  className="inline-flex items-center gap-1.5 rounded-md border border-fg/10 bg-fg/[0.04] px-2.5 py-1.5 text-[11px] font-medium text-fg/75 transition hover:border-fg/20 hover:bg-fg/[0.08] hover:text-fg active:scale-[0.97]"
                >
                  <FolderOpen size={11} strokeWidth={2.5} />
                  <span>{t("components.extra.libraryButton")}</span>
                </button>
              )}
            </div>
          </div>

          <input
            ref={avatarInputRef}
            type="file"
            accept="image/*"
            onChange={onAvatarFileChange}
            className="hidden"
          />
        </div>

        <div className="space-y-2">
          <label className="text-[12px] font-medium uppercase tracking-wide text-fg/70">
            {t("characters.lorebook.keywordDetectionMode")}
          </label>

          <div className="grid grid-cols-2 gap-1.5 rounded-xl border border-fg/10 bg-fg/[0.04] p-1.5">
            {detectionModes.map((mode) => {
              const Icon = mode.icon;
              const selected = keywordDetectionMode === mode.value;
              return (
                <button
                  key={mode.value}
                  type="button"
                  onClick={() => onKeywordDetectionModeChange(mode.value)}
                  className={`flex items-center justify-center gap-2 rounded-lg px-3 py-2.5 text-xs font-medium transition ${
                    selected
                      ? "bg-accent/15 text-accent shadow-sm"
                      : "text-fg/60 hover:bg-fg/5 hover:text-fg/80"
                  }`}
                >
                  <Icon size={14} />
                  <span className="truncate">{t(mode.titleKey)}</span>
                </button>
              );
            })}
          </div>

          <div className="flex items-start gap-2.5 rounded-xl border border-accent/15 bg-accent/[0.06] px-3.5 py-3">
            <div className="mt-0.5 flex h-5 w-5 shrink-0 items-center justify-center rounded-full bg-accent/20 text-accent">
              <Check size={12} strokeWidth={3} />
            </div>
            <p className="text-xs leading-relaxed text-fg/70">
              {t(activeDetectionMode.descriptionKey)}
            </p>
          </div>
        </div>

        <button
          type="button"
          onClick={onSave}
          disabled={saveDisabled}
          className="w-full rounded-md border border-accent/40 bg-accent/15 px-4 py-3.5 text-sm font-semibold text-accent transition hover:border-accent/50 hover:bg-accent/20 active:scale-[0.98] disabled:cursor-not-allowed disabled:border-fg/10 disabled:bg-fg/5 disabled:text-fg/40 disabled:active:scale-100"
        >
          {t("common.buttons.save")}
        </button>
      </div>
    </BottomMenu>
  );
}
