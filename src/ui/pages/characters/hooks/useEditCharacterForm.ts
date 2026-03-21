import { useCallback, useEffect, useReducer, useRef, type ChangeEvent } from "react";
import { useNavigate } from "react-router-dom";
import { listCharacters, saveCharacter, readSettings } from "../../../../core/storage/repo";
import type {
  AvatarCrop,
  ChatTemplate,
  CharacterVoiceConfig,
  Model,
  Scene,
  SystemPromptTemplate,
} from "../../../../core/storage/schemas";
import { processBackgroundImage } from "../../../../core/utils/image";
import { convertToImageRef, deleteImageRef } from "../../../../core/storage/images";
import { saveAvatar, loadAvatar } from "../../../../core/storage/avatars";
import { listPromptTemplates } from "../../../../core/prompts/service";
import { invalidateAvatarCache } from "../../../hooks/useAvatar";
import {
  exportCharacterWithFormat,
  downloadJson,
  generateExportFilenameWithFormat,
  type CharacterFileFormat,
} from "../../../../core/storage/characterTransfer";
import {
  APP_DEFAULT_TEMPLATE_ID,
  isSystemPromptTemplate,
} from "../../../../core/prompts/constants";
import { isRenderableImageUrl } from "../../../../core/utils/image";

type EditCharacterState = {
  loading: boolean;
  saving: boolean;
  exporting: boolean;
  error: string | null;
  name: string;
  definition: string;
  description: string;
  scenario: string;
  nickname: string;
  creator: string;
  creatorNotes: string;
  creatorNotesMultilingualText: string;
  sourceText: string;
  tagsText: string;
  avatarPath: string;
  avatarCrop: AvatarCrop | null;
  avatarRoundPath: string | null;
  designDescription: string;
  designReferenceImageIds: string[];
  backgroundImagePath: string;
  scenes: Scene[];
  chatTemplates: ChatTemplate[];
  defaultSceneId: string | null;
  defaultChatTemplateId: string | null;
  newSceneContent: string;
  newSceneDirection: string;
  selectedModelId: string | null;
  selectedFallbackModelId: string | null;
  systemPromptTemplateId: string | null;
  voiceConfig: CharacterVoiceConfig | null;
  voiceAutoplay: boolean;

  disableAvatarGradient: boolean;
  customGradientEnabled: boolean;
  customGradientColors: string[];
  customTextColor: string;
  customTextSecondary: string;
  memoryType: "manual" | "dynamic";
  dynamicMemoryEnabled: boolean;
  models: Model[];
  loadingModels: boolean;
  promptTemplates: SystemPromptTemplate[];
  loadingTemplates: boolean;
  editingSceneId: string | null;
  editingSceneContent: string;
  editingSceneDirection: string;
};

type EditCharacterAction =
  | { type: "SET_LOADING"; payload: boolean }
  | { type: "SET_SAVING"; payload: boolean }
  | { type: "SET_EXPORTING"; payload: boolean }
  | { type: "SET_ERROR"; payload: string | null }
  | { type: "SET_FIELDS"; payload: Partial<EditCharacterState> };

const initialState: EditCharacterState = {
  loading: true,
  saving: false,
  exporting: false,
  error: null,
  name: "",
  definition: "",
  description: "",
  scenario: "",
  nickname: "",
  creator: "",
  creatorNotes: "",
  creatorNotesMultilingualText: "",
  sourceText: "",
  tagsText: "",
  avatarPath: "",
  avatarCrop: null,
  avatarRoundPath: null,
  designDescription: "",
  designReferenceImageIds: [],
  backgroundImagePath: "",
  scenes: [],
  chatTemplates: [],
  defaultSceneId: null,
  defaultChatTemplateId: null,
  newSceneContent: "",
  newSceneDirection: "",
  selectedModelId: null,
  selectedFallbackModelId: null,
  systemPromptTemplateId: null,
  voiceConfig: null,
  voiceAutoplay: false,

  disableAvatarGradient: false,
  customGradientEnabled: false,
  customGradientColors: [],
  customTextColor: "",
  customTextSecondary: "",
  memoryType: "manual",
  dynamicMemoryEnabled: false,
  models: [],
  loadingModels: false,
  promptTemplates: [],
  loadingTemplates: false,
  editingSceneId: null,
  editingSceneContent: "",
  editingSceneDirection: "",
};

function reducer(state: EditCharacterState, action: EditCharacterAction): EditCharacterState {
  switch (action.type) {
    case "SET_LOADING":
      return { ...state, loading: action.payload };
    case "SET_SAVING":
      return { ...state, saving: action.payload };
    case "SET_EXPORTING":
      return { ...state, exporting: action.payload };
    case "SET_ERROR":
      return { ...state, error: action.payload };
    case "SET_FIELDS":
      return { ...state, ...action.payload };
    default:
      return state;
  }
}

export function useEditCharacterForm(characterId: string | undefined) {
  const navigate = useNavigate();
  const [state, dispatch] = useReducer(reducer, initialState);
  const avatarInitial = state.name.trim().charAt(0).toUpperCase() || "?";

  // Track initial state for change detection
  const initialStateRef = useRef<{
    name: string;
    definition: string;
    description: string;
    scenario: string;
    nickname: string;
    creator: string;
    creatorNotes: string;
    creatorNotesMultilingualText: string;
    sourceText: string;
    tagsText: string;
    avatarPath: string;
    avatarCrop: string;
    avatarRoundPath: string;
    designDescription: string;
    designReferenceImageIds: string;
    backgroundImagePath: string;
    scenes: string;
    chatTemplates: string;
    defaultSceneId: string | null;
    defaultChatTemplateId: string | null;
    selectedModelId: string | null;
    selectedFallbackModelId: string | null;
    systemPromptTemplateId: string | null;
    disableAvatarGradient: boolean;
    customGradientEnabled: boolean;
    customGradientColors: string;
    memoryType: string;
    voiceConfig: string;
    voiceAutoplay: boolean;
  } | null>(null);
  const persistedMediaRef = useRef<{
    avatarFilename?: string;
    avatarUrl?: string;
    backgroundImageId?: string;
    backgroundImageUrl?: string;
  }>({});

  const setError = useCallback(
    (value: string | null) => dispatch({ type: "SET_ERROR", payload: value }),
    [],
  );
  const setSaving = useCallback(
    (value: boolean) => dispatch({ type: "SET_SAVING", payload: value }),
    [],
  );
  const setExporting = useCallback(
    (value: boolean) => dispatch({ type: "SET_EXPORTING", payload: value }),
    [],
  );
  const setLoading = useCallback(
    (value: boolean) => dispatch({ type: "SET_LOADING", payload: value }),
    [],
  );
  const setFields = useCallback(
    (payload: Partial<EditCharacterState>) => dispatch({ type: "SET_FIELDS", payload }),
    [],
  );

  // Auto-set default scene if there's only one scene
  useEffect(() => {
    if (state.scenes.length === 1 && !state.defaultSceneId) {
      setFields({ defaultSceneId: state.scenes[0].id });
    }
  }, [state.scenes, state.defaultSceneId, setFields]);

  useEffect(() => {
    if (!state.dynamicMemoryEnabled && state.memoryType !== "manual") {
      setFields({ memoryType: "manual" });
    }
  }, [setFields, state.dynamicMemoryEnabled, state.memoryType]);

  const loadCharacter = useCallback(async () => {
    if (!characterId) return;

    try {
      setLoading(true);
      const allCharacters = await listCharacters();
      const character = allCharacters.find((c) => c.id === characterId);
      if (!character) {
        navigate("/chat");
        return;
      }

      let loadedAvatarPath = "";
      let loadedAvatarRoundPath: string | null = null;
      let backgroundImage = character.backgroundImagePath || "";

      if (character.avatarPath) {
        try {
          const avatarUrl = await loadAvatar("character", character.id, character.avatarPath);
          const avatarRoundUrl = await loadAvatar(
            "character",
            character.id,
            "avatar_round.webp",
          ).catch(() => undefined);
          loadedAvatarPath = avatarUrl || "";
          loadedAvatarRoundPath = avatarRoundUrl || null;
        } catch (err) {
          console.warn("Failed to load avatar:", err);
          loadedAvatarPath = "";
          loadedAvatarRoundPath = null;
        }
      } else {
        loadedAvatarPath = "";
      }

      if (
        backgroundImage &&
        !isRenderableImageUrl(backgroundImage) &&
        backgroundImage.length === 36
      ) {
        try {
          const { convertToImageUrl } = await import("../../../../core/storage/images");
          const assetUrl = await convertToImageUrl(backgroundImage);
          backgroundImage = assetUrl || backgroundImage;
        } catch (err) {
          console.warn("Failed to convert background image ID to URL:", err);
        }
      }

      persistedMediaRef.current = {
        avatarFilename: character.avatarPath ?? undefined,
        avatarUrl: loadedAvatarPath || undefined,
        backgroundImageId: character.backgroundImagePath ?? undefined,
        backgroundImageUrl: backgroundImage || undefined,
      };

      setFields({
        name: character.name,
        definition: character.definition || character.description || "",
        description: character.description || "",
        scenario: character.scenario || "",
        nickname: character.nickname || "",
        creator: character.creator || "",
        creatorNotes: character.creatorNotes || "",
        creatorNotesMultilingualText: character.creatorNotesMultilingual
          ? JSON.stringify(character.creatorNotesMultilingual, null, 2)
          : "",
        sourceText: Array.isArray(character.source) ? character.source.join(", ") : "",
        tagsText: Array.isArray(character.tags) ? character.tags.join(", ") : "",
        avatarPath: loadedAvatarPath,
        avatarCrop: character.avatarCrop ?? null,
        avatarRoundPath: loadedAvatarRoundPath,
        designDescription: character.designDescription || "",
        designReferenceImageIds: Array.isArray(character.designReferenceImageIds)
          ? character.designReferenceImageIds
          : [],
        backgroundImagePath: backgroundImage,
        scenes: character.scenes || [],
        chatTemplates: character.chatTemplates || [],
        defaultSceneId: character.defaultSceneId || null,
        defaultChatTemplateId: character.defaultChatTemplateId || null,
        selectedModelId: character.defaultModelId || null,
        selectedFallbackModelId: character.fallbackModelId || null,
        systemPromptTemplateId: character.promptTemplateId || null,
        voiceConfig: character.voiceConfig ?? null,
        voiceAutoplay: character.voiceAutoplay ?? false,

        disableAvatarGradient: character.disableAvatarGradient || false,
        customGradientEnabled: character.customGradientEnabled || false,
        customGradientColors: character.customGradientColors || [],
        customTextColor: character.customTextColor || "",
        customTextSecondary: character.customTextSecondary || "",
        memoryType: character.memoryType === "dynamic" ? "dynamic" : "manual",
      });

      // Store initial state for change detection
      initialStateRef.current = {
        name: character.name,
        definition: character.definition || character.description || "",
        description: character.description || "",
        scenario: character.scenario || "",
        nickname: character.nickname || "",
        creator: character.creator || "",
        creatorNotes: character.creatorNotes || "",
        creatorNotesMultilingualText: character.creatorNotesMultilingual
          ? JSON.stringify(character.creatorNotesMultilingual, null, 2)
          : "",
        sourceText: Array.isArray(character.source) ? character.source.join(", ") : "",
        tagsText: Array.isArray(character.tags) ? character.tags.join(", ") : "",
        avatarPath: loadedAvatarPath,
        avatarCrop: JSON.stringify(character.avatarCrop ?? null),
        avatarRoundPath: JSON.stringify(loadedAvatarRoundPath ?? null),
        designDescription: character.designDescription || "",
        designReferenceImageIds: JSON.stringify(character.designReferenceImageIds || []),
        backgroundImagePath: backgroundImage,
        scenes: JSON.stringify(character.scenes || []),
        chatTemplates: JSON.stringify(character.chatTemplates || []),
        defaultSceneId: character.defaultSceneId || null,
        defaultChatTemplateId: character.defaultChatTemplateId || null,
        selectedModelId: character.defaultModelId || null,
        selectedFallbackModelId: character.fallbackModelId || null,
        systemPromptTemplateId: character.promptTemplateId || null,
        disableAvatarGradient: character.disableAvatarGradient || false,
        customGradientEnabled: character.customGradientEnabled || false,
        customGradientColors: JSON.stringify(character.customGradientColors || []),
        memoryType: character.memoryType === "dynamic" ? "dynamic" : "manual",
        voiceConfig: JSON.stringify(character.voiceConfig ?? null),
        voiceAutoplay: character.voiceAutoplay ?? false,
      };
      setError(null);
    } catch (err) {
      console.error("Failed to load character:", err);
      setError("Failed to load character");
    } finally {
      setLoading(false);
    }
  }, [characterId, setError, setFields, setLoading]);

  const loadModels = useCallback(async () => {
    try {
      setFields({ loadingModels: true });
      const settings = await readSettings();
      const dynamicEnabled = settings.advancedSettings?.dynamicMemory?.enabled ?? false;
      setFields({
        models: settings.models,
        dynamicMemoryEnabled: dynamicEnabled,
      });
    } catch (err) {
      console.error("Failed to load models:", err);
    } finally {
      setFields({ loadingModels: false });
    }
  }, [setFields]);

  const loadPromptTemplates = useCallback(async () => {
    try {
      setFields({ loadingTemplates: true });
      // Global list (scopes removed)
      const templates = await listPromptTemplates();
      const filtered = templates.filter(
        (template) =>
          isSystemPromptTemplate(template.id) && template.id !== APP_DEFAULT_TEMPLATE_ID,
      );
      setFields({ promptTemplates: filtered });
    } catch (err) {
      console.error("Failed to load prompt templates:", err);
    } finally {
      setFields({ loadingTemplates: false });
    }
  }, [setFields]);

  useEffect(() => {
    if (!characterId) {
      navigate("/chat");
      return;
    }

    void loadCharacter();
    void loadModels();
    void loadPromptTemplates();
  }, [characterId, loadCharacter, loadModels, loadPromptTemplates]);

  const handleSave = useCallback(async () => {
    if (!characterId || !state.name.trim() || !state.definition.trim()) return;

    try {
      setSaving(true);
      setError(null);
      const previousDesignReferenceImageIds = initialStateRef.current
        ? (JSON.parse(initialStateRef.current.designReferenceImageIds) as string[])
        : [];

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
          setError("Creator notes multilingual must be valid JSON object");
          return;
        }
      }

      // Save avatar using new centralized system if it's a new upload (data URL)
      let avatarFilename: string | undefined = undefined;
      if (state.avatarPath) {
        if (state.avatarPath.startsWith("data:")) {
          avatarFilename = await saveAvatar(
            "character",
            characterId,
            state.avatarPath,
            state.avatarRoundPath,
          );
          if (!avatarFilename) {
            console.error("[EditCharacter] Failed to save avatar image");
          } else {
            invalidateAvatarCache("character", characterId);
          }
        } else {
          avatarFilename =
            persistedMediaRef.current.avatarUrl &&
            state.avatarPath === persistedMediaRef.current.avatarUrl
              ? persistedMediaRef.current.avatarFilename
              : state.avatarPath;
        }
      } else {
        invalidateAvatarCache("character", characterId);
      }

      const backgroundImageId = state.backgroundImagePath
        ? state.backgroundImagePath.startsWith("data:")
          ? await convertToImageRef(state.backgroundImagePath)
          : persistedMediaRef.current.backgroundImageUrl &&
              state.backgroundImagePath === persistedMediaRef.current.backgroundImageUrl
            ? persistedMediaRef.current.backgroundImageId
            : state.backgroundImagePath
        : undefined;

      const designReferenceImageIds = (
        await Promise.all(
          state.designReferenceImageIds.map(async (value) => {
            if (!value) return null;
            if (value.startsWith("data:")) {
              const imageId = await convertToImageRef(value);
              return imageId || null;
            }
            return value;
          }),
        )
      ).filter((value): value is string => typeof value === "string" && value.length > 0);

      await saveCharacter({
        id: characterId,
        name: state.name.trim(),
        definition: state.definition.trim(),
        description: state.description.trim() || undefined,
        nickname: state.nickname.trim() || undefined,
        designDescription: state.designDescription.trim() || undefined,
        designReferenceImageIds:
          designReferenceImageIds.length > 0 ? designReferenceImageIds : undefined,
        creator: state.creator.trim() || undefined,
        creatorNotes: state.creatorNotes.trim() || undefined,
        creatorNotesMultilingual,
        tags: tags.length > 0 ? tags : undefined,
        avatarPath: avatarFilename,
        avatarCrop: avatarFilename ? (state.avatarCrop ?? undefined) : undefined,
        backgroundImagePath: backgroundImageId,
        scenes: state.scenes,
        chatTemplates: state.chatTemplates,
        defaultSceneId: state.defaultSceneId,
        defaultChatTemplateId: state.defaultChatTemplateId,
        defaultModelId: state.selectedModelId,
        fallbackModelId: state.selectedFallbackModelId,
        promptTemplateId: state.systemPromptTemplateId,
        voiceConfig: state.voiceConfig ?? undefined,
        voiceAutoplay: state.voiceAutoplay,

        disableAvatarGradient: state.disableAvatarGradient,
        customGradientEnabled: state.customGradientEnabled,
        customGradientColors:
          state.customGradientColors.length > 0 ? state.customGradientColors : undefined,
        customTextColor: state.customTextColor || undefined,
        customTextSecondary: state.customTextSecondary || undefined,
        memoryType: state.dynamicMemoryEnabled ? state.memoryType : "manual",
      });

      // Sync only name/definition/description with trimmed values
      setFields({
        name: state.name.trim(),
        definition: state.definition.trim(),
        description: state.description.trim(),
        scenario: state.scenario.trim(),
        nickname: state.nickname.trim(),
        designDescription: state.designDescription.trim(),
        designReferenceImageIds,
        creator: state.creator.trim(),
        creatorNotes: state.creatorNotes.trim(),
        creatorNotesMultilingualText: state.creatorNotesMultilingualText.trim(),
        sourceText: state.sourceText.trim(),
        tagsText: state.tagsText.trim(),
      });

      // Update initial state ref to match current state (for change detection)
      initialStateRef.current = {
        name: state.name.trim(),
        definition: state.definition.trim(),
        description: state.description.trim(),
        scenario: state.scenario.trim(),
        nickname: state.nickname.trim(),
        creator: state.creator.trim(),
        creatorNotes: state.creatorNotes.trim(),
        creatorNotesMultilingualText: state.creatorNotesMultilingualText.trim(),
        sourceText: state.sourceText.trim(),
        tagsText: state.tagsText.trim(),
        avatarPath: state.avatarPath,
        avatarCrop: JSON.stringify(state.avatarCrop ?? null),
        avatarRoundPath: JSON.stringify(state.avatarRoundPath ?? null),
        designDescription: state.designDescription.trim(),
        designReferenceImageIds: JSON.stringify(designReferenceImageIds),
        backgroundImagePath: state.backgroundImagePath,
        scenes: JSON.stringify(state.scenes),
        chatTemplates: JSON.stringify(state.chatTemplates),
        defaultSceneId: state.defaultSceneId,
        defaultChatTemplateId: state.defaultChatTemplateId,
        selectedModelId: state.selectedModelId,
        selectedFallbackModelId: state.selectedFallbackModelId,
        systemPromptTemplateId: state.systemPromptTemplateId,
        disableAvatarGradient: state.disableAvatarGradient,
        customGradientEnabled: state.customGradientEnabled,
        customGradientColors: JSON.stringify(state.customGradientColors),
        memoryType: state.dynamicMemoryEnabled ? state.memoryType : "manual",
        voiceConfig: JSON.stringify(state.voiceConfig ?? null),
        voiceAutoplay: state.voiceAutoplay,
      };
      const removedDesignReferenceImageIds = previousDesignReferenceImageIds.filter(
        (imageId) => !designReferenceImageIds.includes(imageId),
      );
      if (removedDesignReferenceImageIds.length > 0) {
        await Promise.all(removedDesignReferenceImageIds.map((imageId) => deleteImageRef(imageId)));
      }
    } catch (err: any) {
      console.error("Failed to save character:", err);
      setError(err?.message || "Failed to save character");
    } finally {
      setSaving(false);
    }
  }, [characterId, setError, setFields, setSaving, state]);

  const handleExport = useCallback(
    async (format: CharacterFileFormat = "uec") => {
      if (!characterId) return;

      try {
        setExporting(true);
        setError(null);

        const exportJson = await exportCharacterWithFormat(characterId, format);
        const filename = generateExportFilenameWithFormat(state.name || "character", format);
        await downloadJson(exportJson, filename);
      } catch (err: any) {
        console.error("Failed to export character:", err);
        setError(err?.message || "Failed to export character");
      } finally {
        setExporting(false);
      }
    },
    [characterId, setError, setExporting, state.name],
  );

  const addScene = useCallback(() => {
    if (!state.newSceneContent.trim()) return;

    const sceneId = globalThis.crypto?.randomUUID?.() ?? `${Date.now()}-${Math.random()}`;
    const timestamp = Date.now();

    const newScenes = [
      ...state.scenes,
      {
        id: sceneId,
        content: state.newSceneContent.trim(),
        direction: state.newSceneDirection.trim() || undefined,
        createdAt: timestamp,
      },
    ];

    setFields({
      scenes: newScenes,
      defaultSceneId: newScenes.length === 1 ? sceneId : state.defaultSceneId,
      newSceneContent: "",
      newSceneDirection: "",
    });
  }, [
    setFields,
    state.defaultSceneId,
    state.newSceneContent,
    state.newSceneDirection,
    state.scenes,
  ]);

  const deleteScene = useCallback(
    (sceneId: string) => {
      const newScenes = state.scenes.filter((s) => s.id !== sceneId);
      const nextDefaultSceneId =
        state.defaultSceneId === sceneId
          ? newScenes.length === 1
            ? newScenes[0].id
            : null
          : state.defaultSceneId;

      setFields({ scenes: newScenes, defaultSceneId: nextDefaultSceneId });
    },
    [setFields, state.defaultSceneId, state.scenes],
  );

  const startEditingScene = useCallback(
    (scene: Scene) => {
      setFields({
        editingSceneId: scene.id,
        editingSceneContent: scene.content,
        editingSceneDirection: scene.direction || "",
      });
    },
    [setFields],
  );

  const saveEditedScene = useCallback(() => {
    if (!state.editingSceneId || !state.editingSceneContent.trim()) return;

    const updatedScenes = state.scenes.map((scene) =>
      scene.id === state.editingSceneId
        ? {
            ...scene,
            content: state.editingSceneContent.trim(),
            direction: state.editingSceneDirection.trim() || undefined,
          }
        : scene,
    );

    setFields({
      scenes: updatedScenes,
      editingSceneId: null,
      editingSceneContent: "",
      editingSceneDirection: "",
    });
  }, [
    setFields,
    state.editingSceneContent,
    state.editingSceneDirection,
    state.editingSceneId,
    state.scenes,
  ]);

  const cancelEditingScene = useCallback(() => {
    setFields({
      editingSceneId: null,
      editingSceneContent: "",
      editingSceneDirection: "",
    });
  }, [setFields]);

  const handleBackgroundImageUpload = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      if (!file) return;

      const input = event.target;
      void processBackgroundImage(file)
        .then((dataUrl: string) => {
          setFields({ backgroundImagePath: dataUrl });
        })
        .catch((error: any) => {
          console.warn("EditCharacter: failed to process background image", error);
        })
        .finally(() => {
          input.value = "";
        });
    },
    [setFields],
  );

  const handleAvatarUpload = useCallback(
    (event: ChangeEvent<HTMLInputElement>) => {
      const file = event.target.files?.[0];
      if (!file) return;

      const reader = new FileReader();
      reader.onload = () => {
        setFields({
          avatarPath: reader.result as string,
          avatarCrop: null,
          avatarRoundPath: null,
        });
      };
      reader.readAsDataURL(file);

      event.target.value = "";
    },
    [setFields],
  );

  const resetToInitial = useCallback(() => {
    const initial = initialStateRef.current;
    if (!initial) return;
    setFields({
      name: initial.name,
      definition: initial.definition,
      description: initial.description,
      scenario: initial.scenario,
      nickname: initial.nickname,
      creator: initial.creator,
      creatorNotes: initial.creatorNotes,
      creatorNotesMultilingualText: initial.creatorNotesMultilingualText,
      sourceText: initial.sourceText,
      tagsText: initial.tagsText,
      avatarPath: initial.avatarPath,
      avatarCrop: JSON.parse(initial.avatarCrop) as AvatarCrop | null,
      avatarRoundPath: JSON.parse(initial.avatarRoundPath) as string | null,
      designDescription: initial.designDescription,
      designReferenceImageIds: JSON.parse(initial.designReferenceImageIds) as string[],
      backgroundImagePath: initial.backgroundImagePath,
      scenes: JSON.parse(initial.scenes) as Scene[],
      defaultSceneId: initial.defaultSceneId,
      selectedModelId: initial.selectedModelId,
      selectedFallbackModelId: initial.selectedFallbackModelId,
      systemPromptTemplateId: initial.systemPromptTemplateId,
      disableAvatarGradient: initial.disableAvatarGradient,
      customGradientEnabled: initial.customGradientEnabled,
      customGradientColors: JSON.parse(initial.customGradientColors) as string[],
      memoryType: initial.memoryType === "dynamic" ? "dynamic" : "manual",
      voiceConfig: JSON.parse(initial.voiceConfig) as CharacterVoiceConfig | null,
      voiceAutoplay: initial.voiceAutoplay,
      newSceneContent: "",
      newSceneDirection: "",
      editingSceneId: null,
      editingSceneContent: "",
      editingSceneDirection: "",
    });
    setError(null);
  }, [setError, setFields]);

  return {
    state,
    actions: {
      setFields,
      handleSave,
      handleExport,
      addScene,
      deleteScene,
      startEditingScene,
      saveEditedScene,
      cancelEditingScene,
      handleBackgroundImageUpload,
      handleAvatarUpload,
      resetToInitial,
    },
    computed: {
      avatarInitial,
      canSave: (() => {
        // Must have name and definition
        if (!state.name.trim() || !state.definition.trim() || state.saving) return false;

        // If initial state not yet loaded, don't allow save
        const initial = initialStateRef.current;
        if (!initial) return false;

        // Check for actual changes
        const hasChanges =
          state.name !== initial.name ||
          state.definition !== initial.definition ||
          state.description !== initial.description ||
          state.scenario !== initial.scenario ||
          state.nickname !== initial.nickname ||
          state.designDescription !== initial.designDescription ||
          JSON.stringify(state.designReferenceImageIds) !== initial.designReferenceImageIds ||
          state.creator !== initial.creator ||
          state.creatorNotes !== initial.creatorNotes ||
          state.creatorNotesMultilingualText !== initial.creatorNotesMultilingualText ||
          state.sourceText !== initial.sourceText ||
          state.tagsText !== initial.tagsText ||
          state.avatarPath !== initial.avatarPath ||
          JSON.stringify(state.avatarCrop ?? null) !== initial.avatarCrop ||
          JSON.stringify(state.avatarRoundPath ?? null) !== initial.avatarRoundPath ||
          state.backgroundImagePath !== initial.backgroundImagePath ||
          JSON.stringify(state.scenes) !== initial.scenes ||
          JSON.stringify(state.chatTemplates) !== initial.chatTemplates ||
          state.defaultSceneId !== initial.defaultSceneId ||
          state.defaultChatTemplateId !== initial.defaultChatTemplateId ||
          state.selectedModelId !== initial.selectedModelId ||
          state.selectedFallbackModelId !== initial.selectedFallbackModelId ||
          state.systemPromptTemplateId !== initial.systemPromptTemplateId ||
          state.disableAvatarGradient !== initial.disableAvatarGradient ||
          state.customGradientEnabled !== initial.customGradientEnabled ||
          JSON.stringify(state.customGradientColors) !== initial.customGradientColors ||
          state.memoryType !== initial.memoryType ||
          JSON.stringify(state.voiceConfig ?? null) !== initial.voiceConfig ||
          state.voiceAutoplay !== initial.voiceAutoplay;

        return hasChanges;
      })(),
    },
  };
}
