import { useState, useRef, useEffect, useMemo } from "react";
import { motion } from "framer-motion";
import { Ban, BookOpen, Check, Copy, Image as ImageIcon, PenLine, Upload } from "lucide-react";
import { useI18n } from "../../../../../core/i18n/context";
import { insertSceneImageToken, storeSceneImageFromFile } from "../../../../../core/scene/inlineImages";
import { typography, radius, spacing, interactive, cn } from "../../../../design-tokens";
import type { Character } from "../../../../../core/storage/schemas";
import { getPlatform } from "../../../../../core/utils/platform";
import { AvatarImage } from "../../../../components/AvatarImage";
import { useAvatar } from "../../../../hooks/useAvatar";
import { FieldLabel, OptionRow } from "../OptionRow";

interface AvailableScene {
  characterId: string;
  characterName: string;
  sceneId: string;
  content: string;
}

function SceneCard({
  scene,
  character,
  sceneNumber,
  selected,
  onSelect,
  onCopy,
}: {
  scene: AvailableScene;
  character: Character | undefined;
  sceneNumber: number;
  selected: boolean;
  onSelect: () => void;
  onCopy: () => void;
}) {
  const { t } = useI18n();
  const avatarUrl = useAvatar("character", character?.id, character?.avatarPath, "round");

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={onSelect}
      onKeyDown={(e) => {
        if (e.key === "Enter" || e.key === " ") {
          e.preventDefault();
          onSelect();
        }
      }}
      className={cn(
        "w-full cursor-pointer p-3 text-left",
        radius.lg,
        "border",
        interactive.transition.fast,
        selected
          ? "border-accent/40 bg-accent/10"
          : "border-fg/10 bg-surface-el/85 hover:border-fg/20",
      )}
    >
      <div className="flex items-center gap-2.5">
        <div className="h-8 w-8 shrink-0 overflow-hidden rounded-full bg-linear-to-br from-fg/8 to-fg/4 ring-1 ring-fg/10">
          {avatarUrl ? (
            <AvatarImage
              src={avatarUrl}
              alt={scene.characterName}
              crop={character?.avatarCrop}
              applyCrop
            />
          ) : (
            <div className="flex h-full w-full items-center justify-center text-[10px] font-bold text-fg/60">
              {scene.characterName.slice(0, 2).toUpperCase()}
            </div>
          )}
        </div>
        <div className="min-w-0 flex-1">
          <p
            className={cn(
              "truncate text-sm font-medium",
              selected ? "text-accent" : "text-fg",
            )}
          >
            {scene.characterName}
          </p>
          <p className={cn(typography.caption.size, "mt-0.5 text-fg/40")}>
            {t("groupChats.create.startingScene.sceneNumber", { number: String(sceneNumber) })}
          </p>
        </div>
        <button
          onClick={(e) => {
            e.stopPropagation();
            onCopy();
          }}
          title={t("groupChats.create.startingScene.copyToCustom")}
          aria-label={t("groupChats.create.startingScene.copyToCustom")}
          className={cn(
            "flex h-7 w-7 shrink-0 items-center justify-center rounded-full text-fg/50",
            interactive.transition.fast,
            "hover:bg-fg/10 hover:text-fg",
          )}
        >
          <Copy size={13} />
        </button>
        <span
          className={cn(
            "flex h-4 w-4 shrink-0 items-center justify-center rounded-full border",
            interactive.transition.fast,
            selected ? "border-accent" : "border-fg/25",
          )}
        >
          {selected && <span className="h-2 w-2 rounded-full bg-accent" />}
        </span>
      </div>
      <p className="mt-2 line-clamp-3 text-xs leading-relaxed text-fg/55">{scene.content}</p>
    </div>
  );
}

interface GroupStartingSceneStepProps {
  sceneSource: "none" | "custom" | "character";
  onSceneSourceChange: (value: "none" | "custom" | "character") => void;
  customScene: string;
  onCustomSceneChange: (value: string) => void;
  selectedCharacterSceneId: string | null;
  onSelectedCharacterSceneIdChange: (value: string | null) => void;
  availableScenes: AvailableScene[];
  selectedCharacters: Character[];
}

export function GroupStartingSceneStep({
  sceneSource,
  onSceneSourceChange,
  customScene,
  onCustomSceneChange,
  selectedCharacterSceneId,
  onSelectedCharacterSceneIdChange,
  availableScenes,
  selectedCharacters,
}: GroupStartingSceneStepProps) {
  const { t } = useI18n();
  const isDesktop = useMemo(() => getPlatform().type === "desktop", []);
  const [showAutocomplete, setShowAutocomplete] = useState(false);
  const [autocompleteSearch, setAutocompleteSearch] = useState("");
  const [autocompleteIndex, setAutocompleteIndex] = useState(0);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const inlineImageInputRef = useRef<HTMLInputElement>(null);

  const filteredCharacters = selectedCharacters.filter((c) =>
    c.name.toLowerCase().includes(autocompleteSearch.toLowerCase()),
  );

  const handleSceneInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value;
    const cursorPos = e.target.selectionStart;

    onCustomSceneChange(value);

    const textBeforeCursor = value.substring(0, cursorPos);
    const match = textBeforeCursor.match(/\{\{@"([^"]*?)$/);

    if (match && selectedCharacters.length > 0) {
      const searchTerm = match[1];
      setAutocompleteSearch(searchTerm);
      setAutocompleteIndex(0);
      setShowAutocomplete(true);
    } else {
      setShowAutocomplete(false);
    }
  };

  const handleSceneKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (!showAutocomplete) return;

    if (e.key === "ArrowDown") {
      e.preventDefault();
      setAutocompleteIndex((prev) => (prev + 1) % filteredCharacters.length);
    } else if (e.key === "ArrowUp") {
      e.preventDefault();
      setAutocompleteIndex(
        (prev) => (prev - 1 + filteredCharacters.length) % filteredCharacters.length,
      );
    } else if (e.key === "Enter" && filteredCharacters.length > 0) {
      e.preventDefault();
      insertCharacterName(filteredCharacters[autocompleteIndex].name);
    } else if (e.key === "Escape") {
      e.preventDefault();
      setShowAutocomplete(false);
    }
  };

  const insertCharacterName = (characterName: string) => {
    if (!textareaRef.current) return;

    const textarea = textareaRef.current;
    const cursorPos = textarea.selectionStart;
    const textBeforeCursor = customScene.substring(0, cursorPos);
    const textAfterCursor = customScene.substring(cursorPos);

    const match = textBeforeCursor.match(/\{\{@"([^"]*?)$/);
    if (!match) return;

    const trailingToken = textAfterCursor.match(/^[^"{}\s]*(?:"\}\})?/);
    const remainder = textAfterCursor.slice(trailingToken ? trailingToken[0].length : 0);

    const replaceStart = cursorPos - match[0].length + 4;
    const newText = customScene.substring(0, replaceStart) + characterName + '"}}' + remainder;

    onCustomSceneChange(newText);
    setShowAutocomplete(false);

    setTimeout(() => {
      const newCursorPos = replaceStart + characterName.length + 3;
      textarea.setSelectionRange(newCursorPos, newCursorPos);
      textarea.focus();
    }, 0);
  };

  const handleInlineImageUpload = async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    const input = event.target;
    if (!file) return;
    const cursor = textareaRef.current?.selectionStart ?? customScene.length;
    try {
      const stored = await storeSceneImageFromFile(file);
      if (stored) {
        const { content, nextCursor } = insertSceneImageToken(
          customScene,
          cursor,
          stored.imageId,
          stored.ext,
        );
        onCustomSceneChange(content);
        setTimeout(() => {
          const el = textareaRef.current;
          if (el) {
            el.setSelectionRange(nextCursor, nextCursor);
            el.focus();
          }
        }, 0);
      }
    } catch (error) {
      console.warn("GroupStartingSceneStep: failed to store inline scene image", error);
    } finally {
      input.value = "";
    }
  };

  useEffect(() => {
    const handleClickOutside = () => {
      if (showAutocomplete) {
        setShowAutocomplete(false);
      }
    };

    document.addEventListener("click", handleClickOutside);
    return () => document.removeEventListener("click", handleClickOutside);
  }, [showAutocomplete]);

  const handleCopyToCustom = (content: string) => {
    onSceneSourceChange("custom");
    onCustomSceneChange(content);
    onSelectedCharacterSceneIdChange(null);
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -16 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className="grid items-start gap-6 lg:grid-cols-[minmax(0,2fr)_minmax(0,3fr)] lg:gap-x-10"
    >
      <div className={spacing.section}>
        <div>
          <h1 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
            {t("groupChats.create.startingScene.title")}
          </h1>
          <p className={cn(typography.body.size, "mt-1 text-fg/50")}>
            {t("groupChats.create.startingScene.subtitle")}
          </p>
        </div>

        <div className={spacing.field}>
          <FieldLabel>{t("groupChats.create.startingScene.sceneSource")}</FieldLabel>
          <div className="space-y-2">
            <OptionRow
              selected={sceneSource === "none"}
              onSelect={() => onSceneSourceChange("none")}
              icon={Ban}
              label={t("groupChats.create.startingScene.none")}
              description={t("groupChats.create.startingScene.noneDesc")}
            />
            <OptionRow
              selected={sceneSource === "custom"}
              onSelect={() => onSceneSourceChange("custom")}
              icon={PenLine}
              label={t("groupChats.create.startingScene.custom")}
              description={t("groupChats.create.startingScene.customDesc")}
            />
            <OptionRow
              selected={sceneSource === "character"}
              onSelect={() => onSceneSourceChange("character")}
              icon={BookOpen}
              label={t("groupChats.create.startingScene.fromCharacter")}
              description={t("groupChats.create.startingScene.fromCharacterDesc")}
              disabled={availableScenes.length === 0}
            />
          </div>
        </div>
      </div>

      {sceneSource === "custom" && (
        <div className={spacing.field}>
          <FieldLabel>{t("groupChats.create.startingScene.sceneContent")}</FieldLabel>

          <div className="relative">
            {showAutocomplete && filteredCharacters.length > 0 && (
              <div
                className={cn(
                  "absolute left-0 right-0 z-50 max-h-48 overflow-y-auto",
                  isDesktop ? "top-full mt-1" : "bottom-full mb-1",
                  radius.lg,
                  "border border-fg/15 bg-surface-el shadow-lg",
                )}
                onClick={(e) => e.stopPropagation()}
              >
                {filteredCharacters.map((character, index) => (
                  <button
                    key={character.id}
                    onClick={() => insertCharacterName(character.name)}
                    className={cn(
                      "flex w-full items-center gap-2 px-3 py-2.5 text-left",
                      "hover:bg-fg/10",
                      interactive.transition.fast,
                      index === autocompleteIndex && "bg-fg/15",
                    )}
                  >
                    <div className="flex-1">
                      <div className="text-sm font-medium text-fg">{character.name}</div>
                      <div className="text-xs text-fg/50">{`{{@"${character.name}"}}`}</div>
                    </div>
                    {index === autocompleteIndex && <Check size={14} className="text-accent" />}
                  </button>
                ))}
              </div>
            )}

            <textarea
              ref={textareaRef}
              value={customScene}
              onChange={handleSceneInput}
              onKeyDown={handleSceneKeyDown}
              placeholder={t("groupChats.create.startingScene.sceneContentPlaceholder")}
              rows={8}
              className={cn(
                "w-full resize-none px-4 py-3",
                radius.lg,
                "border border-fg/10 bg-surface-el/85",
                typography.body.size,
                "text-fg placeholder:text-fg/30",
                interactive.transition.fast,
                "focus:border-fg/30 focus:outline-none",
              )}
            />
          </div>

          <input
            ref={inlineImageInputRef}
            type="file"
            accept="image/*"
            className="hidden"
            onChange={(event) => {
              void handleInlineImageUpload(event);
            }}
          />
          <div className="flex items-center gap-2 rounded-xl border border-fg/10 bg-fg/5 px-3 py-2.5">
            <ImageIcon className="h-3.5 w-3.5 text-fg/50" />
            <span className="text-xs font-medium text-fg/70">{t("sceneImage.add")}</span>
            <button
              type="button"
              onClick={() => inlineImageInputRef.current?.click()}
              className="ml-auto flex items-center gap-1.5 rounded-lg border border-accent/30 bg-accent/10 px-2.5 py-1.5 text-[11px] font-medium text-accent/90 transition active:scale-95 active:bg-accent/20"
            >
              <Upload className="h-3.5 w-3.5" />
              {t("sceneImage.upload")}
            </button>
          </div>
          <p className={cn(typography.bodySmall.size, "text-fg/40")}>
            {t("sceneImage.hint")}
          </p>

          <p className={cn(typography.bodySmall.size, "text-fg/40")}>
            {t("groupChats.create.startingScene.sceneReferenceTip")}
          </p>
        </div>
      )}

      {sceneSource === "character" && availableScenes.length > 0 && (
        <div className={spacing.field}>
          <FieldLabel>{t("groupChats.create.startingScene.selectScene")}</FieldLabel>
          <div className="space-y-2">
            {availableScenes.map((scene) => {
              const sceneNumber =
                availableScenes
                  .filter((s) => s.characterId === scene.characterId)
                  .findIndex((s) => s.sceneId === scene.sceneId) + 1;
              return (
                <SceneCard
                  key={scene.sceneId}
                  scene={scene}
                  character={selectedCharacters.find((c) => c.id === scene.characterId)}
                  sceneNumber={sceneNumber}
                  selected={selectedCharacterSceneId === scene.sceneId}
                  onSelect={() => onSelectedCharacterSceneIdChange(scene.sceneId)}
                  onCopy={() => handleCopyToCustom(scene.content)}
                />
              );
            })}
          </div>
        </div>
      )}
    </motion.div>
  );
}
