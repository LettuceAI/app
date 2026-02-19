import { useState, useRef, useEffect } from "react";
import { motion } from "framer-motion";
import { BookOpen, Check, Copy } from "lucide-react";
import { typography, radius, spacing, interactive, shadows, cn } from "../../../../design-tokens";
import type { Character } from "../../../../../core/storage/schemas";

interface AvailableScene {
  characterId: string;
  characterName: string;
  sceneId: string;
  content: string;
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
  onCreateGroup: () => void;
  canCreate: boolean;
  creating: boolean;
  error: string | null;
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
  onCreateGroup,
  canCreate,
  creating,
  error,
}: GroupStartingSceneStepProps) {
  // Character autocomplete state
  const [showAutocomplete, setShowAutocomplete] = useState(false);
  const [autocompleteSearch, setAutocompleteSearch] = useState("");
  const [autocompleteIndex, setAutocompleteIndex] = useState(0);
  const textareaRef = useRef<HTMLTextAreaElement>(null);

  // Filter characters based on search
  const filteredCharacters = selectedCharacters.filter((c) =>
    c.name.toLowerCase().includes(autocompleteSearch.toLowerCase()),
  );

  // Detect {{@" pattern and show autocomplete
  const handleSceneInput = (e: React.ChangeEvent<HTMLTextAreaElement>) => {
    const value = e.target.value;
    const cursorPos = e.target.selectionStart;

    onCustomSceneChange(value);

    // Look for {{@" pattern before cursor
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

  // Handle keyboard navigation in autocomplete
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

  // Insert character name at cursor position
  const insertCharacterName = (characterName: string) => {
    if (!textareaRef.current) return;

    const textarea = textareaRef.current;
    const cursorPos = textarea.selectionStart;
    const textBeforeCursor = customScene.substring(0, cursorPos);
    const textAfterCursor = customScene.substring(cursorPos);

    // Find the {{@" pattern
    const match = textBeforeCursor.match(/\{\{@"([^"]*?)$/);
    if (!match) return;

    const replaceStart = cursorPos - match[0].length + 4; // After {{@"
    const newText =
      customScene.substring(0, replaceStart) + characterName + '"}}' + textAfterCursor;

    onCustomSceneChange(newText);
    setShowAutocomplete(false);

    // Set cursor after inserted text
    setTimeout(() => {
      const newCursorPos = replaceStart + characterName.length + 3;
      textarea.setSelectionRange(newCursorPos, newCursorPos);
      textarea.focus();
    }, 0);
  };

  // Close autocomplete when clicking outside
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
      className={cn(spacing.section, "flex flex-col")}
    >
      {/* Title */}
      <div className={spacing.tight}>
        <div className="flex items-center gap-2">
          <div className="rounded-lg border border-secondary/30 bg-secondary/10 p-1.5">
            <BookOpen className="h-4 w-4 text-secondary" />
          </div>
          <h2 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
            Starting Scene
          </h2>
        </div>
        <p className={cn(typography.body.size, "mt-2 text-fg/50")}>
          Set the opening scenario for your roleplay
        </p>
      </div>

      {/* Scene Source Selector */}
      <div className={spacing.field}>
        <label
          className={cn(
            typography.label.size,
            typography.label.weight,
            typography.label.tracking,
            "uppercase text-fg/70",
          )}
        >
          Scene Source
        </label>
        <div className="flex gap-2">
          <button
            onClick={() => onSceneSourceChange("none")}
            className={cn(
              "flex-1 py-2.5 px-3",
              radius.md,
              "border text-sm font-medium",
              interactive.transition.fast,
              sceneSource === "none"
                ? "border-accent/40 bg-accent/20 text-accent"
                : "border-fg/15 bg-fg/5 text-fg/70 active:bg-fg/10",
            )}
          >
            None
          </button>
          <button
            onClick={() => onSceneSourceChange("custom")}
            className={cn(
              "flex-1 py-2.5 px-3",
              radius.md,
              "border text-sm font-medium",
              interactive.transition.fast,
              sceneSource === "custom"
                ? "border-accent/40 bg-accent/20 text-accent"
                : "border-fg/15 bg-fg/5 text-fg/70 active:bg-fg/10",
            )}
          >
            Custom
          </button>
          <button
            onClick={() => onSceneSourceChange("character")}
            disabled={availableScenes.length === 0}
            className={cn(
              "flex-1 py-2.5 px-3",
              radius.md,
              "border text-sm font-medium",
              interactive.transition.fast,
              sceneSource === "character"
                ? "border-accent/40 bg-accent/20 text-accent"
                : "border-fg/15 bg-fg/5 text-fg/70 active:bg-fg/10",
              availableScenes.length === 0 && "opacity-50 cursor-not-allowed",
            )}
          >
            From Character
          </button>
        </div>
        <p className={cn(typography.bodySmall.size, "mt-2 text-fg/40")}>
          {sceneSource === "none"
            ? "Start without a predefined scene"
            : sceneSource === "custom"
              ? "Write your own opening scenario"
              : "Use a scene from one of your characters"}
        </p>
      </div>

      {/* Custom Scene Editor */}
      {sceneSource === "custom" && (
        <div className={cn(spacing.field, "relative")}>
          <label
            className={cn(
              typography.label.size,
              typography.label.weight,
              typography.label.tracking,
              "uppercase text-fg/70",
            )}
          >
            Scene Content
          </label>

          {/* Character Autocomplete Popup - Above textarea */}
          {showAutocomplete && filteredCharacters.length > 0 && (
            <div
              className={cn(
                "absolute z-50 mb-1 w-full max-h-48 overflow-y-auto",
                radius.md,
                "border border-fg/20 bg-surface-el shadow-lg",
              )}
              style={{ bottom: "100%" }}
              onClick={(e) => e.stopPropagation()}
            >
              {filteredCharacters.map((character, index) => (
                <button
                  key={character.id}
                  onClick={() => insertCharacterName(character.name)}
                  className={cn(
                    "w-full text-left px-3 py-2.5 flex items-center gap-2",
                    "hover:bg-fg/10",
                    interactive.transition.fast,
                    index === autocompleteIndex && "bg-fg/15",
                  )}
                >
                  <div className="flex-1">
                    <div className="text-sm font-medium text-fg">{character.name}</div>
                    <div className="text-xs text-fg/50">
                      Insert as {`{{@"${character.name}"}}`}
                    </div>
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
            placeholder="Describe the starting scene for this roleplay..."
            rows={6}
            className={cn(
              "w-full px-4 py-3",
              radius.md,
              "border border-fg/15 bg-fg/5",
              "text-fg placeholder:text-fg/30",
              "focus:outline-none focus:border-fg/30",
              interactive.transition.fast,
              "resize-none",
              customScene.trim() && "border-secondary/30 bg-secondary/5",
            )}
          />

          <p className={cn(typography.bodySmall.size, "mt-2 text-fg/40")}>
            Tip: Type{" "}
            <code className="px-1.5 py-0.5 rounded bg-fg/10 text-secondary font-mono text-[10px]">
              {`{{@"`}
            </code>{" "}
            to reference characters
          </p>
        </div>
      )}

      {/* Character Scene Selector */}
      {sceneSource === "character" && availableScenes.length > 0 && (
        <div className={spacing.field}>
          <label
            className={cn(
              typography.label.size,
              typography.label.weight,
              typography.label.tracking,
              "uppercase text-fg/70",
            )}
          >
            Select a Scene
          </label>
          <div className="space-y-2">
            {availableScenes.map((scene) => (
              <div key={scene.sceneId} className="space-y-2">
                <button
                  onClick={() => onSelectedCharacterSceneIdChange(scene.sceneId)}
                  className={cn(
                    "w-full text-left px-4 py-3",
                    radius.md,
                    "border",
                    interactive.transition.fast,
                    selectedCharacterSceneId === scene.sceneId
                      ? "border-accent/40 bg-accent/10"
                      : "border-fg/15 bg-fg/5 active:bg-fg/10",
                  )}
                >
                  <div className="flex items-center gap-2 mb-1.5">
                    <span className="text-sm font-medium text-fg/90">
                      {scene.characterName}'s Scene
                    </span>
                    {selectedCharacterSceneId === scene.sceneId && (
                      <Check size={14} className="text-accent" />
                    )}
                  </div>
                  <div className="text-xs text-fg/60 line-clamp-3">{scene.content}</div>
                </button>
                {selectedCharacterSceneId === scene.sceneId && (
                  <button
                    onClick={() => handleCopyToCustom(scene.content)}
                    className={cn(
                      "w-full py-2 px-3 flex items-center justify-center gap-2",
                      radius.md,
                      "border border-info/40 bg-info/10",
                      "text-xs font-medium text-info",
                      "active:bg-info/20",
                      interactive.transition.fast,
                    )}
                  >
                    <Copy size={12} />
                    Copy to Custom & Edit
                  </button>
                )}
              </div>
            ))}
          </div>
        </div>
      )}

      {/* Error Message */}
      {error && (
        <div
          className={cn(
            "px-4 py-2.5",
            radius.md,
            "border border-danger/30 bg-danger/10",
            "text-sm text-danger",
          )}
        >
          {error}
        </div>
      )}

      {/* Create Button */}
      <div className="pt-4 mt-auto">
        <motion.button
          disabled={!canCreate || creating}
          onClick={onCreateGroup}
          whileTap={{ scale: canCreate && !creating ? 0.97 : 1 }}
          className={cn(
            "w-full py-4 text-base font-semibold",
            radius.md,
            interactive.transition.fast,
            canCreate && !creating
              ? cn(
                  "border border-accent/40 bg-accent/20 text-accent",
                  shadows.glow,
                  "active:border-accent/60 active:bg-accent/30",
                )
              : "cursor-not-allowed border border-fg/5 bg-fg/5 text-fg/30",
          )}
        >
          {creating ? "Creating..." : "Create Group Chat"}
        </motion.button>
      </div>
    </motion.div>
  );
}
