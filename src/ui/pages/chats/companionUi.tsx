import { useCallback, useEffect, useMemo, useState } from "react";
import type {
  Character,
  CompanionEmotionVector,
  MemoryEntityAnchor,
  Session,
} from "../../../core/storage/schemas";
import { getSessionMeta, listCharacters, listSessionPreviews } from "../../../core/storage/repo";
import { useI18n } from "../../../core/i18n/context";

type T = ReturnType<typeof useI18n>["t"];

export const COMPANION_CATEGORY_ORDER = [
  "relationship",
  "milestone",
  "boundary",
  "preference",
  "profile",
  "routine",
  "episodic",
  "emotional_snapshot",
] as const;

export type CompanionMemoryCategory = (typeof COMPANION_CATEGORY_ORDER)[number];

export function companionCategoryLabel(t: T, category: CompanionMemoryCategory): string {
  switch (category) {
    case "relationship":
      return t("chats.companionUi.relationship");
    case "milestone":
      return t("chats.companionUi.milestones");
    case "boundary":
      return t("chats.companionUi.boundaries");
    case "preference":
      return t("chats.companionUi.preferences");
    case "profile":
      return t("chats.companionUi.profile");
    case "routine":
      return t("chats.companionUi.routines");
    case "episodic":
      return t("chats.companionUi.episodes");
    case "emotional_snapshot":
      return t("chats.companionUi.emotionalSnapshots");
  }
}

export type CompanionMemoryItem = {
  id: string;
  index: number;
  text: string;
  category: CompanionMemoryCategory;
  createdAt: number;
  lastAccessedAt: number;
  isPinned: boolean;
  isCold: boolean;
  importanceScore: number;
  persistenceImportance: number;
  promptImportance: number;
  volatility: number;
  sourceRole: string | null;
  canonicalEntities: MemoryEntityAnchor[];
  factSignature: string | null;
  factPolarity: number | null;
  supersededBy: string | null;
  supersededAt: number | null;
  supersedes: string[];
  isActive: boolean;
};

export function isCompanionChat(character?: Character | null, session?: Session | null): boolean {
  return character?.mode === "companion" || session?.mode === "companion";
}

export function normalizeCompanionCategory(value?: string | null): CompanionMemoryCategory {
  switch (value) {
    case "relationship":
    case "milestone":
    case "boundary":
    case "preference":
    case "profile":
    case "routine":
    case "episodic":
    case "emotional_snapshot":
      return value;
    default:
      return "profile";
  }
}

export function buildCompanionMemoryItems(session?: Session | null): CompanionMemoryItem[] {
  if (!session) return [];

  if (session.memoryEmbeddings?.length) {
    return session.memoryEmbeddings
      .map((memory, index) => ({
        id: memory.id || `memory-${index}`,
        index,
        text: memory.text,
        category: normalizeCompanionCategory(memory.category),
        createdAt: memory.createdAt || 0,
        lastAccessedAt: memory.lastAccessedAt || 0,
        isPinned: memory.isPinned ?? false,
        isCold: memory.isCold ?? false,
        importanceScore: memory.importanceScore ?? 1,
        persistenceImportance: memory.persistenceImportance ?? memory.importanceScore ?? 1,
        promptImportance: memory.promptImportance ?? memory.importanceScore ?? 1,
        volatility: memory.volatility ?? 0.4,
        sourceRole: memory.sourceRole ?? null,
        canonicalEntities: memory.canonicalEntities ?? [],
        factSignature: memory.factSignature ?? null,
        factPolarity: memory.factPolarity ?? null,
        supersededBy: memory.supersededBy ?? null,
        supersededAt: memory.supersededAt ?? null,
        supersedes: memory.supersedes ?? [],
        isActive: !memory.supersededBy,
      }))
      .sort((a, b) => {
        if (a.isPinned !== b.isPinned) return a.isPinned ? -1 : 1;
        if (a.isActive !== b.isActive) return a.isActive ? -1 : 1;
        if (a.promptImportance !== b.promptImportance) {
          return b.promptImportance - a.promptImportance;
        }
        if (a.lastAccessedAt !== b.lastAccessedAt) return b.lastAccessedAt - a.lastAccessedAt;
        return b.createdAt - a.createdAt;
      });
  }

  return (session.memories ?? []).map((text, index) => ({
    id: `memory-${index}`,
    index,
    text,
    category: "profile",
    createdAt: 0,
    lastAccessedAt: 0,
    isPinned: false,
    isCold: false,
    importanceScore: 1,
    persistenceImportance: 1,
    promptImportance: 1,
    volatility: 0.4,
    sourceRole: null,
    canonicalEntities: [],
    factSignature: null,
    factPolarity: null,
    supersededBy: null,
    supersededAt: null,
    supersedes: [],
    isActive: true,
  }));
}

export function formatRelativeTime(t: T, ts?: number): string {
  if (!ts) return t("chats.companionUi.unknownTime");
  const diff = Date.now() - ts;
  const seconds = Math.floor(diff / 1000);
  if (seconds < 60) return t("chats.companionUi.justNow");
  const minutes = Math.floor(seconds / 60);
  if (minutes < 60) return t("chats.companionUi.minutesAgo", { minutes });
  const hours = Math.floor(minutes / 60);
  if (hours < 24) return t("chats.companionUi.hoursAgo", { hours });
  const days = Math.floor(hours / 24);
  if (days < 7) return t("chats.companionUi.daysAgo", { days });
  return new Date(ts).toLocaleDateString();
}

export function formatPercent(value?: number | null): string {
  if (value == null || Number.isNaN(value)) return "0%";
  return `${Math.round(Math.max(0, Math.min(1, value)) * 100)}%`;
}

export function topEmotionEntries(vector?: CompanionEmotionVector | null, limit = 4) {
  if (!vector) return [];
  return Object.entries(vector)
    .map(([key, value]) => ({ key, value: typeof value === "number" ? value : 0 }))
    .sort((a, b) => b.value - a.value)
    .filter((entry) => entry.value > 0.01)
    .slice(0, limit);
}

export function emotionLabel(key: string): string {
  return key
    .replace(/([A-Z])/g, " $1")
    .replace(/^./, (char) => char.toUpperCase())
    .trim();
}

export function summarizeSoulText(t: T, value?: string | null): string {
  if (!value?.trim()) return t("chats.companionUi.notSetYet");
  const compact = value.trim().replace(/\s+/g, " ");
  return compact.length > 160 ? `${compact.slice(0, 157)}...` : compact;
}

export function useCompanionSessionData(characterId?: string, requestedSessionId?: string | null) {
  const [session, setSession] = useState<Session | null>(null);
  const [character, setCharacter] = useState<Character | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const { t } = useI18n();

  const load = useCallback(async () => {
    if (!characterId) {
      setError(t("chats.companionUi.missingCharacterId"));
      setLoading(false);
      return;
    }

    setLoading(true);
    setError(null);
    try {
      const characters = await listCharacters();
      const foundCharacter = characters.find((item) => item.id === characterId) ?? null;
      setCharacter(foundCharacter);

      let targetSession: Session | null = null;
      if (requestedSessionId) {
        targetSession = await getSessionMeta(requestedSessionId).catch(() => null);
      }

      if (!targetSession) {
        const previews = await listSessionPreviews(characterId, 1).catch(() => []);
        const latestId = previews[0]?.id;
        targetSession = latestId ? await getSessionMeta(latestId).catch(() => null) : null;
      }

      if (!targetSession) {
        setSession(null);
        setError(t("chats.companionUi.sessionNotFound"));
        return;
      }

      setSession(targetSession);
    } catch (err: any) {
      setSession(null);
      setError(err?.message || t("chats.companionUi.failedLoadCompanion"));
    } finally {
      setLoading(false);
    }
  }, [characterId, requestedSessionId, t]);

  useEffect(() => {
    void load();
  }, [load]);

  const memoryItems = useMemo(() => buildCompanionMemoryItems(session), [session]);

  return {
    session,
    setSession,
    character,
    loading,
    error,
    reload: load,
    memoryItems,
  };
}
