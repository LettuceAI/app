import { useReducer, useEffect, useCallback, useMemo } from "react";
import { storageBridge } from "../../../../core/storage/files";
import { listCharacters } from "../../../../core/storage/repo";
import type { Character, Scene } from "../../../../core/storage/schemas";

export enum Step {
  SelectCharacters = 1,
  GroupSetup = 2,
  StartingScene = 3,
}

export interface AvailableScene {
  characterId: string;
  characterName: string;
  sceneId: string;
  content: string;
}

interface GroupChatCreateFormState {
  // Step
  step: Step;

  // Step 1: Character Selection
  characters: Character[];
  selectedIds: Set<string>;
  loadingCharacters: boolean;

  // Step 2: Group Setup
  groupName: string;
  chatType: "conversation" | "roleplay";
  speakerSelectionMethod: "llm" | "heuristic" | "round_robin";
  backgroundImagePath: string;

  // Step 3: Starting Scene
  sceneSource: "none" | "custom" | "character";
  customScene: string;
  selectedCharacterSceneId: string | null;

  // UI state
  creating: boolean;
  error: string | null;
}

type GroupChatCreateFormAction =
  | { type: "SET_STEP"; payload: Step }
  | { type: "SET_CHARACTERS"; payload: Character[] }
  | { type: "TOGGLE_CHARACTER"; payload: string }
  | { type: "SET_LOADING_CHARACTERS"; payload: boolean }
  | { type: "SET_GROUP_NAME"; payload: string }
  | { type: "SET_CHAT_TYPE"; payload: "conversation" | "roleplay" }
  | { type: "SET_SPEAKER_SELECTION_METHOD"; payload: "llm" | "heuristic" | "round_robin" }
  | { type: "SET_BACKGROUND_IMAGE_PATH"; payload: string }
  | { type: "SET_SCENE_SOURCE"; payload: "none" | "custom" | "character" }
  | { type: "SET_CUSTOM_SCENE"; payload: string }
  | { type: "SET_SELECTED_CHARACTER_SCENE_ID"; payload: string | null }
  | { type: "SET_CREATING"; payload: boolean }
  | { type: "SET_ERROR"; payload: string | null }
  | { type: "RESET_FORM" };

const initialState: GroupChatCreateFormState = {
  step: Step.SelectCharacters,
  characters: [],
  selectedIds: new Set<string>(),
  loadingCharacters: true,
  groupName: "",
  chatType: "conversation",
  speakerSelectionMethod: "llm" as const,
  backgroundImagePath: "",
  sceneSource: "none",
  customScene: "",
  selectedCharacterSceneId: null,
  creating: false,
  error: null,
};

function groupChatCreateFormReducer(
  state: GroupChatCreateFormState,
  action: GroupChatCreateFormAction,
): GroupChatCreateFormState {
  switch (action.type) {
    case "SET_STEP":
      return { ...state, step: action.payload, error: null };
    case "SET_CHARACTERS":
      return { ...state, characters: action.payload };
    case "TOGGLE_CHARACTER": {
      const nextSelected = new Set(state.selectedIds);
      if (nextSelected.has(action.payload)) {
        nextSelected.delete(action.payload);
      } else {
        nextSelected.add(action.payload);
      }
      return { ...state, selectedIds: nextSelected, error: null };
    }
    case "SET_LOADING_CHARACTERS":
      return { ...state, loadingCharacters: action.payload };
    case "SET_GROUP_NAME":
      return { ...state, groupName: action.payload, error: null };
    case "SET_CHAT_TYPE":
      return { ...state, chatType: action.payload, error: null };
    case "SET_SPEAKER_SELECTION_METHOD":
      return { ...state, speakerSelectionMethod: action.payload, error: null };
    case "SET_BACKGROUND_IMAGE_PATH":
      return { ...state, backgroundImagePath: action.payload, error: null };
    case "SET_SCENE_SOURCE":
      return { ...state, sceneSource: action.payload, error: null };
    case "SET_CUSTOM_SCENE":
      return { ...state, customScene: action.payload, error: null };
    case "SET_SELECTED_CHARACTER_SCENE_ID":
      return { ...state, selectedCharacterSceneId: action.payload, error: null };
    case "SET_CREATING":
      return { ...state, creating: action.payload };
    case "SET_ERROR":
      return { ...state, error: action.payload };
    case "RESET_FORM":
      return { ...initialState, loadingCharacters: false };
    default:
      return state;
  }
}

type UseGroupChatCreateFormOptions = {
  onCreated?: (sessionId: string) => void;
};

export function useGroupChatCreateForm(options: UseGroupChatCreateFormOptions = {}) {
  const { onCreated } = options;
  const [state, dispatch] = useReducer(groupChatCreateFormReducer, initialState);

  // Load characters on mount
  useEffect(() => {
    let isActive = true;

    (async () => {
      try {
        const chars = await listCharacters();
        if (!isActive) return;
        dispatch({ type: "SET_CHARACTERS", payload: chars });
      } catch (err) {
        console.error("Failed to load characters:", err);
        if (isActive) dispatch({ type: "SET_ERROR", payload: "Failed to load characters" });
      } finally {
        if (isActive) dispatch({ type: "SET_LOADING_CHARACTERS", payload: false });
      }
    })();

    return () => {
      isActive = false;
    };
  }, []);

  // Computed values
  const selectedCharacters = useMemo(
    () => state.characters.filter((c) => state.selectedIds.has(c.id)),
    [state.characters, state.selectedIds],
  );

  const availableScenes = useMemo((): AvailableScene[] => {
    const scenes: AvailableScene[] = [];

    selectedCharacters.forEach((char) => {
      if (char.scenes && char.scenes.length > 0) {
        char.scenes.forEach((scene) => {
          scenes.push({
            characterId: char.id,
            characterName: char.name,
            sceneId: scene.id,
            content: scene.content,
          });
        });
      }
    });

    return scenes;
  }, [selectedCharacters]);

  const defaultName = useMemo(() => {
    if (selectedCharacters.length <= 3) {
      return selectedCharacters.map((c) => c.name).join(", ");
    }
    return `${selectedCharacters
      .slice(0, 2)
      .map((c) => c.name)
      .join(", ")} & ${selectedCharacters.length - 2} others`;
  }, [selectedCharacters]);

  // Actions
  const setStep = useCallback((step: Step) => {
    dispatch({ type: "SET_STEP", payload: step });
  }, []);

  const toggleCharacter = useCallback((characterId: string) => {
    dispatch({ type: "TOGGLE_CHARACTER", payload: characterId });
  }, []);

  const setGroupName = useCallback((value: string) => {
    dispatch({ type: "SET_GROUP_NAME", payload: value });
  }, []);

  const setChatType = useCallback((value: "conversation" | "roleplay") => {
    dispatch({ type: "SET_CHAT_TYPE", payload: value });
  }, []);

  const setSpeakerSelectionMethod = useCallback(
    (value: "llm" | "heuristic" | "round_robin") => {
      dispatch({ type: "SET_SPEAKER_SELECTION_METHOD", payload: value });
    },
    [],
  );

  const setBackgroundImagePath = useCallback((value: string) => {
    dispatch({ type: "SET_BACKGROUND_IMAGE_PATH", payload: value });
  }, []);

  const setSceneSource = useCallback((value: "none" | "custom" | "character") => {
    dispatch({ type: "SET_SCENE_SOURCE", payload: value });
  }, []);

  const setCustomScene = useCallback((value: string) => {
    dispatch({ type: "SET_CUSTOM_SCENE", payload: value });
  }, []);

  const setSelectedCharacterSceneId = useCallback((value: string | null) => {
    dispatch({ type: "SET_SELECTED_CHARACTER_SCENE_ID", payload: value });
  }, []);

  const copySceneToCustom = useCallback((sceneContent: string) => {
    dispatch({ type: "SET_SCENE_SOURCE", payload: "custom" });
    dispatch({ type: "SET_CUSTOM_SCENE", payload: sceneContent });
    dispatch({ type: "SET_SELECTED_CHARACTER_SCENE_ID", payload: null });
  }, []);

  const handleCreate = useCallback(async () => {
    if (state.selectedIds.size < 2) {
      dispatch({
        type: "SET_ERROR",
        payload: "Please select at least 2 characters for a group chat",
      });
      return;
    }

    const name = state.groupName.trim() || defaultName;

    try {
      dispatch({ type: "SET_CREATING", payload: true });
      dispatch({ type: "SET_ERROR", payload: null });

      // Build starting scene for roleplay type
      let startingScene: Scene | null = null;
      if (state.chatType === "roleplay") {
        if (state.sceneSource === "custom" && state.customScene.trim()) {
          startingScene = {
            id: crypto.randomUUID(),
            content: state.customScene.trim(),
            direction: "",
            createdAt: Date.now(),
          };
        } else if (state.sceneSource === "character" && state.selectedCharacterSceneId) {
          const selectedScene = availableScenes.find(
            (s) => s.sceneId === state.selectedCharacterSceneId,
          );
          if (selectedScene) {
            startingScene = {
              id: crypto.randomUUID(),
              content: selectedScene.content,
              direction: "",
              createdAt: Date.now(),
            };
          }
        }
      }

      const session = await storageBridge.groupSessionCreate(
        name,
        Array.from(state.selectedIds),
        null,
        state.chatType,
        startingScene,
        state.backgroundImagePath || null,
        state.speakerSelectionMethod,
      );
      onCreated?.(session.id);
    } catch (err) {
      console.error("Failed to create group session:", err);
      dispatch({ type: "SET_ERROR", payload: "Failed to create group chat" });
    } finally {
      dispatch({ type: "SET_CREATING", payload: false });
    }
  }, [
    state.selectedIds,
    state.groupName,
    state.chatType,
    state.speakerSelectionMethod,
    state.backgroundImagePath,
    state.sceneSource,
    state.customScene,
    state.selectedCharacterSceneId,
    defaultName,
    onCreated,
    availableScenes,
  ]);

  // Computed validation states
  const canContinueFromCharacters = state.selectedIds.size >= 2;
  const canContinueFromSetup = true; // Always can continue
  const canCreate =
    state.chatType === "conversation" ||
    state.sceneSource === "none" ||
    (state.sceneSource === "custom" && state.customScene.trim().length > 0) ||
    (state.sceneSource === "character" && state.selectedCharacterSceneId !== null);

  return {
    state,
    actions: {
      setStep,
      toggleCharacter,
      setGroupName,
      setChatType,
      setSpeakerSelectionMethod,
      setBackgroundImagePath,
      setSceneSource,
      setCustomScene,
      setSelectedCharacterSceneId,
      copySceneToCustom,
      handleCreate,
    },
    computed: {
      selectedCharacters,
      availableScenes,
      defaultName,
      canContinueFromCharacters,
      canContinueFromSetup,
      canCreate,
    },
  } as const;
}
