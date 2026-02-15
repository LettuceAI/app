import { useReducer, useEffect, useCallback } from "react";
import {
  readSettings,
  saveCharacter,
  saveLorebook,
  saveLorebookEntry,
  setCharacterLorebooks,
} from "../../../../core/storage";
import { saveAvatar } from "../../../../core/storage/avatars";
import { convertToImageRef } from "../../../../core/storage/images";
import type {
  AvatarCrop,
  Model,
  Scene,
  SystemPromptTemplate,
  CharacterVoiceConfig,
} from "../../../../core/storage/schemas";
import { listPromptTemplates } from "../../../../core/prompts/service";
import { processBackgroundImage } from "../../../../core/utils/image";
import { invalidateAvatarCache } from "../../../hooks/useAvatar";
import {
  previewCharacterImport,
  readFileAsText,
  type CharacterFileFormat,
  type CharacterBookImport,
} from "../../../../core/storage/characterTransfer";
import { toast } from "../../../components/toast";
import {
  APP_DEFAULT_TEMPLATE_ID,
  isSystemPromptTemplate,
} from "../../../../core/prompts/constants";
export enum Step {
  Identity = 1,
  Description = 2,
  StartingScene = 3,
  Extras = 4,
}

const FORMAT_LABELS: Record<CharacterFileFormat, string> = {
  uec: "Unified Entity Card (UEC)",
  chara_card_v3: "Character Card V3",
  chara_card_v2: "Character Card V2",
  chara_card_v1: "Character Card V1",
  legacy_json: "Legacy JSON",
};

interface CharacterFormState {
  // Form data
  step: Step;
  name: string;
  avatarPath: string;
  avatarCrop: AvatarCrop | null;
  avatarRoundPath: string | null;
  backgroundImagePath: string;
  scenes: Scene[];
  defaultSceneId: string | null;
  definition: string;
  description: string;
  nickname: string;
  creator: string;
  creatorNotes: string;
  creatorNotesMultilingualText: string;
  tagsText: string;
  importedCharacterBook: CharacterBookImport | null;
  selectedModelId: string | null;
  selectedFallbackModelId: string | null;
  systemPromptTemplateId: string | null;
  memoryType: "manual" | "dynamic";
  dynamicMemoryEnabled: boolean;
  disableAvatarGradient: boolean;
  voiceConfig: CharacterVoiceConfig | null;
  voiceAutoplay: boolean;

  // Models
  models: Model[];
  loadingModels: boolean;

  // Prompt templates
  promptTemplates: SystemPromptTemplate[];
  loadingTemplates: boolean;

  // UI state
  saving: boolean;
  error: string | null;
  importingAvatar: boolean;
  avatarImportError: string | null;
}

type CharacterFormAction =
  | { type: "SET_STEP"; payload: Step }
  | { type: "SET_NAME"; payload: string }
  | { type: "SET_AVATAR_PATH"; payload: string }
  | { type: "SET_AVATAR_CROP"; payload: AvatarCrop | null }
  | { type: "SET_AVATAR_ROUND_PATH"; payload: string | null }
  | { type: "SET_BACKGROUND_IMAGE_PATH"; payload: string }
  | { type: "SET_SCENES"; payload: Scene[] }
  | { type: "SET_DEFAULT_SCENE_ID"; payload: string | null }
  | { type: "SET_DEFINITION"; payload: string }
  | { type: "SET_DESCRIPTION"; payload: string }
  | { type: "SET_NICKNAME"; payload: string }
  | { type: "SET_CREATOR"; payload: string }
  | { type: "SET_CREATOR_NOTES"; payload: string }
  | { type: "SET_CREATOR_NOTES_MULTILINGUAL_TEXT"; payload: string }
  | { type: "SET_TAGS_TEXT"; payload: string }
  | { type: "SET_IMPORTED_CHARACTER_BOOK"; payload: CharacterBookImport | null }
  | { type: "SET_SELECTED_MODEL_ID"; payload: string | null }
  | { type: "SET_SELECTED_FALLBACK_MODEL_ID"; payload: string | null }
  | { type: "SET_SYSTEM_PROMPT_TEMPLATE_ID"; payload: string | null }
  | { type: "SET_MEMORY_TYPE"; payload: "manual" | "dynamic" }
  | { type: "SET_DYNAMIC_MEMORY_ENABLED"; payload: boolean }
  | { type: "SET_DISABLE_AVATAR_GRADIENT"; payload: boolean }
  | { type: "SET_VOICE_CONFIG"; payload: CharacterVoiceConfig | null }
  | { type: "SET_VOICE_AUTOPLAY"; payload: boolean }
  | { type: "SET_MODELS"; payload: Model[] }
  | { type: "SET_LOADING_MODELS"; payload: boolean }
  | { type: "SET_PROMPT_TEMPLATES"; payload: SystemPromptTemplate[] }
  | { type: "SET_LOADING_TEMPLATES"; payload: boolean }
  | { type: "SET_SAVING"; payload: boolean }
  | { type: "SET_ERROR"; payload: string | null }
  | { type: "SET_IMPORTING_AVATAR"; payload: boolean }
  | { type: "SET_AVATAR_IMPORT_ERROR"; payload: string | null }
  | { type: "RESET_FORM" };

const initialState: CharacterFormState = {
  step: Step.Identity,
  name: "",
  avatarPath: "",
  avatarCrop: null,
  avatarRoundPath: null,
  backgroundImagePath: "",
  scenes: [],
  defaultSceneId: null,
  definition: "",
  description: "",
  nickname: "",
  creator: "",
  creatorNotes: "",
  creatorNotesMultilingualText: "",
  tagsText: "",
  importedCharacterBook: null,
  selectedModelId: null,
  selectedFallbackModelId: null,
  systemPromptTemplateId: null,
  memoryType: "manual",
  dynamicMemoryEnabled: false,
  disableAvatarGradient: false,
  voiceConfig: null,
  voiceAutoplay: false,
  models: [],
  loadingModels: true,
  promptTemplates: [],
  loadingTemplates: true,
  saving: false,
  error: null,
  importingAvatar: false,
  avatarImportError: null,
};

function characterFormReducer(
  state: CharacterFormState,
  action: CharacterFormAction,
): CharacterFormState {
  switch (action.type) {
    case "SET_STEP":
      return { ...state, step: action.payload };
    case "SET_NAME":
      return { ...state, name: action.payload };
    case "SET_AVATAR_PATH":
      return { ...state, avatarPath: action.payload };
    case "SET_AVATAR_CROP":
      return { ...state, avatarCrop: action.payload };
    case "SET_AVATAR_ROUND_PATH":
      return { ...state, avatarRoundPath: action.payload };
    case "SET_BACKGROUND_IMAGE_PATH":
      return { ...state, backgroundImagePath: action.payload };
    case "SET_SCENES":
      return { ...state, scenes: action.payload };
    case "SET_DEFAULT_SCENE_ID":
      return { ...state, defaultSceneId: action.payload };
    case "SET_DEFINITION":
      return { ...state, definition: action.payload };
    case "SET_DESCRIPTION":
      return { ...state, description: action.payload };
    case "SET_NICKNAME":
      return { ...state, nickname: action.payload };
    case "SET_CREATOR":
      return { ...state, creator: action.payload };
    case "SET_CREATOR_NOTES":
      return { ...state, creatorNotes: action.payload };
    case "SET_CREATOR_NOTES_MULTILINGUAL_TEXT":
      return { ...state, creatorNotesMultilingualText: action.payload };
    case "SET_TAGS_TEXT":
      return { ...state, tagsText: action.payload };
    case "SET_IMPORTED_CHARACTER_BOOK":
      return { ...state, importedCharacterBook: action.payload };
    case "SET_SELECTED_MODEL_ID":
      return { ...state, selectedModelId: action.payload };
    case "SET_SELECTED_FALLBACK_MODEL_ID":
      return { ...state, selectedFallbackModelId: action.payload };
    case "SET_SYSTEM_PROMPT_TEMPLATE_ID":
      return { ...state, systemPromptTemplateId: action.payload };
    case "SET_MEMORY_TYPE":
      return { ...state, memoryType: action.payload };
    case "SET_DYNAMIC_MEMORY_ENABLED":
      return { ...state, dynamicMemoryEnabled: action.payload };
    case "SET_DISABLE_AVATAR_GRADIENT":
      return { ...state, disableAvatarGradient: action.payload };
    case "SET_VOICE_CONFIG":
      return { ...state, voiceConfig: action.payload };
    case "SET_VOICE_AUTOPLAY":
      return { ...state, voiceAutoplay: action.payload };
    case "SET_MODELS":
      return { ...state, models: action.payload };
    case "SET_LOADING_MODELS":
      return { ...state, loadingModels: action.payload };
    case "SET_PROMPT_TEMPLATES":
      return { ...state, promptTemplates: action.payload };
    case "SET_LOADING_TEMPLATES":
      return { ...state, loadingTemplates: action.payload };
    case "SET_SAVING":
      return { ...state, saving: action.payload };
    case "SET_ERROR":
      return { ...state, error: action.payload };
    case "SET_IMPORTING_AVATAR":
      return { ...state, importingAvatar: action.payload };
    case "SET_AVATAR_IMPORT_ERROR":
      return { ...state, avatarImportError: action.payload };
    case "RESET_FORM":
      return initialState;
    default:
      return state;
  }
}

export function useCharacterForm(draftCharacter?: any) {
  const [state, dispatch] = useReducer(characterFormReducer, initialState);

  // Load models and prompt templates on mount
  useEffect(() => {
    let cancelled = false;

    (async () => {
      try {
        const [settings, templates] = await Promise.all([readSettings(), listPromptTemplates()]);

        if (cancelled) return;

        dispatch({ type: "SET_MODELS", payload: settings.models });

        // Use draft character if provided, otherwise defaults
        if (draftCharacter) {
          console.log("[useCharacterForm] Received draftCharacter:", draftCharacter);
          dispatch({ type: "SET_NAME", payload: draftCharacter.name || "" });
          dispatch({
            type: "SET_DEFINITION",
            payload: draftCharacter.definition || draftCharacter.description || "",
          });
          dispatch({ type: "SET_DESCRIPTION", payload: draftCharacter.description || "" });
          dispatch({ type: "SET_NICKNAME", payload: draftCharacter.nickname || "" });
          dispatch({ type: "SET_CREATOR", payload: draftCharacter.creator || "" });
          dispatch({ type: "SET_CREATOR_NOTES", payload: draftCharacter.creatorNotes || "" });
          dispatch({
            type: "SET_CREATOR_NOTES_MULTILINGUAL_TEXT",
            payload: draftCharacter.creatorNotesMultilingual
              ? JSON.stringify(draftCharacter.creatorNotesMultilingual, null, 2)
              : "",
          });
          dispatch({
            type: "SET_TAGS_TEXT",
            payload: Array.isArray(draftCharacter.tags) ? draftCharacter.tags.join(", ") : "",
          });
          dispatch({
            type: "SET_IMPORTED_CHARACTER_BOOK",
            payload: draftCharacter.characterBook ?? null,
          });
          dispatch({ type: "SET_AVATAR_PATH", payload: draftCharacter.avatarPath || "" });
          dispatch({
            type: "SET_AVATAR_CROP",
            payload: draftCharacter.avatarCrop ?? null,
          });
          dispatch({
            type: "SET_BACKGROUND_IMAGE_PATH",
            payload: draftCharacter.backgroundImagePath || "",
          });
          dispatch({
            type: "SET_DISABLE_AVATAR_GRADIENT",
            payload: draftCharacter.disableAvatarGradient ?? false,
          });
          dispatch({
            type: "SET_SELECTED_MODEL_ID",
            payload:
              draftCharacter.defaultModelId ||
              settings.defaultModelId ||
              settings.models[0]?.id ||
              null,
          });
          dispatch({
            type: "SET_SELECTED_FALLBACK_MODEL_ID",
            payload: draftCharacter.fallbackModelId || null,
          });
          dispatch({
            type: "SET_SYSTEM_PROMPT_TEMPLATE_ID",
            payload: draftCharacter.promptTemplateId || null,
          });

          if (draftCharacter.scenes && draftCharacter.scenes.length > 0) {
            const mappedScenes = draftCharacter.scenes.map((s: any) => ({
              id: s.id || crypto.randomUUID(),
              content: s.content || "",
              createdAt: Date.now(),
              direction: s.direction || null,
              variants: [],
            }));
            dispatch({ type: "SET_SCENES", payload: mappedScenes });
            dispatch({
              type: "SET_DEFAULT_SCENE_ID",
              payload: draftCharacter.defaultSceneId || mappedScenes[0].id,
            });
          }
        } else {
          const defaultId = settings.defaultModelId ?? settings.models[0]?.id ?? null;
          dispatch({ type: "SET_SELECTED_MODEL_ID", payload: defaultId });
        }

        const dynamicEnabled = settings.advancedSettings?.dynamicMemory?.enabled ?? false;
        dispatch({ type: "SET_DYNAMIC_MEMORY_ENABLED", payload: dynamicEnabled });
        if (dynamicEnabled) {
          if (!draftCharacter?.memoryType) {
            dispatch({ type: "SET_MEMORY_TYPE", payload: "dynamic" });
          }
        } else {
          dispatch({ type: "SET_MEMORY_TYPE", payload: "manual" });
        }

        const filteredTemplates = templates.filter(
          (template) =>
            isSystemPromptTemplate(template.id) && template.id !== APP_DEFAULT_TEMPLATE_ID,
        );
        dispatch({ type: "SET_PROMPT_TEMPLATES", payload: filteredTemplates });
      } catch (err) {
        console.error("Failed to load settings", err);
      } finally {
        if (!cancelled) {
          dispatch({ type: "SET_LOADING_MODELS", payload: false });
          dispatch({ type: "SET_LOADING_TEMPLATES", payload: false });
        }
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [draftCharacter]);

  // Auto-set default scene if there's only one scene
  useEffect(() => {
    if (state.scenes.length === 1 && !state.defaultSceneId) {
      dispatch({ type: "SET_DEFAULT_SCENE_ID", payload: state.scenes[0].id });
    }
  }, [state.scenes, state.defaultSceneId]);

  useEffect(() => {
    if (!state.dynamicMemoryEnabled && state.memoryType !== "manual") {
      dispatch({ type: "SET_MEMORY_TYPE", payload: "manual" });
    }
  }, [state.dynamicMemoryEnabled, state.memoryType]);

  // Actions
  const setStep = useCallback(
    (step: Step) => {
      dispatch({ type: "SET_STEP", payload: step });
    },
    [state.dynamicMemoryEnabled],
  );

  const setName = useCallback((name: string) => {
    dispatch({ type: "SET_NAME", payload: name });
  }, []);

  const setAvatarPath = useCallback((path: string) => {
    dispatch({ type: "SET_AVATAR_PATH", payload: path });
    dispatch({ type: "SET_IMPORTING_AVATAR", payload: false });
    dispatch({ type: "SET_AVATAR_IMPORT_ERROR", payload: null });
  }, []);

  const setAvatarCrop = useCallback((crop: AvatarCrop | null) => {
    dispatch({ type: "SET_AVATAR_CROP", payload: crop });
  }, []);

  const setAvatarRoundPath = useCallback((path: string | null) => {
    dispatch({ type: "SET_AVATAR_ROUND_PATH", payload: path });
  }, []);

  const setBackgroundImagePath = useCallback((path: string) => {
    dispatch({ type: "SET_BACKGROUND_IMAGE_PATH", payload: path });
  }, []);

  const setScenes = useCallback((scenes: Scene[]) => {
    dispatch({ type: "SET_SCENES", payload: scenes });
  }, []);

  const setDefaultSceneId = useCallback((id: string | null) => {
    dispatch({ type: "SET_DEFAULT_SCENE_ID", payload: id });
  }, []);

  const setDescription = useCallback((description: string) => {
    dispatch({ type: "SET_DESCRIPTION", payload: description });
  }, []);

  const setNickname = useCallback((nickname: string) => {
    dispatch({ type: "SET_NICKNAME", payload: nickname });
  }, []);

  const setCreator = useCallback((creator: string) => {
    dispatch({ type: "SET_CREATOR", payload: creator });
  }, []);

  const setCreatorNotes = useCallback((creatorNotes: string) => {
    dispatch({ type: "SET_CREATOR_NOTES", payload: creatorNotes });
  }, []);

  const setCreatorNotesMultilingualText = useCallback((text: string) => {
    dispatch({ type: "SET_CREATOR_NOTES_MULTILINGUAL_TEXT", payload: text });
  }, []);

  const setTagsText = useCallback((tagsText: string) => {
    dispatch({ type: "SET_TAGS_TEXT", payload: tagsText });
  }, []);

  const setDefinition = useCallback((definition: string) => {
    dispatch({ type: "SET_DEFINITION", payload: definition });
  }, []);

  const setSelectedModelId = useCallback(
    (id: string | null) => {
      dispatch({ type: "SET_SELECTED_MODEL_ID", payload: id });
      if (id && state.selectedFallbackModelId === id) {
        dispatch({ type: "SET_SELECTED_FALLBACK_MODEL_ID", payload: null });
      }
    },
    [state.selectedFallbackModelId],
  );

  const setSelectedFallbackModelId = useCallback((id: string | null) => {
    dispatch({ type: "SET_SELECTED_FALLBACK_MODEL_ID", payload: id });
  }, []);

  const setSystemPromptTemplateId = useCallback((id: string | null) => {
    dispatch({ type: "SET_SYSTEM_PROMPT_TEMPLATE_ID", payload: id });
  }, []);

  const setMemoryType = useCallback((memoryType: "manual" | "dynamic") => {
    dispatch({ type: "SET_MEMORY_TYPE", payload: memoryType });
  }, []);

  const setDisableAvatarGradient = useCallback((value: boolean) => {
    dispatch({ type: "SET_DISABLE_AVATAR_GRADIENT", payload: value });
  }, []);

  const setVoiceConfig = useCallback((value: CharacterVoiceConfig | null) => {
    dispatch({ type: "SET_VOICE_CONFIG", payload: value });
  }, []);

  const setVoiceAutoplay = useCallback((value: boolean) => {
    dispatch({ type: "SET_VOICE_AUTOPLAY", payload: value });
  }, []);

  const handleAvatarUpload = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    const reader = new FileReader();
    reader.onload = () => {
      dispatch({ type: "SET_AVATAR_PATH", payload: reader.result as string });
      dispatch({ type: "SET_AVATAR_CROP", payload: null });
      dispatch({ type: "SET_AVATAR_ROUND_PATH", payload: null });
    };
    reader.readAsDataURL(file);
  }, []);

  const handleBackgroundImageUpload = useCallback((event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    const input = event.target;
    void processBackgroundImage(file)
      .then((dataUrl: any) => {
        dispatch({ type: "SET_BACKGROUND_IMAGE_PATH", payload: dataUrl });
      })
      .catch((error: any) => {
        console.warn("CharacterForm: failed to process background image", error);
      })
      .finally(() => {
        input.value = "";
      });
  }, []);

  const handleImport = useCallback(async (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    try {
      dispatch({ type: "SET_ERROR", payload: null });
      const jsonContent = await readFileAsText(file);

      const characterData = await previewCharacterImport(jsonContent);
      const settings = await readSettings();
      const legacyState = settings.appState as unknown as Record<string, unknown>;
      const autoDownloadCharacterCardAvatars =
        settings.appState.autoDownloadCharacterCardAvatars ??
        (typeof legacyState.autoDownloadDiscoveryAvatars === "boolean"
          ? legacyState.autoDownloadDiscoveryAvatars
          : true);
      if (characterData.fileFormat) {
        const label = FORMAT_LABELS[characterData.fileFormat] || characterData.fileFormat;
        if (characterData.fileFormat === "legacy_json") {
          toast.warning(
            "Legacy JSON import detected",
            "JSON imports are deprecated and will be removed soon. Use Settings > Convert Files.",
          );
        } else {
          toast.success("Import ready", `Detected ${label}`);
        }
      }

      const sceneIdMap = new Map<string, string>();

      const newScenes = characterData.scenes.map((scene) => {
        const newSceneId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
        sceneIdMap.set(scene.id, newSceneId);

        const variantIdMap = new Map<string, string>();

        const newVariants =
          scene.variants?.map((variant) => {
            const newVariantId =
              globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
            variantIdMap.set(variant.id, newVariantId);

            return {
              id: newVariantId,
              content: variant.content,
              createdAt: Date.now(),
            };
          }) || [];

        const newSelectedVariantId = scene.selectedVariantId
          ? variantIdMap.get(scene.selectedVariantId) || null
          : null;

        return {
          id: newSceneId,
          content: scene.content,
          createdAt: Date.now(),
          selectedVariantId: newSelectedVariantId,
          variants: newVariants,
        };
      });

      const newDefaultSceneId = characterData.defaultSceneId
        ? sceneIdMap.get(characterData.defaultSceneId) || newScenes[0]?.id || null
        : newScenes[0]?.id || null;
      const defaultSceneDirection = characterData.scenario?.trim();
      const scenesWithScenario = defaultSceneDirection
        ? newScenes.map((scene) =>
            scene.id === newDefaultSceneId ? { ...scene, direction: defaultSceneDirection } : scene,
          )
        : newScenes;

      dispatch({ type: "SET_NAME", payload: characterData.name });
      dispatch({
        type: "SET_DEFINITION",
        payload: characterData.definition || characterData.description || "",
      });
      dispatch({ type: "SET_DESCRIPTION", payload: characterData.description || "" });
      dispatch({ type: "SET_NICKNAME", payload: characterData.nickname || "" });
      dispatch({ type: "SET_CREATOR", payload: characterData.creator || "" });
      dispatch({ type: "SET_CREATOR_NOTES", payload: characterData.creatorNotes || "" });
      dispatch({
        type: "SET_CREATOR_NOTES_MULTILINGUAL_TEXT",
        payload: characterData.creatorNotesMultilingual
          ? JSON.stringify(characterData.creatorNotesMultilingual, null, 2)
          : "",
      });
      dispatch({
        type: "SET_TAGS_TEXT",
        payload: Array.isArray(characterData.tags) ? characterData.tags.join(", ") : "",
      });
      dispatch({ type: "SET_SCENES", payload: scenesWithScenario });
      dispatch({ type: "SET_DEFAULT_SCENE_ID", payload: newDefaultSceneId });
      dispatch({
        type: "SET_IMPORTED_CHARACTER_BOOK",
        payload: characterData.characterBook ?? null,
      });
      dispatch({
        type: "SET_DISABLE_AVATAR_GRADIENT",
        payload: characterData.disableAvatarGradient || false,
      });
      dispatch({
        type: "SET_SYSTEM_PROMPT_TEMPLATE_ID",
        payload: characterData.promptTemplateId || null,
      });
      const importedMemoryType = characterData.memoryType === "dynamic" ? "dynamic" : "manual";
      dispatch({
        type: "SET_MEMORY_TYPE",
        payload: state.dynamicMemoryEnabled ? importedMemoryType : "manual",
      });

      if (characterData.avatarData) {
        if (/^https?:\/\//i.test(characterData.avatarData)) {
          if (!autoDownloadCharacterCardAvatars) {
            dispatch({ type: "SET_IMPORTING_AVATAR", payload: false });
            dispatch({
              type: "SET_AVATAR_IMPORT_ERROR",
              payload:
                "Remote avatar download is disabled in Security settings.\nUpload an avatar manually.",
            });
            dispatch({ type: "SET_AVATAR_PATH", payload: "" });
            dispatch({ type: "SET_AVATAR_ROUND_PATH", payload: null });
            dispatch({ type: "SET_AVATAR_CROP", payload: null });
          } else {
            dispatch({ type: "SET_IMPORTING_AVATAR", payload: true });
            dispatch({ type: "SET_AVATAR_IMPORT_ERROR", payload: null });
            dispatch({ type: "SET_AVATAR_PATH", payload: "" });
            dispatch({ type: "SET_AVATAR_ROUND_PATH", payload: null });
            dispatch({ type: "SET_AVATAR_CROP", payload: null });

            const imageDataUrl = await new Promise<string>((resolve, reject) => {
              const image = new Image();
              image.crossOrigin = "anonymous";

              const cleanup = () => {
                image.onload = null;
                image.onerror = null;
              };

              image.onload = () => {
                try {
                  const canvas = document.createElement("canvas");
                  canvas.width = image.naturalWidth;
                  canvas.height = image.naturalHeight;
                  const ctx = canvas.getContext("2d");
                  if (!ctx) {
                    cleanup();
                    reject(new Error("Failed to process avatar image"));
                    return;
                  }
                  ctx.drawImage(image, 0, 0);
                  const dataUrl = canvas.toDataURL("image/png");
                  cleanup();
                  resolve(dataUrl);
                } catch {
                  cleanup();
                  reject(new Error("Avatar URL could not be converted"));
                }
              };

              image.onerror = () => {
                cleanup();
                reject(new Error("Failed to load avatar URL"));
              };

              image.src = characterData.avatarData as string;
            });

            dispatch({ type: "SET_AVATAR_PATH", payload: imageDataUrl });
            dispatch({ type: "SET_AVATAR_ROUND_PATH", payload: null });
            dispatch({ type: "SET_AVATAR_CROP", payload: null });
            dispatch({ type: "SET_IMPORTING_AVATAR", payload: false });
          }
        } else {
          dispatch({ type: "SET_AVATAR_PATH", payload: characterData.avatarData });
          dispatch({ type: "SET_AVATAR_ROUND_PATH", payload: null });
          dispatch({ type: "SET_AVATAR_CROP", payload: null });
          dispatch({ type: "SET_IMPORTING_AVATAR", payload: false });
          dispatch({ type: "SET_AVATAR_IMPORT_ERROR", payload: null });
        }
      } else {
        dispatch({ type: "SET_IMPORTING_AVATAR", payload: false });
      }
      if (characterData.avatarCrop) {
        dispatch({ type: "SET_AVATAR_CROP", payload: characterData.avatarCrop });
      }

      if (characterData.backgroundImageData) {
        dispatch({
          type: "SET_BACKGROUND_IMAGE_PATH",
          payload: characterData.backgroundImageData,
        });
      }

      console.log("[handleImport] Character data loaded into form, ready to save");
    } catch (error: any) {
      console.error("Failed to import character:", error);
      dispatch({ type: "SET_ERROR", payload: error?.message || "Failed to import character" });
      dispatch({ type: "SET_IMPORTING_AVATAR", payload: false });
      if (
        String(error?.message || "")
          .toLowerCase()
          .includes("avatar")
      ) {
        dispatch({
          type: "SET_AVATAR_IMPORT_ERROR",
          payload: error?.message || "Avatar URL failed to load",
        });
      }
    } finally {
      // Clear the file input
      event.target.value = "";
    }
  }, []);

  const handleSave = useCallback(async () => {
    if (
      state.definition.trim().length === 0 ||
      state.selectedModelId === null ||
      state.saving ||
      state.scenes.length === 0
    ) {
      return;
    }

    const resolveErrorMessage = (err: unknown, fallback: string) => {
      if (typeof err === "string") return err;
      if (!err || typeof err !== "object") return fallback;
      const anyErr = err as any;
      if (anyErr.message) return String(anyErr.message);
      if (anyErr.error) {
        if (typeof anyErr.error === "string") return anyErr.error;
        if (anyErr.error?.message) return String(anyErr.error.message);
        try {
          return JSON.stringify(anyErr.error);
        } catch {
          return fallback;
        }
      }
      return fallback;
    };

    try {
      dispatch({ type: "SET_SAVING", payload: true });
      dispatch({ type: "SET_ERROR", payload: null });

      const parseCommaSeparated = (raw: string): string[] =>
        raw
          .split(",")
          .map((item) => item.trim())
          .filter((item) => item.length > 0);
      const tags = parseCommaSeparated(state.tagsText);
      let creatorNotesMultilingual: Record<string, string> | null | undefined = undefined;
      if (state.creatorNotesMultilingualText.trim()) {
        try {
          const parsed = JSON.parse(state.creatorNotesMultilingualText);
          if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) {
            throw new Error("creatorNotesMultilingual must be a JSON object");
          }
          const normalized: Record<string, string> = {};
          for (const [key, value] of Object.entries(parsed as Record<string, unknown>)) {
            if (typeof value === "string") normalized[key] = value;
          }
          creatorNotesMultilingual = normalized;
        } catch {
          dispatch({
            type: "SET_ERROR",
            payload: "Creator notes multilingual must be valid JSON object",
          });
          return false;
        }
      }

      console.log(
        "[CreateCharacter] Saving with avatarPath:",
        state.avatarPath ? "present" : "empty",
      );
      console.log(
        "[CreateCharacter] Saving with backgroundImagePath:",
        state.backgroundImagePath ? "present" : "empty",
      );

      // Generate character ID first so we can use it for avatar storage
      const characterId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
      console.log("[CreateCharacter] Generated character ID:", characterId);

      // Save avatar using new centralized system
      let avatarFilename: string | undefined = undefined;
      if (state.avatarPath) {
        avatarFilename = await saveAvatar(
          "character",
          characterId,
          state.avatarPath,
          state.avatarRoundPath,
        );
        if (!avatarFilename) {
          console.error("[CreateCharacter] Failed to save avatar image");
        } else {
          console.log("[CreateCharacter] Avatar saved as:", avatarFilename);
          invalidateAvatarCache("character", characterId);
        }
      }

      // Background images still use the old system (they're stored per-session)
      let backgroundImageId: string | undefined = undefined;
      if (state.backgroundImagePath) {
        backgroundImageId = await convertToImageRef(state.backgroundImagePath);
        if (!backgroundImageId) {
          console.error("[CreateCharacter] Failed to save background image");
        }
      }

      console.log("[CreateCharacter] Avatar filename:", avatarFilename || "none");
      console.log("[CreateCharacter] Background image ID:", backgroundImageId || "none");

      const characterData = {
        id: characterId,
        name: state.name.trim(),
        avatarPath: avatarFilename || undefined,
        avatarCrop: avatarFilename ? (state.avatarCrop ?? undefined) : undefined,
        backgroundImagePath: backgroundImageId || undefined,
        definition: state.definition.trim(),
        description: state.description.trim() || undefined,
        nickname: state.nickname.trim() || undefined,
        creator: state.creator.trim() || undefined,
        creatorNotes: state.creatorNotes.trim() || undefined,
        creatorNotesMultilingual,
        tags: tags.length > 0 ? tags : undefined,
        scenes: state.scenes,
        defaultSceneId: state.defaultSceneId || state.scenes[0]?.id || null,
        defaultModelId: state.selectedModelId,
        fallbackModelId: state.selectedFallbackModelId,
        promptTemplateId: state.systemPromptTemplateId,
        memoryType: state.dynamicMemoryEnabled ? state.memoryType : "manual",
        disableAvatarGradient: state.disableAvatarGradient,
        voiceConfig: state.voiceConfig || undefined,
        voiceAutoplay: state.voiceAutoplay,
      };

      console.log(
        "[CreateCharacter] Saving character data:",
        JSON.stringify(characterData, null, 2),
      );

      await saveCharacter(characterData);

      if (state.importedCharacterBook?.entries?.length) {
        const lorebook = await saveLorebook({
          name: state.importedCharacterBook.name?.trim() || `${state.name.trim()} Lorebook`,
        });

        const sanitize = (value: unknown): string =>
          typeof value === "string" ? value.trim() : "";

        for (let i = 0; i < state.importedCharacterBook.entries.length; i += 1) {
          const item = state.importedCharacterBook.entries[i];
          const keys = [
            ...(Array.isArray(item.keys) ? item.keys : []),
            ...(Array.isArray(item.secondary_keys) ? item.secondary_keys : []),
          ]
            .map(sanitize)
            .filter(Boolean);

          await saveLorebookEntry({
            lorebookId: lorebook.id,
            title: sanitize(item.name) || keys[0] || `Entry ${i + 1}`,
            content: sanitize(item.content),
            keywords: Array.from(new Set(keys)),
            enabled: item.enabled !== false,
            caseSensitive: item.case_sensitive === true,
            alwaysActive: item.constant === true,
            priority: typeof item.priority === "number" ? item.priority : 0,
            displayOrder: typeof item.insertion_order === "number" ? item.insertion_order : i,
          });
        }

        await setCharacterLorebooks(characterId, [lorebook.id]);
      }

      return true; // Success
    } catch (e: any) {
      console.error("Failed to save character:", e);
      dispatch({ type: "SET_ERROR", payload: resolveErrorMessage(e, "Failed to save character") });
      return false; // Failure
    } finally {
      dispatch({ type: "SET_SAVING", payload: false });
    }
  }, [
    state.name,
    state.avatarPath,
    state.avatarCrop,
    state.avatarRoundPath,
    state.backgroundImagePath,
    state.scenes,
    state.defaultSceneId,
    state.definition,
    state.description,
    state.nickname,
    state.creator,
    state.creatorNotes,
    state.creatorNotesMultilingualText,
    state.tagsText,
    state.importedCharacterBook,
    state.selectedModelId,
    state.selectedFallbackModelId,
    state.systemPromptTemplateId,
    state.memoryType,
    state.dynamicMemoryEnabled,
    state.voiceConfig,
    state.voiceAutoplay,
    state.saving,
  ]);

  // Computed values
  const canContinueIdentity =
    state.name.trim().length > 0 && !state.saving && !state.importingAvatar;
  const canContinueStartingScene = state.scenes.length > 0 && !state.saving;
  const canSaveDescription =
    state.definition.trim().length > 0 && state.selectedModelId !== null && !state.saving;
  const progress =
    state.step === Step.Identity
      ? 0.25
      : state.step === Step.StartingScene
        ? 0.5
        : state.step === Step.Description
          ? 0.75
          : 1;

  return {
    state,
    actions: {
      setStep,
      setName,
      setAvatarPath,
      setAvatarCrop,
      setAvatarRoundPath,
      setBackgroundImagePath,
      setScenes,
      setDefaultSceneId,
      setDefinition,
      setDescription,
      setNickname,
      setCreator,
      setCreatorNotes,
      setCreatorNotesMultilingualText,
      setTagsText,
      setSelectedModelId,
      setSelectedFallbackModelId,
      setSystemPromptTemplateId,
      setMemoryType,
      setDisableAvatarGradient,
      setVoiceConfig,
      setVoiceAutoplay,
      handleAvatarUpload,
      handleBackgroundImageUpload,
      handleImport,
      handleSave,
    },
    computed: {
      canContinueIdentity,
      canContinueStartingScene,
      canSaveDescription,
      progress,
    },
  };
}
