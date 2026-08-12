import { useCallback, useEffect, useMemo, useReducer, useState } from "react";

import { storageBridge } from "../../../../core/storage/files";
import { listCharacters } from "../../../../core/storage/repo";
import type { Character } from "../../../../core/storage/schemas";
import {
  groupChatCreateReducer,
  initialGroupChatCreateState,
} from "../reducers/groupChatCreateReducer";

type GroupChatCreateControllerOptions = {
  onCreated?: (sessionId: string) => void;
};

export function useGroupChatCreateController(options: GroupChatCreateControllerOptions = {}) {
  const { onCreated } = options;
  const [characters, setCharacters] = useState<Character[]>([]);
  const [ui, dispatch] = useReducer(groupChatCreateReducer, initialGroupChatCreateState);

  // Get available scenes from selected characters
  const availableScenes = useMemo(() => {
    const selectedChars = characters.filter((c) => ui.selectedIds.has(c.id));
    const scenes: Array<{
      characterId: string;
      characterName: string;
      sceneId: string;
      content: string;
    }> = [];

    selectedChars.forEach((char) => {
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
  }, [characters, ui.selectedIds]);

  useEffect(() => {
    let isActive = true;

    (async () => {
      try {
        const chars = await listCharacters();
        if (!isActive) return;
        setCharacters(chars);
      } catch (err) {
        console.error("Failed to load characters:", err);
        if (isActive) dispatch({ type: "set-error", value: "Failed to load characters" });
      } finally {
        if (isActive) dispatch({ type: "set-loading", value: false });
      }
    })();

    return () => {
      isActive = false;
    };
  }, []);

  const setGroupName = useCallback((value: string) => {
    dispatch({ type: "set-group-name", value });
  }, []);

  const toggleCharacter = useCallback((characterId: string) => {
    dispatch({ type: "toggle-character", id: characterId });
  }, []);

  const setChatType = useCallback((value: "conversation" | "roleplay") => {
    dispatch({ type: "set-chat-type", value });
  }, []);

  const setCustomScene = useCallback((value: string) => {
    dispatch({ type: "set-custom-scene", value });
  }, []);

  const setSelectedCharacterSceneId = useCallback((value: string | null) => {
    dispatch({ type: "set-selected-character-scene-id", value });
  }, []);

  const setSceneSource = useCallback((value: "none" | "custom" | "character") => {
    dispatch({ type: "set-scene-source", value });
  }, []);

  const defaultName = useMemo(() => {
    const selectedChars = characters.filter((c) => ui.selectedIds.has(c.id));
    if (selectedChars.length <= 3) {
      return selectedChars.map((c) => c.name).join(", ");
    }
    return `${selectedChars
      .slice(0, 2)
      .map((c) => c.name)
      .join(", ")} & ${selectedChars.length - 2} others`;
  }, [characters, ui.selectedIds]);

  const handleCreate = useCallback(async () => {
    if (ui.selectedIds.size < 2) {
      dispatch({
        type: "set-error",
        value: "Please select at least 2 characters for a group chat",
      });
      return;
    }

    const name = ui.groupName.trim() || defaultName;

    try {
      dispatch({ type: "set-creating", value: true });
      dispatch({ type: "set-error", value: null });

      // Build starting scene for roleplay type
      let startingScene = null;
      if (ui.chatType === "roleplay") {
        if (ui.sceneSource === "custom" && ui.customScene.trim()) {
          startingScene = {
            id: crypto.randomUUID(),
            content: ui.customScene.trim(),
            direction: "",
            createdAt: Date.now(),
          };
        } else if (ui.sceneSource === "character" && ui.selectedCharacterSceneId) {
          // Find the selected scene from available scenes
          const selectedScene = availableScenes.find(
            (s) => s.sceneId === ui.selectedCharacterSceneId,
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

      const group = await storageBridge.groupCreate(
        name,
        Array.from(ui.selectedIds),
        null,
        ui.chatType,
        startingScene,
      );
      const session = await storageBridge.groupCreateSession(group.id);
      onCreated?.(session.id);
    } catch (err) {
      console.error("Failed to create group character:", err);
      dispatch({ type: "set-error", value: "Failed to create group chat" });
    } finally {
      dispatch({ type: "set-creating", value: false });
    }
  }, [
    ui.selectedIds,
    ui.groupName,
    ui.chatType,
    ui.sceneSource,
    ui.customScene,
    ui.selectedCharacterSceneId,
    defaultName,
    onCreated,
    availableScenes,
  ]);

  return {
    characters,
    ui,
    defaultName,
    availableScenes,
    setGroupName,
    toggleCharacter,
    setChatType,
    setCustomScene,
    setSelectedCharacterSceneId,
    setSceneSource,
    handleCreate,
  } as const;
}
