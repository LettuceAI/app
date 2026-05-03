import { useCallback, useEffect, useMemo, useReducer, useState } from "react";

import { storageBridge } from "../../../../core/storage/files";
import { updateGroupSessionDisableCharacterLorebooks } from "../../../../core/storage/repo";
import type {
  GroupSession,
  GroupParticipation,
  Character,
  Persona,
} from "../../../../core/storage/schemas";
import {
  groupChatSettingsUiReducer,
  initialGroupChatSettingsUiState,
} from "../reducers/groupChatSettingsReducer";
import { useI18n } from "../../../../core/i18n/context";

interface SettingsControllerOptions {
  layoutSession?: GroupSession | null;
  layoutCharacters?: Character[];
  layoutPersonas?: Persona[];
  updateSession?: (session: GroupSession | null) => void;
}

export function useGroupChatSettingsController(
  groupSessionId?: string,
  options: SettingsControllerOptions = {},
) {
  const {
    layoutSession,
    layoutCharacters = [],
    layoutPersonas = [],
    updateSession,
  } = options;
  const { t } = useI18n();

  const session = layoutSession ?? null;
  const characters = layoutCharacters;
  const personas = layoutPersonas;
  const [participationStats, setParticipationStats] = useState<GroupParticipation[]>([]);
  const [messageCount, setMessageCount] = useState<number>(0);
  const [ui, dispatch] = useReducer(groupChatSettingsUiReducer, initialGroupChatSettingsUiState);

  const setUi = useCallback((patch: Partial<typeof ui>) => {
    dispatch({ type: "PATCH", patch });
  }, []);

  // Only fetch stats + message count (session, characters, personas come from layout)
  const loadData = useCallback(async () => {
    if (!groupSessionId || !layoutSession) return;

    try {
      setUi({ loading: true, error: null });

      const [stats, msgCount] = await Promise.all([
        storageBridge.groupParticipationStats(groupSessionId),
        storageBridge.groupMessageCount(groupSessionId),
      ]);

      setParticipationStats(stats);
      setMessageCount(msgCount);
      setUi({ nameDraft: layoutSession.name });
    } catch (err) {
      console.error("Failed to load group chat settings:", err);
      setUi({ error: t("groupChats.sessionSettingsController.failedToLoad") });
    } finally {
      setUi({ loading: false });
    }
  }, [groupSessionId, layoutSession, setUi, t]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  const groupCharacters = useMemo(() => {
    if (!session) return [];
    return session.characterIds
      .map((id) => characters.find((c) => c.id === id))
      .filter(Boolean) as Character[];
  }, [session, characters]);

  const availableCharacters = useMemo(() => {
    if (!session) return [];
    return characters.filter((c) => !session.characterIds.includes(c.id));
  }, [session, characters]);

  const mutedCharacterIds = useMemo(
    () => new Set(session?.mutedCharacterIds ?? []),
    [session?.mutedCharacterIds],
  );

  const currentPersona = useMemo(() => {
    if (!session?.personaId) return null;
    return personas.find((p) => p.id === session.personaId) || null;
  }, [session, personas]);

  const currentPersonaDisplay = useMemo(() => {
    if (!session?.personaId) return t("groupChats.sessionSettingsController.noPersona");
    if (!currentPersona) return t("groupChats.sessionSettingsController.customPersona");
    return currentPersona.isDefault ? `${currentPersona.title} (default)` : currentPersona.title;
  }, [currentPersona, session?.personaId, t]);

  const handleSaveName = useCallback(async () => {
    if (!session || !ui.nameDraft.trim()) return;

    try {
      setUi({ saving: true });
      const updated = await storageBridge.groupSessionUpdate(
        session.id,
        ui.nameDraft.trim(),
        session.characterIds,
        session.personaId,
      );
      updateSession?.(updated);
      setUi({ editingName: false });
    } catch (err) {
      console.error("Failed to save name:", err);
    } finally {
      setUi({ saving: false });
    }
  }, [session, ui.nameDraft, setUi, updateSession]);

  const handleChangePersona = useCallback(
    async (personaId: string | null) => {
      if (!session) return;

      try {
        setUi({ saving: true });
        const updated = await storageBridge.groupSessionUpdate(
          session.id,
          session.name,
          session.characterIds,
          personaId,
        );
        updateSession?.(updated);
        setUi({ showPersonaSelector: false });
      } catch (err) {
        console.error("Failed to change persona:", err);
      } finally {
        setUi({ saving: false });
      }
    },
    [session, setUi, updateSession],
  );

  const handleAddCharacter = useCallback(
    async (characterId: string) => {
      if (!session) return;

      try {
        setUi({ saving: true });
        const updated = await storageBridge.groupSessionAddCharacter(session.id, characterId);
        updateSession?.(updated);
        setUi({ showAddCharacter: false });
      } catch (err) {
        console.error("Failed to add character:", err);
      } finally {
        setUi({ saving: false });
      }
    },
    [session, setUi, updateSession],
  );

  const handleRemoveCharacter = useCallback(
    async (characterId: string) => {
      if (!session) return;

      if (session.characterIds.length <= 2) {
        setUi({ showRemoveConfirm: null });
        return;
      }

      try {
        setUi({ saving: true });
        const updated = await storageBridge.groupSessionRemoveCharacter(session.id, characterId);
        updateSession?.(updated);
        setUi({ showRemoveConfirm: null });
      } catch (err) {
        console.error("Failed to remove character:", err);
      } finally {
        setUi({ saving: false });
      }
    },
    [session, setUi, updateSession],
  );

  const handleChangeSpeakerSelectionMethod = useCallback(
    async (method: "llm" | "heuristic" | "round_robin") => {
      if (!session) return;
      try {
        setUi({ saving: true });
        const updated = await storageBridge.groupSessionUpdateSpeakerSelectionMethod(
          session.id,
          method,
        );
        updateSession?.(updated);
      } catch (err) {
        console.error("Failed to update speaker selection method:", err);
      } finally {
        setUi({ saving: false });
      }
    },
    [session, setUi, updateSession],
  );

  const handleSetCharacterMuted = useCallback(
    async (characterId: string, muted: boolean) => {
      if (!session) return;
      const nextMuted = new Set(session.mutedCharacterIds ?? []);
      const activeCount = session.characterIds.length - nextMuted.size;
      if (muted && activeCount <= 1 && !nextMuted.has(characterId)) {
        setUi({ error: t("groupChats.sessionSettingsController.minOneActive") });
        return;
      }
      if (muted) {
        nextMuted.add(characterId);
      } else {
        nextMuted.delete(characterId);
      }

      try {
        setUi({ saving: true });
        const updated = await storageBridge.groupSessionUpdateMutedCharacterIds(
          session.id,
          Array.from(nextMuted),
        );
        updateSession?.(updated);
      } catch (err) {
        console.error("Failed to update muted characters:", err);
      } finally {
        setUi({ saving: false });
      }
    },
    [session, setUi, updateSession, t],
  );

  const handleUpdateBackgroundImage = useCallback(
    async (backgroundImagePath: string | null) => {
      if (!session) return;

      try {
        setUi({ saving: true });
        const updated = await storageBridge.groupSessionUpdateBackgroundImage(
          session.id,
          backgroundImagePath,
        );
        updateSession?.(updated);
      } catch (err) {
        console.error("Failed to update background image:", err);
        throw err;
      } finally {
        setUi({ saving: false });
      }
    },
    [session, setUi, updateSession],
  );

  const handleSetDisableCharacterLorebooks = useCallback(
    async (disableCharacterLorebooks: boolean) => {
      if (!session) return;
      try {
        setUi({ saving: true });
        const updated = await updateGroupSessionDisableCharacterLorebooks(
          session.id,
          disableCharacterLorebooks,
        );
        updateSession?.(updated);
      } catch (err) {
        console.error("Failed to update session lorebook behavior:", err);
      } finally {
        setUi({ saving: false });
      }
    },
    [session, setUi, updateSession],
  );

  const getParticipationPercent = useCallback(
    (characterId: string) => {
      if (!participationStats.length) return 0;
      const total = participationStats.reduce((sum, stat) => sum + stat.speakCount, 0);
      const stat = participationStats.find((s) => s.characterId === characterId);
      if (!stat || total === 0) return 0;
      return Math.round((stat.speakCount / total) * 100);
    },
    [participationStats],
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
    session,
    characters,
    personas,
    participationStats,
    messageCount,
    groupCharacters,
    availableCharacters,
    currentPersona,
    currentPersonaDisplay,
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
    handleChangeSpeakerSelectionMethod,
    handleSetCharacterMuted,
    handleUpdateBackgroundImage,
    handleSetDisableCharacterLorebooks,
    mutedCharacterIds,
    getParticipationPercent,
  } as const;
}
