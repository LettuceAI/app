import { useCallback, useEffect, useState } from "react";
import { Outlet, useOutletContext, useParams } from "react-router-dom";
import type { Character } from "../../../core/storage/schemas";
import { listCharacters } from "../../../core/storage/repo";
import { useImageData } from "../../hooks/useImageData";
import {
  isImageLight,
  getThemeForBackground,
  type ThemeColors,
} from "../../../core/utils/imageAnalysis";

export interface ChatLayoutContext {
  character: Character | null;
  characterLoading: boolean;
  backgroundImageData: string | undefined;
  isBackgroundLight: boolean;
  theme: ThemeColors;
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

  const [isBackgroundLight, setIsBackgroundLight] = useState(false);
  const [theme, setTheme] = useState<ThemeColors>(getThemeForBackground(false));

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
        const chars = await listCharacters();
        const match = chars.find((c) => c.id === characterId) ?? null;
        if (!cancelled) setCharacter(match);
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

  const reloadCharacter = useCallback(() => {
    setLoadCount((c) => c + 1);
  }, []);

  const backgroundImageData = useImageData(character?.backgroundImagePath);

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

  const ctx: ChatLayoutContext = {
    character,
    characterLoading: loading,
    backgroundImageData,
    isBackgroundLight,
    theme,
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
      <Outlet context={ctx} />
    </>
  );
}
