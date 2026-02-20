import { useCallback, useEffect, useState } from "react";
import { Outlet, useOutletContext, useParams } from "react-router-dom";
import type {
  GroupSession,
  Character,
  Persona,
  Settings,
  ChatAppearanceSettings,
} from "../../../core/storage/schemas";
import { createDefaultChatAppearanceSettings } from "../../../core/storage/schemas";
import { storageBridge } from "../../../core/storage/files";
import { listCharacters, listPersonas, readSettings } from "../../../core/storage/repo";
import { useImageData } from "../../hooks/useImageData";
import {
  analyzeImageBrightness,
  computeChatTheme,
  getDefaultThemeSync,
  type ThemeColors,
} from "../../../core/utils/imageAnalysis";

export interface GroupChatLayoutContext {
  session: GroupSession | null;
  sessionLoading: boolean;
  characters: Character[];
  personas: Persona[];
  settings: Settings | null;
  backgroundImageData: string | undefined;
  isBackgroundLight: boolean;
  theme: ThemeColors;
  chatAppearance: ChatAppearanceSettings;
  reloadSession: () => void;
}

export function useGroupChatLayoutContext() {
  return useOutletContext<GroupChatLayoutContext>();
}

export function GroupChatLayout() {
  const { groupSessionId } = useParams<{ groupSessionId: string }>();
  const [session, setSession] = useState<GroupSession | null>(null);
  const [characters, setCharacters] = useState<Character[]>([]);
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [settings, setSettings] = useState<Settings | null>(null);
  const [loading, setLoading] = useState(true);
  const [loadCount, setLoadCount] = useState(0);

  const [bgBrightness, setBgBrightness] = useState<number | null>(null);
  const [chatAppearance, setChatAppearance] = useState<ChatAppearanceSettings>(
    createDefaultChatAppearanceSettings(),
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
        if (!cancelled) {
          setSession(sessionData);
          setCharacters(chars);
          setPersonas(personaList);
          setSettings(settingsData);
          const globalAppearance =
            settingsData.advancedSettings?.chatAppearance ?? createDefaultChatAppearanceSettings();
          setChatAppearance(globalAppearance);
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

  const reloadSession = useCallback(() => {
    setLoadCount((c) => c + 1);
  }, []);

  const backgroundImageData = useImageData(session?.backgroundImagePath);

  useEffect(() => {
    let mounted = true;

    if (!backgroundImageData) {
      setBgBrightness(null);
      computeChatTheme(chatAppearance, null).then((t) => {
        if (mounted) setTheme(t);
      });
      return () => { mounted = false; };
    }

    analyzeImageBrightness(backgroundImageData).then((brightness) => {
      if (!mounted) return;
      setBgBrightness(brightness);
      computeChatTheme(chatAppearance, brightness).then((t) => {
        if (mounted) setTheme(t);
      });
    });

    return () => { mounted = false; };
  }, [backgroundImageData, chatAppearance]);

  const isBackgroundLight = bgBrightness !== null && bgBrightness > 127.5;

  const ctx: GroupChatLayoutContext = {
    session,
    sessionLoading: loading,
    characters,
    personas,
    settings,
    backgroundImageData,
    isBackgroundLight,
    theme,
    chatAppearance,
    reloadSession,
  };

  return <Outlet context={ctx} />;
}
