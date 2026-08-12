import { useCallback, useEffect, useMemo, useState } from "react";
import { Outlet, useOutletContext, useParams } from "react-router-dom";
import { listen } from "@tauri-apps/api/event";
import type {
  GroupSession,
  Group,
  Character,
  Persona,
  Settings,
  ChatAppearanceSettings,
  ChatAppearanceOverride,
} from "../../../core/storage/schemas";
import {
  createDefaultChatAppearanceSettings,
  mergeChatAppearance,
} from "../../../core/storage/schemas";
import { storageBridge } from "../../../core/storage/files";
import { getGroup, listCharacters, listPersonas, readSettings } from "../../../core/storage/repo";
import { SESSION_UPDATED_EVENT, SETTINGS_UPDATED_EVENT } from "../../../core/storage/repo";
import { useImageData } from "../../hooks/useImageData";
import {
  analyzeImageBrightness,
  computeChatTheme,
  getDefaultThemeSync,
  type ThemeColors,
} from "../../../core/utils/imageAnalysis";

export type AppearanceFieldUpdater = <K extends keyof ChatAppearanceSettings>(
  key: K,
  value: ChatAppearanceSettings[K],
) => void;

export interface GroupChatLayoutContext {
  session: GroupSession | null;
  sessionLoading: boolean;
  characters: Character[];
  personas: Persona[];
  settings: Settings | null;
  group: Group | null;
  backgroundImageData: string | undefined;
  isBackgroundLight: boolean;
  theme: ThemeColors;
  chatAppearance: ChatAppearanceSettings;
  reloadSession: () => void;
  updateSession: (session: GroupSession | null) => void;
  updateGroup: (group: Group | null) => void;
  draftAppearanceOverride: ChatAppearanceOverride | null;
  setDraftAppearanceOverride: (next: ChatAppearanceOverride | null) => void;
  appearanceFieldUpdater: AppearanceFieldUpdater | null;
  registerAppearanceFieldUpdater: (fn: AppearanceFieldUpdater | null) => void;
}

export function useGroupChatLayoutContext() {
  return useOutletContext<GroupChatLayoutContext>();
}

export function GroupChatLayout() {
  const { groupSessionId } = useParams<{ groupSessionId: string }>();
  const [session, setSession] = useState<GroupSession | null>(null);
  const [group, setGroup] = useState<Group | null>(null);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadCount, setLoadCount] = useState(0);

  const [bgBrightness, setBgBrightness] = useState<number | null>(null);
  const [baseChatAppearance, setBaseChatAppearance] = useState<ChatAppearanceSettings>(
    createDefaultChatAppearanceSettings(),
  );
  const [draftAppearanceOverride, setDraftAppearanceOverride] =
    useState<ChatAppearanceOverride | null>(null);
  const [appearanceFieldUpdater, setAppearanceFieldUpdater] =
    useState<AppearanceFieldUpdater | null>(null);
  const registerAppearanceFieldUpdater = useCallback(
    (fn: AppearanceFieldUpdater | null) => setAppearanceFieldUpdater(() => fn),
    [],
  );
  const chatAppearance = useMemo(
    () =>
      draftAppearanceOverride
        ? mergeChatAppearance(baseChatAppearance, draftAppearanceOverride)
        : baseChatAppearance,
    [baseChatAppearance, draftAppearanceOverride],
  );
  const [theme, setTheme] = useState<ThemeColors>(getDefaultThemeSync());

  useEffect(() => {
    let cancelled = false;
    (async () => {
      if (!groupSessionId) {
        setLoading(false);
        setSession(null);
        return;
      }
      try {
        setLoading(true);
        const [sessionData, chars, personaList, settingsData] = await Promise.all([
          storageBridge.groupSessionGet(groupSessionId),
          listCharacters(),
          listPersonas(),
          readSettings(),
        ]);
        const groupData = sessionData?.groupCharacterId
          ? await getGroup(sessionData.groupCharacterId).catch((err) => {
              console.error("GroupChatLayout: failed to load group", err);
              return null;
            })
          : null;
        if (!cancelled) {
          setSession(sessionData);
          setGroup(groupData);
          setCharacters(chars);
          setPersonas(personaList);
          setSettings(settingsData);
          const globalAppearance =
            settingsData.advancedSettings?.chatAppearance ?? createDefaultChatAppearanceSettings();
          setBaseChatAppearance(
            mergeChatAppearance(globalAppearance, groupData?.chatAppearance ?? undefined),
          );
        }
      } catch (err) {
        console.error("GroupChatLayout: failed to load data", err);
        if (!cancelled) setSession(null);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [groupSessionId, loadCount]);

  useEffect(() => {
    const onSettingsUpdated = () => {
      setLoadCount((c) => c + 1);
    };
    window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    return () => window.removeEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
  }, []);

  useEffect(() => {
    const onSessionUpdated = () => {
      setLoadCount((c) => c + 1);
    };
    window.addEventListener(SESSION_UPDATED_EVENT, onSessionUpdated);
    return () => window.removeEventListener(SESSION_UPDATED_EVENT, onSessionUpdated);
  }, []);

  useEffect(() => {
    if (!groupSessionId) return;
    let unlisteners: Array<() => void> = [];

    const setup = async () => {
      try {
        const processing = await listen("group-dynamic-memory:processing", (event: any) => {
          if (event.payload?.sessionId !== groupSessionId) return;
          setLoadCount((c) => c + 1);
        });
        const success = await listen("group-dynamic-memory:success", (event: any) => {
          if (event.payload?.sessionId !== groupSessionId) return;
          setLoadCount((c) => c + 1);
        });
        const cancelled = await listen("group-dynamic-memory:cancelled", (event: any) => {
          if (event.payload?.sessionId !== groupSessionId) return;
          setLoadCount((c) => c + 1);
        });
        const failure = await listen("group-dynamic-memory:error", (event: any) => {
          if (event.payload?.sessionId !== groupSessionId) return;
          setLoadCount((c) => c + 1);
        });
        unlisteners = [processing, success, cancelled, failure];
      } catch (err) {
        console.error("GroupChatLayout: failed to setup memory listeners", err);
      }
    };

    void setup();
    return () => {
      unlisteners.forEach((unlisten) => unlisten());
    };
  }, [groupSessionId]);

  const reloadSession = useCallback(() => {
    setLoadCount((c) => c + 1);
  }, []);

  const updateSession = useCallback((nextSession: GroupSession | null) => {
    setSession(nextSession);
  }, []);

  const updateGroup = useCallback(
    (nextGroup: Group | null) => {
      setGroup(nextGroup);
      const globalAppearance =
        settings?.advancedSettings?.chatAppearance ?? createDefaultChatAppearanceSettings();
      setBaseChatAppearance(
        mergeChatAppearance(globalAppearance, nextGroup?.chatAppearance ?? undefined),
      );
    },
    [settings],
  );

  const backgroundImageData = useImageData(session?.backgroundImagePath);

  useEffect(() => {
    let mounted = true;

    if (!backgroundImageData) {
      setBgBrightness(null);
      computeChatTheme(chatAppearance, null).then((t) => {
        if (mounted) setTheme(t);
      });
      return () => {
        mounted = false;
      };
    }

    analyzeImageBrightness(backgroundImageData).then((brightness) => {
      if (!mounted) return;
      setBgBrightness(brightness);
      computeChatTheme(chatAppearance, brightness).then((t) => {
        if (mounted) setTheme(t);
      });
    });

    return () => {
      mounted = false;
    };
  }, [backgroundImageData, chatAppearance]);

  const isBackgroundLight = bgBrightness !== null && bgBrightness > 127.5;

  const ctx: GroupChatLayoutContext = {
    session,
    sessionLoading: loading,
    characters,
    personas,
    settings,
    group,
    backgroundImageData,
    isBackgroundLight,
    theme,
    chatAppearance,
    reloadSession,
    updateSession,
    updateGroup,
    draftAppearanceOverride,
    setDraftAppearanceOverride,
    appearanceFieldUpdater,
    registerAppearanceFieldUpdater,
  };

  return (
    <>
      {backgroundImageData && (
        <div
          className="pointer-events-none fixed inset-0 z-0"
          style={{
            backgroundImage: `url(${backgroundImageData})`,
            backgroundSize: "cover",
            backgroundPosition: "center",
            backgroundRepeat: "no-repeat",
          }}
        />
      )}
      {backgroundImageData && chatAppearance.backgroundBlur > 0 && (
        <div
          className="pointer-events-none fixed inset-0 z-0 transform-gpu"
          style={{
            backdropFilter: `blur(${chatAppearance.backgroundBlur}px)`,
            WebkitBackdropFilter: `blur(${chatAppearance.backgroundBlur}px)`,
            backgroundColor: "rgba(0, 0, 0, 0.01)",
          }}
        />
      )}
      {backgroundImageData && chatAppearance.backgroundDim > 0 && (
        <div
          className="pointer-events-none fixed inset-0 z-0"
          style={{
            backgroundColor: `rgba(0, 0, 0, ${chatAppearance.backgroundDim / 100})`,
          }}
        />
      )}
      <Outlet context={ctx} />
    </>
  );
}
