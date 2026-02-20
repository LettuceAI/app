import { useCallback, useEffect, useState } from "react";
import { Outlet, useOutletContext, useParams } from "react-router-dom";
import type { Character, ChatAppearanceSettings } from "../../../core/storage/schemas";
import {
  createDefaultChatAppearanceSettings,
  mergeChatAppearance,
} from "../../../core/storage/schemas";
import { listCharacters, readSettings } from "../../../core/storage/repo";
import { SETTINGS_UPDATED_EVENT } from "../../../core/storage/repo";
import { useImageData } from "../../hooks/useImageData";
import {
  analyzeImageBrightness,
  computeChatTheme,
  getDefaultThemeSync,
  type ThemeColors,
} from "../../../core/utils/imageAnalysis";

export interface ChatLayoutContext {
  character: Character | null;
  characterLoading: boolean;
  backgroundImageData: string | undefined;
  isBackgroundLight: boolean;
  theme: ThemeColors;
  chatAppearance: ChatAppearanceSettings;
  reloadCharacter: () => void;
}

export function useChatLayoutContext() {
  return useOutletContext<ChatLayoutContext>();
}

export function ChatLayout() {
  const { characterId } = useParams<{ characterId: string }>();
  const [character, setCharacter] = useState<Character | null>(null);
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
      if (!characterId) {
        setLoading(false);
        setCharacter(null);
        return;
      }
      try {
        setLoading(true);
        const [chars, settings] = await Promise.all([listCharacters(), readSettings()]);
        const match = chars.find((c) => c.id === characterId) ?? null;
        if (!cancelled) {
          setCharacter(match);
          const globalAppearance =
            settings.advancedSettings?.chatAppearance ?? createDefaultChatAppearanceSettings();
          const merged = mergeChatAppearance(globalAppearance, match?.chatAppearance);
          setChatAppearance(merged);
        }
      } catch (err) {
        console.error("ChatLayout: failed to load character", err);
        if (!cancelled) setCharacter(null);
      } finally {
        if (!cancelled) setLoading(false);
      }
    })();
    return () => {
      cancelled = true;
    };
  }, [characterId, loadCount]);

  useEffect(() => {
    const onSettingsUpdated = () => {
      setLoadCount((c) => c + 1);
    };
    window.addEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
    return () => window.removeEventListener(SETTINGS_UPDATED_EVENT, onSettingsUpdated);
  }, []);

  const reloadCharacter = useCallback(() => {
    setLoadCount((c) => c + 1);
  }, []);

  const backgroundImageData = useImageData(character?.backgroundImagePath);

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

  const ctx: ChatLayoutContext = {
    character,
    characterLoading: loading,
    backgroundImageData,
    isBackgroundLight,
    theme,
    chatAppearance,
    reloadCharacter,
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
          className="pointer-events-none fixed inset-0 z-0 transform-gpu backdrop-blur-md will-change-opacity"
          style={{
            opacity: Math.min(1, chatAppearance.backgroundBlur / 20),
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
