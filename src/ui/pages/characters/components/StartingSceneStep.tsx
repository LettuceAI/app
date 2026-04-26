import React from "react";
import { AnimatePresence, motion } from "framer-motion";
import { Plus, X, BookOpen, Edit2, ChevronDown, EyeOff, Image as ImageIcon, Upload, FolderOpen, Trash2 } from "lucide-react";
import { useLocation, useNavigate } from "react-router-dom";
import type { CharacterMode, Scene } from "../../../../core/storage/schemas";
import { typography, radius, spacing, interactive, shadows, cn } from "../../../design-tokens";
import { BottomMenu } from "../../../components/BottomMenu";
import { useI18n } from "../../../../core/i18n/context";
import { processBackgroundImage } from "../../../../core/utils/image";
import { useImageData } from "../../../hooks/useImageData";
import { convertFilePathToDataUrl } from "../../../../core/storage/images";
import {
  buildBackgroundLibrarySelectionKey,
  buildCharacterCreateLibraryReturnKey,
  type BackgroundLibrarySelectionPayload,
} from "../../../components/AvatarPicker/librarySelection";

interface SceneEditorDraft {
  target: "new" | "edit";
  editingSceneId: string | null;
  newSceneContent: string;
  newSceneDirection: string;
  newSceneBackgroundImagePath: string;
  editingSceneContent: string;
  editingSceneDirection: string;
  editingSceneBackgroundImagePath: string;
  showNewDirectionInput: boolean;
  editDirectionExpanded: boolean;
}

const CREATE_SCENE_BACKGROUND_SELECTION_KEY = "create-character-scene-background";
const CREATE_SCENE_EDITOR_DRAFT_KEY = "create-character-scene-editor-draft";
const CREATE_SCENE_EDITOR_RETURN_KEY = "create-character-scene-editor-return";

function loadStartingSceneEditorDraft(): SceneEditorDraft | null {
  if (typeof window === "undefined") {
    return null;
  }

  if (sessionStorage.getItem(CREATE_SCENE_EDITOR_RETURN_KEY) !== "true") {
    return null;
  }

  const rawDraft = sessionStorage.getItem(CREATE_SCENE_EDITOR_DRAFT_KEY);
  if (!rawDraft) {
    return null;
  }

  try {
    return JSON.parse(rawDraft) as SceneEditorDraft;
  } catch (error) {
    console.error("Failed to parse starting scene editor draft:", error);
    sessionStorage.removeItem(CREATE_SCENE_EDITOR_DRAFT_KEY);
    sessionStorage.removeItem(CREATE_SCENE_EDITOR_RETURN_KEY);
    return null;
  }
}

interface StartingSceneStepProps {
  scenes: Scene[];
  onScenesChange: (scenes: Scene[]) => void;
  defaultSceneId: string | null;
  onDefaultSceneIdChange: (id: string | null) => void;
  onContinue: () => void;
  canContinue: boolean;
  mode?: CharacterMode;
}

export function StartingSceneStep({
  scenes,
  onScenesChange,
  defaultSceneId,
  onDefaultSceneIdChange,
  onContinue,
  canContinue,
  mode = "roleplay",
}: StartingSceneStepProps) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const location = useLocation();
  const isCompanion = mode === "companion";
  const returnPath = `${location.pathname}${location.search}`;
  const initialSceneEditorDraft = React.useMemo(() => loadStartingSceneEditorDraft(), []);
  const restoredSceneBackgroundTarget = initialSceneEditorDraft?.target ?? null;
  const [newSceneContent, setNewSceneContent] = React.useState(
    () => initialSceneEditorDraft?.newSceneContent ?? "",
  );
  const [newSceneDirection, setNewSceneDirection] = React.useState(
    () => initialSceneEditorDraft?.newSceneDirection ?? "",
  );
  const [showNewDirectionInput, setShowNewDirectionInput] = React.useState(
    () => initialSceneEditorDraft?.showNewDirectionInput ?? false,
  );
  const [editingSceneId, setEditingSceneId] = React.useState<string | null>(
    () => initialSceneEditorDraft?.editingSceneId ?? null,
  );
  const [editingSceneContent, setEditingSceneContent] = React.useState(
    () => initialSceneEditorDraft?.editingSceneContent ?? "",
  );
  const [editingSceneDirection, setEditingSceneDirection] = React.useState(
    () => initialSceneEditorDraft?.editingSceneDirection ?? "",
  );
  const [newSceneBackgroundImagePath, setNewSceneBackgroundImagePath] = React.useState(
    () => initialSceneEditorDraft?.newSceneBackgroundImagePath ?? "",
  );
  const [editingSceneBackgroundImagePath, setEditingSceneBackgroundImagePath] = React.useState(
    () => initialSceneEditorDraft?.editingSceneBackgroundImagePath ?? "",
  );
  const [editDirectionExpanded, setEditDirectionExpanded] = React.useState(
    () => initialSceneEditorDraft?.editDirectionExpanded ?? false,
  );
  const [expandedSceneId, setExpandedSceneId] = React.useState<string | null>(null);
  const newSceneBackgroundInputRef = React.useRef<HTMLInputElement | null>(null);
  const editSceneBackgroundInputRef = React.useRef<HTMLInputElement | null>(null);

  React.useEffect(() => {
    const rawSelection = sessionStorage.getItem(
      buildBackgroundLibrarySelectionKey(CREATE_SCENE_BACKGROUND_SELECTION_KEY),
    );

    let cancelled = false;
    const clearResumeStateTimeout = window.setTimeout(() => {
      sessionStorage.removeItem(CREATE_SCENE_EDITOR_DRAFT_KEY);
      sessionStorage.removeItem(CREATE_SCENE_EDITOR_RETURN_KEY);
      sessionStorage.removeItem(
        buildBackgroundLibrarySelectionKey(CREATE_SCENE_BACKGROUND_SELECTION_KEY),
      );
    }, 0);

    if (rawSelection) {
      let parsed: BackgroundLibrarySelectionPayload | null = null;
      try {
        parsed = JSON.parse(rawSelection) as BackgroundLibrarySelectionPayload;
      } catch (error) {
        console.error("Failed to parse starting scene background library selection:", error);
      }

      if (parsed?.filePath) {
        void (async () => {
          const dataUrl = await convertFilePathToDataUrl(parsed.filePath);
          if (!dataUrl || cancelled) return;
          if (restoredSceneBackgroundTarget === "edit") {
            setEditingSceneBackgroundImagePath(dataUrl);
          } else {
            setNewSceneBackgroundImagePath(dataUrl);
          }
        })();
      }
    }

    return () => {
      cancelled = true;
      window.clearTimeout(clearResumeStateTimeout);
    };
  }, [restoredSceneBackgroundTarget]);

  const addScene = () => {
    if (!newSceneContent.trim()) return;

    const sceneId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
    const timestamp = Date.now();

    const newScene: Scene = {
      id: sceneId,
      content: newSceneContent.trim(),
      direction: newSceneDirection.trim() || undefined,
      backgroundImagePath: newSceneBackgroundImagePath || undefined,
      createdAt: timestamp,
    };

    const updatedScenes = [...scenes, newScene];
    onScenesChange(updatedScenes);

    if (updatedScenes.length === 1) {
      onDefaultSceneIdChange(sceneId);
    }

    setNewSceneContent("");
    setNewSceneDirection("");
    setNewSceneBackgroundImagePath("");
    setShowNewDirectionInput(false);
  };

  const deleteScene = (sceneId: string) => {
    const updatedScenes = scenes.filter((s) => s.id !== sceneId);
    onScenesChange(updatedScenes);

    if (defaultSceneId === sceneId) {
      onDefaultSceneIdChange(
        updatedScenes.length === 1 ? updatedScenes[0].id : updatedScenes[0]?.id || null,
      );
    }
  };

  const startEditingScene = (scene: Scene) => {
    setEditingSceneId(scene.id);
    setEditingSceneContent(scene.content);
    setEditingSceneDirection(scene.direction || "");
    setEditingSceneBackgroundImagePath(scene.backgroundImagePath || "");
    setEditDirectionExpanded(Boolean(scene.direction));
  };

  const saveEditedScene = () => {
    if (!editingSceneId || !editingSceneContent.trim()) return;

    const updatedScenes = scenes.map((scene) =>
      scene.id === editingSceneId
        ? {
            ...scene,
            content: editingSceneContent.trim(),
            direction: editingSceneDirection.trim() || undefined,
            backgroundImagePath: editingSceneBackgroundImagePath || undefined,
          }
        : scene,
    );
    onScenesChange(updatedScenes);

    setEditingSceneId(null);
    setEditingSceneContent("");
    setEditingSceneDirection("");
    setEditingSceneBackgroundImagePath("");
    setEditDirectionExpanded(false);
  };

  const cancelEditingScene = () => {
    setEditingSceneId(null);
    setEditingSceneContent("");
    setEditingSceneDirection("");
    setEditingSceneBackgroundImagePath("");
    setEditDirectionExpanded(false);
  };

  const handleSceneBackgroundUpload = React.useCallback(
    async (
      event: React.ChangeEvent<HTMLInputElement>,
      mode: "new" | "edit",
    ) => {
      const file = event.target.files?.[0];
      const input = event.target;
      if (!file) return;

      try {
        const dataUrl = await processBackgroundImage(file);
        if (mode === "new") {
          setNewSceneBackgroundImagePath(dataUrl);
        } else {
          setEditingSceneBackgroundImagePath(dataUrl);
        }
      } catch (error) {
        console.warn("StartingSceneStep: failed to process scene background", error);
      } finally {
        input.value = "";
      }
    },
    [],
  );

  const handleChooseSceneBackgroundFromLibrary = React.useCallback(
    (target: "new" | "edit") => {
      const draft: SceneEditorDraft = {
        target,
        editingSceneId,
        newSceneContent,
        newSceneDirection,
        newSceneBackgroundImagePath,
        editingSceneContent,
        editingSceneDirection,
        editingSceneBackgroundImagePath,
        showNewDirectionInput,
        editDirectionExpanded,
      };
      sessionStorage.setItem(CREATE_SCENE_EDITOR_DRAFT_KEY, JSON.stringify(draft));
      sessionStorage.setItem(CREATE_SCENE_EDITOR_RETURN_KEY, "true");
      sessionStorage.setItem(buildCharacterCreateLibraryReturnKey(returnPath), "true");
      navigate("/library/images/pick", {
        state: {
          returnPath,
          selectionStorageKey: CREATE_SCENE_BACKGROUND_SELECTION_KEY,
          selectionKind: "background",
        },
      });
    },
    [
      editDirectionExpanded,
      editingSceneBackgroundImagePath,
      editingSceneContent,
      editingSceneDirection,
      editingSceneId,
      navigate,
      newSceneBackgroundImagePath,
      newSceneContent,
      newSceneDirection,
      returnPath,
      showNewDirectionInput,
    ],
  );

  return (
    <div className={cn(spacing.section, "flex flex-col flex-1 min-h-0")}>
      {/* Title */}
      <div className={spacing.tight}>
        <div className="flex items-center gap-2">
          <div className="rounded-lg border border-info/30 bg-info/10 p-1.5">
            <BookOpen className="h-4 w-4 text-info" />
          </div>
          <h2 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
            {isCompanion ? "Opening Context" : t("characters.scenes.title")}
          </h2>
          {scenes.length > 0 && (
            <span className="ml-auto rounded-full border border-fg/10 bg-fg/5 px-2 py-0.5 text-xs text-fg/70">
              {scenes.length}
            </span>
          )}
        </div>
        <p className={cn(typography.body.size, "mt-2 text-fg/50")}>
          {isCompanion
            ? "Optional first-chat context for this companion. Companion sessions can start without a scene."
            : t("characters.scenes.subtitle")}
        </p>
      </div>

      {/* Desktop: Side-by-side / Mobile: stacked */}
      <div className="flex flex-col lg:flex-row lg:gap-6">
        {/* Left: Existing Scenes */}
        <div className={cn("lg:flex-1 lg:min-w-0", spacing.item, "space-y-2")}>
          <AnimatePresence initial={false}>
            {scenes.map((scene, index) => {
              const isEditing = editingSceneId === scene.id;
              const isDefault = defaultSceneId === scene.id;
              const isExpanded = expandedSceneId === scene.id || isEditing;

              return (
                <motion.div
                  key={scene.id}
                  initial={{ opacity: 0, height: 0 }}
                  animate={{ opacity: 1, height: "auto" }}
                  exit={{ opacity: 0, height: 0 }}
                  transition={{ duration: 0.15, ease: "easeOut" }}
                  className={cn(
                    "overflow-hidden rounded-xl border transition-colors duration-150",
                    isDefault
                      ? "border-accent/30 bg-accent/5"
                      : "border-fg/10 bg-fg/5",
                  )}
                >
                  {/* Scene Header - clickable to expand/collapse */}
                  <button
                    onClick={() => !isEditing && setExpandedSceneId(isExpanded ? null : scene.id)}
                    className={cn(
                      "flex w-full items-center gap-2 border-b px-3.5 py-2.5 text-left transition-colors duration-150",
                      isDefault
                        ? "border-accent/20 bg-accent/10"
                        : "border-fg/10 bg-fg/5",
                    )}
                  >
                    {/* Scene number badge */}
                    <div
                      className={cn(
                        "flex h-6 w-6 shrink-0 items-center justify-center rounded-lg border text-xs font-medium",
                        isDefault
                          ? "border-accent/40 bg-accent/20 text-accent/80"
                          : "border-fg/10 bg-fg/5 text-fg/60",
                      )}
                    >
                      {index + 1}
                    </div>

                    {/* Default badge */}
                    {isDefault && (
                      <div className="flex items-center gap-1 rounded-full border border-accent/40 bg-accent/20 px-2 py-0.5">
                        <div className="h-1.5 w-1.5 rounded-full bg-accent" />
                        <span className="text-[10px] font-medium text-accent/80">{t("characters.scenes.default")}</span>
                      </div>
                    )}

                    {/* Direction indicator */}
                    {scene.direction && (
                      <div
                        className="flex items-center gap-1 rounded-full border border-fg/10 bg-fg/5 px-1.5 py-0.5"
                        title={t("characters.scenes.hasSceneDirection")}
                      >
                        <EyeOff className="h-3 w-3 text-fg/40" />
                      </div>
                    )}

                    {/* Preview text when collapsed */}
                    {!isExpanded && !isEditing && (
                      <span className="flex-1 truncate text-sm text-fg/50">
                        {scene.content.slice(0, 50)}
                        {scene.content.length > 50 ? "..." : ""}
                      </span>
                    )}

                    {/* Expand indicator */}
                    {!isEditing && (
                      <ChevronDown
                        className={cn(
                          "h-4 w-4 text-fg/40 ml-auto transition-transform duration-150",
                          isExpanded && "rotate-180",
                        )}
                      />
                    )}
                  </button>

                  {/* Scene Content */}
                  <div
                    className={cn(
                      "grid transition-[grid-template-rows] duration-200 ease-out",
                      isExpanded ? "grid-rows-[1fr]" : "grid-rows-[0fr]",
                    )}
                  >
                    <div className="overflow-hidden">
                      <div className="p-3.5">
                        <div className="space-y-3">
                          <p className="text-sm leading-relaxed text-fg/90">{scene.content}</p>

                          {/* Scene Direction */}
                          {scene.direction && (
                            <div className="pt-2 border-t border-fg/5">
                              <p className="text-[10px] font-medium text-fg/40 mb-1">
                                Scene Direction
                              </p>
                              <p className="text-xs leading-relaxed text-fg/50 italic">
                                {scene.direction}
                              </p>
                            </div>
                          )}

                          {scene.backgroundImagePath && (
                            <SceneBackgroundPreview
                              path={scene.backgroundImagePath}
                              label="Scene background"
                              compact
                            />
                          )}

                          {/* Actions when expanded */}
                          <div className="flex items-center gap-2 pt-2 border-t border-fg/5">
                            {!isDefault && (
                              <button
                                onClick={(e) => {
                                  e.stopPropagation();
                                  onDefaultSceneIdChange(scene.id);
                                }}
                                className="rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-xs font-medium text-fg/60 transition active:scale-95 active:bg-fg/10"
                              >
                                Set as Default
                              </button>
                            )}
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                startEditingScene(scene);
                              }}
                              className="rounded-lg border border-fg/10 bg-fg/5 p-1.5 text-fg/60 transition active:scale-95 active:bg-fg/10"
                            >
                              <Edit2 className="h-3.5 w-3.5" />
                            </button>
                            <button
                              onClick={(e) => {
                                e.stopPropagation();
                                deleteScene(scene.id);
                              }}
                              className="rounded-lg border border-fg/10 bg-fg/5 p-1.5 text-fg/50 transition active:bg-danger/10 active:text-danger"
                            >
                              <X className="h-3.5 w-3.5" />
                            </button>
                          </div>
                        </div>
                      </div>
                    </div>
                  </div>
                </motion.div>
              );
            })}
          </AnimatePresence>

          {scenes.length === 0 && (
            <div className="rounded-xl border border-dashed border-fg/10 bg-fg/2 px-4 py-8 text-center">
              <BookOpen className="mx-auto h-8 w-8 text-fg/20 mb-2" />
              <p className="text-sm text-fg/40">
                {isCompanion ? "No opening context yet" : "No scenes yet"}
              </p>
              <p className="text-xs text-fg/30 mt-1">
                {isCompanion
                  ? "You can skip this for companion mode."
                  : "Create your first scene to get started"}
              </p>
            </div>
          )}
        </div>

        {/* Right: Add New Scene */}
        <div className={cn("lg:flex-1 lg:min-w-0 mt-3 lg:mt-0", spacing.item)}>
          <textarea
            value={newSceneContent}
            onChange={(e) => setNewSceneContent(e.target.value)}
            rows={6}
            placeholder={
              isCompanion
                ? "Optional opening context, like where the companion is or what they were doing before the first message..."
                : "Create a starting scene or scenario for roleplay (e.g., 'You find yourself in a mystical forest at twilight...')"
            }
            className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/20 px-3.5 py-3 text-sm leading-relaxed text-fg placeholder-fg/40 transition focus:border-fg/25 focus:outline-none"
          />
          <div className="flex items-center justify-between mt-1">
            {!showNewDirectionInput && !newSceneDirection ? (
              <button
                type="button"
                onClick={() => setShowNewDirectionInput(true)}
                className="flex items-center gap-1.5 text-[11px] text-fg/40 hover:text-fg/60 transition"
              >
                <EyeOff className="h-3 w-3" />+ Add Direction
              </button>
            ) : (
              <span className="text-[11px] text-fg/40">Direction added</span>
            )}
            <span className="text-[11px] text-fg/40">{wordCount(newSceneContent)} words</span>
          </div>
          <div className="mt-2 text-[11px] text-fg/50">
            Use <code className="text-accent/80">{"{{char}}"}</code> for the character and{" "}
            <code className="text-accent/80">{"{{user}}"}</code> (alias{" "}
            <code className="text-accent/80">{"{{persona}}"}</code>) for the persona.
          </div>

          <input
            ref={newSceneBackgroundInputRef}
            type="file"
            accept="image/*"
            className="hidden"
            onChange={(event) => {
              void handleSceneBackgroundUpload(event, "new");
            }}
          />

          <div className="mt-3">
            <div className="flex items-center gap-2 mb-2">
              <ImageIcon className="h-3.5 w-3.5 text-fg/50" />
              <span className="text-xs font-medium text-fg/70">Scene Background</span>
              <span className="text-[10px] text-fg/35">Optional</span>
            </div>
            {newSceneBackgroundImagePath ? (
              <SceneBackgroundDropzone
                path={newSceneBackgroundImagePath}
                onLibrary={() => handleChooseSceneBackgroundFromLibrary("new")}
                onUpload={() => newSceneBackgroundInputRef.current?.click()}
                onRemove={() => setNewSceneBackgroundImagePath("")}
              />
            ) : (
              <SceneBackgroundEmptyZone
                onLibrary={() => handleChooseSceneBackgroundFromLibrary("new")}
                onUpload={() => newSceneBackgroundInputRef.current?.click()}
                hint="Overrides the character background for chats using this scene."
              />
            )}
          </div>

          {/* Scene Direction Input - CSS grid for smooth height */}
          <div
            className={cn(
              "grid transition-[grid-template-rows] duration-150 ease-out",
              showNewDirectionInput || newSceneDirection ? "grid-rows-[1fr]" : "grid-rows-[0fr]",
            )}
          >
            <div className="overflow-hidden">
              <div className="mt-3 space-y-1.5">
                <div className="flex items-center justify-between">
                  <span className="flex items-center gap-1.5 text-xs font-medium text-fg/50">
                    <EyeOff className="h-3 w-3" />
                    Scene Direction
                  </span>
                  {!newSceneDirection && (
                    <button
                      type="button"
                      onClick={() => setShowNewDirectionInput(false)}
                      className="text-[11px] text-fg/40 hover:text-fg/60"
                    >
                      Cancel
                    </button>
                  )}
                </div>
                <textarea
                  value={newSceneDirection}
                  onChange={(e) => setNewSceneDirection(e.target.value)}
                  rows={2}
                  placeholder="e.g., 'The hostage will be rescued' or 'Maintain tense atmosphere'"
                  className="w-full resize-none rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm leading-relaxed text-fg placeholder-fg/30 transition focus:border-fg/20 focus:outline-none"
                />
                <p className="text-[10px] text-fg/30">
                  Hidden guidance for the AI on how this scene should unfold
                </p>
              </div>
            </div>
          </div>

          <button
            onClick={addScene}
            disabled={!newSceneContent.trim()}
            className={cn(
              "mt-3 flex items-center gap-2 rounded-xl px-3.5 py-2.5 text-sm font-medium transition active:scale-[0.98]",
              newSceneContent.trim()
                ? "border border-info/40 bg-info/20 text-info active:bg-info/30"
                : "border border-fg/10 bg-fg/5 text-fg/40",
            )}
          >
            <Plus className="h-4 w-4" />
            Add Scene
          </button>
        </div>
      </div>

      {/* Continue Button - moved to bottom */}
      <div className="pt-4 mt-auto space-y-3">
        <p className="text-xs text-fg/50 text-center">
          {isCompanion
            ? "Opening context is optional for companions; long-term continuity comes from companion memory."
            : "Create multiple starting scenarios. One will be selected when starting a new chat."}
        </p>
        <button
          disabled={!canContinue}
          onClick={onContinue}
          className={cn(
            "w-full py-4 text-base font-semibold transition active:scale-[0.98]",
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
          {isCompanion && scenes.length === 0 ? "Skip Context" : t("characters.scenes.continueToScenes")}
        </button>
      </div>

      {/* Edit Scene Bottom Menu */}
      <BottomMenu isOpen={editingSceneId !== null} onClose={cancelEditingScene} title="Edit Scene">
        <div className="space-y-4">
          <input
            ref={editSceneBackgroundInputRef}
            type="file"
            accept="image/*"
            className="hidden"
            onChange={(event) => {
              void handleSceneBackgroundUpload(event, "edit");
            }}
          />

          {/* Scene Content */}
          <div>
            <textarea
              value={editingSceneContent}
              onChange={(e) => setEditingSceneContent(e.target.value)}
              rows={10}
              className="w-full resize-none rounded-xl border border-fg/10 bg-surface-el/30 px-4 py-3 text-sm leading-relaxed text-fg placeholder-fg/40 transition focus:border-fg/20 focus:outline-none"
              placeholder="Enter scene content..."
            />
            <div className="mt-1 flex justify-end text-[11px] text-fg/40">
              {wordCount(editingSceneContent)} words
            </div>
          </div>

          {/* Scene Direction - Collapsible */}
          <div className="border-t border-fg/10 pt-3">
            {!editDirectionExpanded && !editingSceneDirection ? (
              <button
                type="button"
                onClick={() => setEditDirectionExpanded(true)}
                className="flex w-full items-center justify-between py-1"
              >
                <span className="flex items-center gap-1.5 text-xs font-medium text-fg/50">
                  <EyeOff className="h-3 w-3" />
                  Scene Direction
                </span>
                <span className="text-[11px] text-fg/40">+ Add</span>
              </button>
            ) : (
              <div className="space-y-2">
                <div className="flex items-center justify-between">
                  <span className="flex items-center gap-1.5 text-xs font-medium text-fg/50">
                    <EyeOff className="h-3 w-3" />
                    Scene Direction
                  </span>
                  {!editingSceneDirection && (
                    <button
                      type="button"
                      onClick={() => setEditDirectionExpanded(false)}
                      className="text-[11px] text-fg/40 hover:text-fg/60"
                    >
                      Cancel
                    </button>
                  )}
                </div>
                <textarea
                  value={editingSceneDirection}
                  onChange={(e) => setEditingSceneDirection(e.target.value)}
                  rows={2}
                  className="w-full resize-none rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-sm leading-relaxed text-fg placeholder-fg/30 transition focus:border-fg/20 focus:outline-none"
                  placeholder="e.g., 'The hostage will be rescued' or 'Build tension gradually'"
                />
                <p className="text-[10px] text-fg/30">
                  Hidden guidance for the AI on how this scene should unfold
                </p>
              </div>
            )}
          </div>

          <div className="border-t border-fg/10 pt-3">
            <div className="flex items-center gap-2 mb-2">
              <ImageIcon className="h-3.5 w-3.5 text-fg/50" />
              <span className="text-xs font-medium text-fg/70">Scene Background</span>
              <span className="text-[10px] text-fg/35">Optional</span>
            </div>
            {editingSceneBackgroundImagePath ? (
              <SceneBackgroundDropzone
                path={editingSceneBackgroundImagePath}
                onLibrary={() => handleChooseSceneBackgroundFromLibrary("edit")}
                onUpload={() => editSceneBackgroundInputRef.current?.click()}
                onRemove={() => setEditingSceneBackgroundImagePath("")}
              />
            ) : (
              <SceneBackgroundEmptyZone
                onLibrary={() => handleChooseSceneBackgroundFromLibrary("edit")}
                onUpload={() => editSceneBackgroundInputRef.current?.click()}
                hint="Used as the chat background for this scene unless the session overrides it."
              />
            )}
          </div>

          <div className="flex gap-3">
            <button
              onClick={cancelEditingScene}
              className={cn(
                "flex-1 py-3 text-sm font-medium text-fg/70 transition",
                "border border-fg/10 bg-fg/5",
                "hover:bg-fg/10 hover:text-fg",
                "active:scale-[0.98]",
                radius.lg,
              )}
            >
              Cancel
            </button>
            <button
              onClick={saveEditedScene}
              disabled={!editingSceneContent.trim()}
              className={cn(
                "flex-1 py-3 text-sm font-semibold text-fg transition",
                "bg-linear-to-r from-accent to-accent/80",
                "hover:from-accent/80 hover:to-accent/60",
                "active:scale-[0.98]",
                "disabled:cursor-not-allowed disabled:opacity-50",
                radius.lg,
              )}
            >
              Save
            </button>
          </div>
        </div>
      </BottomMenu>
    </div>
  );
}

const wordCount = (text: string) => {
  const trimmed = text.trim();
  if (!trimmed) return 0;
  return trimmed.split(/\s+/).length;
};

function SceneBackgroundPreview({
  path,
  label,
  compact = false,
}: {
  path: string;
  label: string;
  compact?: boolean;
}) {
  const imageData = useImageData(path) ?? path;

  return (
    <div className="overflow-hidden rounded-xl border border-fg/10 bg-fg/4">
      <img
        src={imageData}
        alt={label}
        className={cn("w-full object-cover", compact ? "h-24" : "h-28")}
      />
      <div className="flex items-center gap-2 border-t border-fg/10 px-3 py-2 text-[11px] text-fg/55">
        <Upload className="h-3 w-3" />
        {label}
      </div>
    </div>
  );
}

function SceneBackgroundEmptyZone({
  onLibrary,
  onUpload,
  hint,
}: {
  onLibrary: () => void;
  onUpload: () => void;
  hint: string;
}) {
  return (
    <div className="rounded-xl border border-dashed border-fg/15 bg-fg/2 p-4">
      <div className="flex flex-col items-center text-center">
        <div className="rounded-xl border border-fg/10 bg-fg/5 p-2.5 mb-2">
          <ImageIcon className="h-5 w-5 text-fg/40" />
        </div>
        <p className="text-xs text-fg/50 mb-3 max-w-xs">{hint}</p>
        <div className="flex items-center gap-2 w-full max-w-xs">
          <button
            type="button"
            onClick={onLibrary}
            className="flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-xs font-medium text-fg/70 transition active:scale-95 active:bg-fg/10"
          >
            <FolderOpen className="h-3.5 w-3.5" />
            Library
          </button>
          <button
            type="button"
            onClick={onUpload}
            className="flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-accent/30 bg-accent/10 px-3 py-2 text-xs font-medium text-accent/90 transition active:scale-95 active:bg-accent/20"
          >
            <Upload className="h-3.5 w-3.5" />
            Upload
          </button>
        </div>
      </div>
    </div>
  );
}

function SceneBackgroundDropzone({
  path,
  onLibrary,
  onUpload,
  onRemove,
}: {
  path: string;
  onLibrary: () => void;
  onUpload: () => void;
  onRemove: () => void;
}) {
  const imageData = useImageData(path) ?? path;

  return (
    <div className="group relative overflow-hidden rounded-xl border border-fg/15 bg-fg/4">
      <img src={imageData} alt="Scene background" className="w-full h-40 object-cover" />
      <div className="absolute inset-0 bg-linear-to-t from-black/70 via-black/20 to-transparent pointer-events-none" />
      <div className="absolute top-2 right-2">
        <button
          type="button"
          onClick={onRemove}
          className="flex items-center justify-center rounded-lg border border-white/20 bg-black/50 backdrop-blur-sm p-1.5 text-white/90 transition active:scale-95 hover:bg-black/70"
          title="Remove background"
        >
          <Trash2 className="h-3.5 w-3.5" />
        </button>
      </div>
      <div className="absolute inset-x-0 bottom-0 p-2.5 flex items-center gap-2">
        <button
          type="button"
          onClick={onLibrary}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-white/15 bg-black/50 backdrop-blur-sm px-3 py-1.5 text-[11px] font-medium text-white/90 transition active:scale-95 hover:bg-black/70"
        >
          <FolderOpen className="h-3 w-3" />
          Library
        </button>
        <button
          type="button"
          onClick={onUpload}
          className="flex flex-1 items-center justify-center gap-1.5 rounded-lg border border-white/15 bg-black/50 backdrop-blur-sm px-3 py-1.5 text-[11px] font-medium text-white/90 transition active:scale-95 hover:bg-black/70"
        >
          <Upload className="h-3 w-3" />
          Upload
        </button>
      </div>
    </div>
  );
}
