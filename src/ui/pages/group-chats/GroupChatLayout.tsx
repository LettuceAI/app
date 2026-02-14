import { useCallback, useEffect, useState } from "react";
import { Outlet, useOutletContext, useParams } from "react-router-dom";
import type {
  GroupSession,
  Character,
  Persona,
  Settings,
} from "../../../core/storage/schemas";
import { storageBridge } from "../../../core/storage/files";
import { listCharacters, listPersonas, readSettings } from "../../../core/storage/repo";
import { useImageData } from "../../hooks/useImageData";
import {
  isImageLight,
  getThemeForBackground,
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

  const [isBackgroundLight, setIsBackgroundLight] = useState(false);
  const [theme, setTheme] = useState<ThemeColors>(getThemeForBackground(false));

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
    if (!backgroundImageData) {
      setIsBackgroundLight(false);
      setTheme(getThemeForBackground(false));
      return;
    }

    let mounted = true;
    isImageLight(backgroundImageData).then((isLight) => {
      if (mounted) {
        setIsBackgroundLight(isLight);
        setTheme(getThemeForBackground(isLight));
      }
    });
    return () => {
      mounted = false;
    };
  }, [backgroundImageData]);

  const ctx: GroupChatLayoutContext = {
    session,
    sessionLoading: loading,
    characters,
    personas,
    settings,
    backgroundImageData,
    isBackgroundLight,
    theme,
    reloadSession,
  };

  return <Outlet context={ctx} />;
}
