import { useCallback, useEffect, useMemo, useReducer, useState } from "react";

import { storageBridge } from "../../../../core/storage/files";
import {
  listCharacters,
  listPersonas,
  updateGroupDisableCharacterLorebooks,
} from "../../../../core/storage/repo";
import type { Character, Group, Persona } from "../../../../core/storage/schemas";
import {
  groupChatSettingsUiReducer,
  initialGroupChatSettingsUiState,
} from "../reducers/groupChatSettingsReducer";
import { useI18n } from "../../../../core/i18n/context";

export function useGroupSettingsController(groupId?: string) {
  const { t } = useI18n();
  const [group, setGroup] = useState<Group | null>(null);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [ui, dispatch] = useReducer(groupChatSettingsUiReducer, initialGroupChatSettingsUiState);

  const setUi = useCallback((patch: Partial<typeof ui>) => {
    dispatch({ type: "PATCH", patch });
  }, []);

  const loadData = useCallback(async () => {
    if (!groupId) return;

    try {
      setUi({ loading: true, error: null });
      const [groupData, chars, personaList] = await Promise.all([
        storageBridge.groupGet(groupId),
        listCharacters(),
        listPersonas(),
      ]);

      if (!groupData) {
        setUi({ error: t("groupChats.groupSettingsController.notFound") });
        return;
      }

      setGroup(groupData);
      setCharacters(chars);
      setPersonas(personaList);
      setUi({ nameDraft: groupData.name });
    } catch (err) {
      console.error("Failed to load group settings:", err);
      setUi({ error: t("groupChats.groupSettingsController.failedToLoad") });
    } finally {
      setUi({ loading: false });
    }
  }, [groupId, setUi, t]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const groupCharacters = useMemo(() => {
    if (!group) return [];
    return group.characterIds
      .map((id) => characters.find((c) => c.id === id))
      .filter(Boolean) as Character[];
  }, [group, characters]);

  const availableCharacters = useMemo(() => {
    if (!group) return [];
    return characters.filter((c) => !group.characterIds.includes(c.id));
  }, [group, characters]);

  const currentPersona = useMemo(() => {
    if (!group?.personaId) return null;
    return personas.find((p) => p.id === group.personaId) || null;
  }, [group, personas]);

  const currentPersonaDisplay = useMemo(() => {
    if (!group?.personaId) return t("groupChats.sessionSettingsController.noPersona");
    if (!currentPersona) return t("groupChats.sessionSettingsController.customPersona");
    return currentPersona.isDefault
      ? `${currentPersona.title} (default)`
      : currentPersona.title;
  }, [group?.personaId, currentPersona, t]);

  const mutedCharacterIds = useMemo(
    () => new Set(group?.mutedCharacterIds ?? []),
    [group?.mutedCharacterIds],
  );

  const handleSaveName = useCallback(async () => {
    if (!group || !ui.nameDraft.trim()) return;
    const prev = group;
    try {
      setUi({ saving: true });
      const trimmed = ui.nameDraft.trim();
      setGroup({ ...group, name: trimmed });
      await storageBridge.groupUpdateName(group.id, trimmed);
      setUi({ editingName: false });
    } catch (err) {
      setGroup(prev);
      console.error("Failed to save group name:", err);
    } finally {
      setUi({ saving: false });
    }
  }, [group, ui.nameDraft, setUi]);

  const handleChangePersona = useCallback(
    (personaId: string | null) => {
      if (!group) return;
      const prev = group;
      setUi({ saving: true, showPersonaSelector: false });
      setGroup({ ...group, personaId });
      void storageBridge
        .groupUpdatePersona(group.id, personaId)
        .catch((err) => {
          setGroup(prev);
          console.error("Failed to change persona:", err);
        })
        .finally(() => {
          setUi({ saving: false });
        });
    },
    [group, setUi],
  );

  const handleAddCharacter = useCallback(
    (characterId: string) => {
      if (!group) return;
      if (group.characterIds.includes(characterId)) {
        setUi({ showAddCharacter: false });
        return;
      }
      const prev = group;
      const nextIds = [...group.characterIds, characterId];
      setUi({ saving: true, showAddCharacter: false });
      setGroup({ ...group, characterIds: nextIds });
      void storageBridge
        .groupUpdateCharacterIds(group.id, nextIds)
        .catch((err) => {
          setGroup(prev);
          console.error("Failed to add character:", err);
        })
        .finally(() => {
          setUi({ saving: false });
        });
    },
    [group, setUi],
  );

  const handleRemoveCharacter = useCallback(
    (characterId: string) => {
      if (!group) return;
      if (group.characterIds.length <= 2) {
        setUi({ showRemoveConfirm: null });
        return;
      }
      const prev = group;
      const nextCharacterIds = group.characterIds.filter((id) => id !== characterId);
      // Rust auto-prunes muted list, but update optimistic state too
      const nextMutedIds = group.mutedCharacterIds.filter((id) => id !== characterId);

      setUi({ saving: true, showRemoveConfirm: null });
      setGroup({ ...group, characterIds: nextCharacterIds, mutedCharacterIds: nextMutedIds });
      void storageBridge
        .groupUpdateCharacterIds(group.id, nextCharacterIds)
        .catch((err) => {
          setGroup(prev);
          console.error("Failed to remove character:", err);
        })
        .finally(() => {
          setUi({ saving: false });
        });
    },
    [group, setUi],
  );

  const handleSetCharacterMuted = useCallback(
    (characterId: string, muted: boolean) => {
      if (!group) return;

      const nextMuted = new Set(group.mutedCharacterIds);
      const activeCount = group.characterIds.length - nextMuted.size;
      if (muted && activeCount <= 1 && !nextMuted.has(characterId)) {
        setUi({ error: t("groupChats.sessionSettingsController.minOneActive") });
        return;
      }

      if (muted) nextMuted.add(characterId);
      else nextMuted.delete(characterId);

      const prev = group;
      const nextMutedArr = Array.from(nextMuted);
      setUi({ saving: true });
      setGroup({ ...group, mutedCharacterIds: nextMutedArr });
      void storageBridge
        .groupUpdateMutedCharacterIds(group.id, nextMutedArr)
        .catch((err) => {
          setGroup(prev);
          console.error("Failed to update muted defaults:", err);
        })
        .finally(() => {
          setUi({ saving: false });
        });
    },
    [group, setUi, t],
  );

  const handleChangeSpeakerSelectionMethod = useCallback(
    (method: Group["speakerSelectionMethod"]) => {
      if (!group) return;
      const prev = group;
      setUi({ saving: true });
      setGroup({ ...group, speakerSelectionMethod: method });
      void storageBridge
        .groupUpdateSpeakerSelectionMethod(group.id, method)
        .catch((err) => {
          setGroup(prev);
          console.error("Failed to update speaker selection method:", err);
        })
        .finally(() => {
          setUi({ saving: false });
        });
    },
    [group, setUi],
  );

  const handleChangeMemoryType = useCallback(
    (memoryType: Group["memoryType"]) => {
      if (!group) return;
      const prev = group;
      setUi({ saving: true });
      setGroup({ ...group, memoryType });
      void storageBridge
        .groupUpdateMemoryType(group.id, memoryType)
        .catch((err) => {
          setGroup(prev);
          console.error("Failed to update memory type:", err);
        })
        .finally(() => {
          setUi({ saving: false });
        });
    },
    [group, setUi],
  );

  const handleUpdateBackgroundImage = useCallback(
    async (nextPath: string | null) => {
      if (!group) return;
      const prev = group;
      try {
        setUi({ saving: true });
        setGroup({ ...group, backgroundImagePath: nextPath });
        await storageBridge.groupUpdateBackgroundImage(group.id, nextPath);
      } catch (err) {
        setGroup(prev);
        throw err;
      } finally {
        setUi({ saving: false });
      }
    },
    [group, setUi],
  );

  const handleSetDisableCharacterLorebooks = useCallback(
    async (disableCharacterLorebooks: boolean) => {
      if (!group) return;
      const prev = group;
      try {
        setUi({ saving: true });
        setGroup({ ...group, disableCharacterLorebooks });
        const updated = await updateGroupDisableCharacterLorebooks(group.id, disableCharacterLorebooks);
        setGroup(updated);
      } catch (err) {
        setGroup(prev);
        console.error("Failed to update group lorebook behavior:", err);
      } finally {
        setUi({ saving: false });
      }
    },
    [group, setUi],
  );

  const setEditingName = useCallback((value: boolean) => setUi({ editingName: value }), [setUi]);
  const setNameDraft = useCallback((value: string) => setUi({ nameDraft: value }), [setUi]);
  const setShowPersonaSelector = useCallback(
    (value: boolean) => setUi({ showPersonaSelector: value }),
    [setUi],
  );
  const setShowAddCharacter = useCallback(
    (value: boolean) => setUi({ showAddCharacter: value }),
    [setUi],
  );
  const setShowRemoveConfirm = useCallback(
    (value: string | null) => setUi({ showRemoveConfirm: value }),
    [setUi],
  );

  return {
    group,
    personas,
    groupCharacters,
    availableCharacters,
    currentPersona,
    currentPersonaDisplay,
    mutedCharacterIds,
    ui,
    setEditingName,
    setNameDraft,
    setShowPersonaSelector,
    setShowAddCharacter,
    setShowRemoveConfirm,
    handleSaveName,
    handleChangePersona,
    handleAddCharacter,
    handleRemoveCharacter,
    handleSetCharacterMuted,
    handleChangeSpeakerSelectionMethod,
    handleChangeMemoryType,
    handleUpdateBackgroundImage,
    handleSetDisableCharacterLorebooks,
  } as const;
}
