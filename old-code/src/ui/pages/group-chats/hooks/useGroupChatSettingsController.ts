import { useCallback, useEffect, useMemo, useReducer, useRef, useState } from "react";

import { storageBridge } from "../../../../core/storage/files";
import { updateGroupSessionDisableCharacterLorebooks } from "../../../../core/storage/repo";
import type {
  GroupSession,
  GroupSessionOverrideKey,
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
  const uiRef = useRef(ui);
  uiRef.current = ui;

  const setUi = useCallback((patch: Partial<typeof ui>) => {
    dispatch({ type: "PATCH", patch });
  }, []);

  // Only fetch stats + message count (session, characters, personas come from layout)
  const layoutSessionId = layoutSession?.id ?? null;
  const layoutSessionName = layoutSession?.name ?? null;

  const loadData = useCallback(async () => {
    if (!groupSessionId || !layoutSessionId) return;

    try {
      setUi({ loading: true, error: null });

      const [stats, msgCount] = await Promise.all([
        storageBridge.groupParticipationStats(groupSessionId),
        storageBridge.groupMessageCount(groupSessionId),
      ]);

      setParticipationStats(stats);
      setMessageCount(msgCount);
    } catch (err) {
      console.error("Failed to load group chat settings:", err);
      setUi({ error: t("groupChats.sessionSettingsController.failedToLoad") });
    } finally {
      setUi({ loading: false });
    }
  }, [groupSessionId, layoutSessionId, setUi, t]);

  useEffect(() => {
    void loadData();
  }, [loadData]);

  useEffect(() => {
    if (layoutSessionName === null || uiRef.current.editingName) return;
    setUi({ nameDraft: layoutSessionName });
  }, [layoutSessionName, setUi]);

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

  const applyOptimisticSessionUpdate = useCallback(
    async (
      patch: Partial<GroupSession>,
      request: (current: GroupSession) => Promise<GroupSession | null>,
    ) => {
      if (!session) return;
      const previous = session;
      updateSession?.({ ...previous, ...patch });
      try {
        setUi({ saving: true });
        const updated = await request(previous);
        if (updated) updateSession?.(updated);
      } catch (err) {
        updateSession?.(previous);
        throw err;
      } finally {
        setUi({ saving: false });
      }
    },
    [session, setUi, updateSession],
  );

  const handleSaveName = useCallback(async () => {
    if (!session || !ui.nameDraft.trim()) return;

    try {
      setUi({ saving: true });
      const trimmed = ui.nameDraft.trim();
      await storageBridge.groupSessionUpdateTitle(session.id, trimmed);
      updateSession?.({ ...session, name: trimmed });
      setUi({ editingName: false });
    } catch (err) {
      console.error("Failed to save name:", err);
    } finally {
      setUi({ saving: false });
    }
  }, [session, ui.nameDraft, setUi, updateSession]);

  const handleChangePersona = useCallback(
    async (personaId: string | null) => {
      try {
        await applyOptimisticSessionUpdate({ personaId }, (current) =>
          storageBridge.groupSessionUpdatePersona(current.id, personaId),
        );
        setUi({ showPersonaSelector: false });
      } catch (err) {
        console.error("Failed to change persona:", err);
      }
    },
    [applyOptimisticSessionUpdate, setUi],
  );

  const handleClearOverride = useCallback(
    async (key: GroupSessionOverrideKey) => {
      if (!session) return;

      try {
        setUi({ saving: true });
        const updated = await storageBridge.groupSessionClearOverride(session.id, key);
        updateSession?.(updated);
      } catch (err) {
        console.error("Failed to reset to group default:", err);
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
    async (method: "llm" | "heuristic" | "round_robin" | "director" | "director_action") => {
      try {
        await applyOptimisticSessionUpdate({ speakerSelectionMethod: method }, (current) =>
          storageBridge.groupSessionUpdateSpeakerSelectionMethod(current.id, method),
        );
      } catch (err) {
        console.error("Failed to update speaker selection method:", err);
      }
    },
    [applyOptimisticSessionUpdate],
  );

  const handleChangeCharacterModel = useCallback(
    async (characterId: string, modelId: string | null) => {
      if (!session) return;
      const nextOverrides = { ...(session.characterModelOverrides ?? {}) };
      if (modelId) {
        nextOverrides[characterId] = modelId;
      } else {
        delete nextOverrides[characterId];
      }
      try {
        await applyOptimisticSessionUpdate(
          { characterModelOverrides: nextOverrides },
          (current) =>
            storageBridge.groupSessionUpdateCharacterModelOverride(
              current.id,
              characterId,
              modelId,
            ),
        );
      } catch (err) {
        console.error("Failed to update character model override:", err);
      }
    },
    [applyOptimisticSessionUpdate, session],
  );

  const handleChangePromptTemplate = useCallback(
    async (promptTemplateId: string | null) => {
      if (!session) return;
      const patch: Partial<GroupSession> =
        session.chatType === "roleplay"
          ? { groupChatRoleplayPromptTemplateId: promptTemplateId }
          : { groupChatPromptTemplateId: promptTemplateId };
      try {
        await applyOptimisticSessionUpdate(patch, (current) =>
          storageBridge.groupSessionUpdatePromptTemplate(
            current.id,
            current.chatType,
            promptTemplateId,
          ),
        );
      } catch (err) {
        console.error("Failed to update group prompt template:", err);
      }
    },
    [applyOptimisticSessionUpdate, session],
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

      const mutedCharacterIds = Array.from(nextMuted);
      try {
        await applyOptimisticSessionUpdate({ mutedCharacterIds }, (current) =>
          storageBridge.groupSessionUpdateMutedCharacterIds(current.id, mutedCharacterIds),
        );
      } catch (err) {
        console.error("Failed to update muted characters:", err);
      }
    },
    [applyOptimisticSessionUpdate, session, setUi, t],
  );

  const handleUpdateBackgroundImage = useCallback(
    async (backgroundImagePath: string | null) => {
      try {
        await applyOptimisticSessionUpdate({ backgroundImagePath }, (current) =>
          storageBridge.groupSessionUpdateBackgroundImage(current.id, backgroundImagePath),
        );
      } catch (err) {
        console.error("Failed to update background image:", err);
        throw err;
      }
    },
    [applyOptimisticSessionUpdate],
  );

  const handleSetDisableCharacterLorebooks = useCallback(
    async (disableCharacterLorebooks: boolean) => {
      try {
        await applyOptimisticSessionUpdate({ disableCharacterLorebooks }, (current) =>
          updateGroupSessionDisableCharacterLorebooks(current.id, disableCharacterLorebooks),
        );
      } catch (err) {
        console.error("Failed to update session lorebook behavior:", err);
      }
    },
    [applyOptimisticSessionUpdate],
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
    handleClearOverride,
    handleAddCharacter,
    handleRemoveCharacter,
    handleChangeSpeakerSelectionMethod,
    handleChangeCharacterModel,
    handleChangePromptTemplate,
    handleSetCharacterMuted,
    handleUpdateBackgroundImage,
    handleSetDisableCharacterLorebooks,
    mutedCharacterIds,
    getParticipationPercent,
  } as const;
}
