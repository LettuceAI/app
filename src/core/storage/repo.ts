import { z } from "zod";
import { storageBridge } from "./files";
import { getDefaultCharacterRules } from "./defaults";
import {
  CharacterSchema,
  LorebookSchema,
  LorebookEntrySchema,
  SessionSchema,
  SettingsSchema,
  PersonaSchema,
  MessageSchema,
  GroupSessionSchema,
  type Character,
  type Session,
  type Settings,
  type Persona,
  type StoredMessage,
  type ProviderCredential,
  type Model,
  type AppState,
  type Lorebook,
  type LorebookEntry,
  type GroupSession,
  createDefaultSettings,
  createDefaultAccessibilitySettings,
} from "./schemas";

const SessionPreviewSchema = z.object({
  id: z.string(),
  characterId: z.string(),
  title: z.string(),
  updatedAt: z.number(),
  archived: z.boolean(),
  lastMessage: z.string(),
  messageCount: z.number(),
});

export type SessionPreview = z.infer<typeof SessionPreviewSchema>;

export const SETTINGS_UPDATED_EVENT = "lettuceai:settings-updated";
export const SESSION_UPDATED_EVENT = "lettuceai:session-updated";

function broadcastSettingsUpdated() {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent(SETTINGS_UPDATED_EVENT));
  }
}

function broadcastSessionUpdated() {
  if (typeof window !== "undefined") {
    window.dispatchEvent(new CustomEvent(SESSION_UPDATED_EVENT));
  }
}

function now() {
  return Date.now();
}

function uuidv4(): string {
  const bytes = new Uint8Array(16);
  (globalThis.crypto || ({} as any)).getRandomValues?.(bytes);
  bytes[6] = (bytes[6] & 0x0f) | 0x40;
  bytes[8] = (bytes[8] & 0x3f) | 0x80;
  const hex = Array.from(bytes, (b) => b.toString(16).padStart(2, "0"));
  return (
    hex.slice(0, 4).join("") +
    "-" +
    hex.slice(4, 6).join("") +
    "-" +
    hex.slice(6, 8).join("") +
    "-" +
    hex.slice(8, 10).join("") +
    "-" +
    hex.slice(10, 16).join("")
  );
}

export async function readSettings(): Promise<Settings> {
  const fallback = createDefaultSettings();
  const data = await storageBridge.readSettings<Settings>(fallback);

  const parsed = SettingsSchema.safeParse(data);
  if (parsed.success) {
    const settings = parsed.data;

    const modelsWithoutProviderLabel = settings.models.filter((m) => !m.providerLabel);
    const missingAccessibility = !settings.advancedSettings?.accessibility;

    for (const model of modelsWithoutProviderLabel) {
      const providerCred = settings.providerCredentials.find(
        (p) => p.providerId === model.providerId,
      );
      if (providerCred) {
        (model as any).providerLabel = providerCred.label;
      }
    }

    if (missingAccessibility) {
      settings.advancedSettings = {
        ...(settings.advancedSettings ?? {}),
        creationHelperEnabled: settings.advancedSettings?.creationHelperEnabled ?? false,
        helpMeReplyEnabled: settings.advancedSettings?.helpMeReplyEnabled ?? true,
        accessibility: createDefaultAccessibilitySettings(),
      };
      await saveAdvancedSettings(settings.advancedSettings);
    }

    return settings;
  }

  await storageBridge.settingsSetDefaults(null, null);
  return fallback;
}

export async function writeSettings(s: Settings, suppressBroadcast = false): Promise<void> {
  SettingsSchema.parse(s);
  await storageBridge.writeSettings(s);
  if (!suppressBroadcast) {
    broadcastSettingsUpdated();
  }
}

// Granular update functions
export async function setDefaultProvider(id: string | null): Promise<void> {
  await storageBridge.settingsSetDefaultProvider(id);
  broadcastSettingsUpdated();
}

export async function setDefaultModel(id: string | null): Promise<void> {
  await storageBridge.settingsSetDefaultModel(id);
  broadcastSettingsUpdated();
}

export async function setAppState(state: AppState): Promise<void> {
  await storageBridge.settingsSetAppState(state);
  broadcastSettingsUpdated();
}

export async function isAnalyticsAvailable(): Promise<boolean> {
  return storageBridge.analyticsIsAvailable();
}

export async function setPromptTemplate(id: string | null): Promise<void> {
  await storageBridge.settingsSetPromptTemplate(id);
  broadcastSettingsUpdated();
}

export async function setSystemPrompt(prompt: string | null): Promise<void> {
  await storageBridge.settingsSetSystemPrompt(prompt);
  broadcastSettingsUpdated();
}

export async function setMigrationVersion(version: number): Promise<void> {
  await storageBridge.settingsSetMigrationVersion(version);
  broadcastSettingsUpdated();
}

export async function addOrUpdateProviderCredential(
  cred: Omit<ProviderCredential, "id"> & { id?: string },
): Promise<ProviderCredential> {
  const entity: ProviderCredential = await storageBridge.providerUpsert({
    id: cred.id ?? uuidv4(),
    ...cred,
  });
  // Ensure a default provider is set if missing
  const current = await readSettings();
  if (!current.defaultProviderCredentialId) {
    await setDefaultProvider(entity.id);
  }
  broadcastSettingsUpdated();
  return entity;
}

export async function removeProviderCredential(id: string): Promise<void> {
  await storageBridge.providerDelete(id);
  const current = await readSettings();
  if (current.defaultProviderCredentialId === id) {
    const nextDefault = current.providerCredentials.find((c) => c.id !== id)?.id ?? null;
    await setDefaultProvider(nextDefault);
  }
  broadcastSettingsUpdated();
}

export async function addOrUpdateModel(
  model: Omit<Model, "id" | "createdAt"> & { id?: string },
): Promise<Model> {
  const entity: Model = await storageBridge.modelUpsert({ id: model.id ?? uuidv4(), ...model });
  const current = await readSettings();
  if (entity.providerId === "llamacpp") {
    const hasLocalProvider = current.providerCredentials.some(
      (cred) => cred.providerId === "llamacpp",
    );
    if (!hasLocalProvider) {
      await addOrUpdateProviderCredential({
        id: uuidv4(),
        providerId: "llamacpp",
        label: "llama.cpp (Local)",
        apiKey: "",
      });
    }
  }
  if (!current.defaultModelId) {
    await setDefaultModel(entity.id);
  }
  broadcastSettingsUpdated();
  return entity;
}

export async function removeModel(id: string): Promise<void> {
  await storageBridge.modelDelete(id);
  const current = await readSettings();
  if (current.defaultModelId === id) {
    const nextDefault = current.models.find((m) => m.id !== id)?.id ?? null;
    await setDefaultModel(nextDefault);
  }
  broadcastSettingsUpdated();
}

export async function setDefaultModelId(id: string): Promise<void> {
  const settings = await readSettings();
  if (settings.models.find((m) => m.id === id)) {
    await setDefaultModel(id);
  }
}

export async function listCharacters(): Promise<Character[]> {
  const data = await storageBridge.charactersList();
  return z.array(CharacterSchema).parse(data);
}

export async function saveCharacter(c: Partial<Character>): Promise<Character> {
  const settings = await readSettings();
  const pureModeLevel =
    settings.appState.pureModeLevel ?? (settings.appState.pureModeEnabled ? "standard" : "off");
  const defaultRules =
    c.rules && c.rules.length > 0 ? c.rules : await getDefaultCharacterRules(pureModeLevel);
  const timestamp = now();

  const scenes = c.scenes ?? [];
  const defaultSceneId = c.defaultSceneId ?? (scenes.length === 1 ? scenes[0].id : null);
  const derivedScenario =
    scenes.find((scene) => scene.id === defaultSceneId)?.direction?.trim() || undefined;
  const entity: Character = {
    id: c.id ?? globalThis.crypto?.randomUUID?.() ?? uuidv4(),
    name: c.name!,
    nickname: c.nickname,
    avatarPath: c.avatarPath,
    avatarCrop: c.avatarCrop,
    backgroundImagePath: c.backgroundImagePath,
    definition: c.definition,
    description: c.description,
    scenario: derivedScenario,
    creatorNotes: c.creatorNotes,
    creator: c.creator,
    creatorNotesMultilingual: c.creatorNotesMultilingual,
    source: ["lettuceai"],
    tags: c.tags,
    scenes,
    defaultSceneId,
    rules: defaultRules,
    defaultModelId: c.defaultModelId ?? null,
    fallbackModelId: c.fallbackModelId ?? null,
    memoryType: c.memoryType ?? "manual",
    promptTemplateId: c.promptTemplateId ?? null,
    disableAvatarGradient: c.disableAvatarGradient ?? false,
    customGradientEnabled: c.customGradientEnabled ?? false,
    customGradientColors: c.customGradientColors,
    customTextColor: c.customTextColor,
    customTextSecondary: c.customTextSecondary,
    voiceConfig: c.voiceConfig,
    voiceAutoplay: c.voiceAutoplay ?? false,
    chatAppearance: c.chatAppearance,
    createdAt: c.createdAt ?? timestamp,
    updatedAt: timestamp,
  } as Character;

  const stored = await storageBridge.characterUpsert(entity);
  return CharacterSchema.parse(stored);
}

export async function deleteCharacter(id: string): Promise<void> {
  await storageBridge.characterDelete(id);
}

// ============================================================================
// Lorebook
// ============================================================================

export async function listLorebooks(): Promise<Lorebook[]> {
  const data = await storageBridge.lorebooksList();
  return z.array(LorebookSchema).parse(data);
}

export async function saveLorebook(
  lorebook: Partial<Lorebook> & { name: string },
): Promise<Lorebook> {
  const timestamp = now();
  const entity = {
    id: lorebook.id ?? uuidv4(),
    name: lorebook.name,
    createdAt: lorebook.createdAt ?? timestamp,
    updatedAt: timestamp,
  };

  const stored = await storageBridge.lorebookUpsert(entity);
  return LorebookSchema.parse(stored);
}

export async function deleteLorebook(lorebookId: string): Promise<void> {
  await storageBridge.lorebookDelete(lorebookId);
}

export async function listCharacterLorebooks(characterId: string): Promise<Lorebook[]> {
  const data = await storageBridge.characterLorebooksList(characterId);
  return z.array(LorebookSchema).parse(data);
}

export async function setCharacterLorebooks(
  characterId: string,
  lorebookIds: string[],
): Promise<void> {
  await storageBridge.characterLorebooksSet(characterId, lorebookIds);
}

export async function listLorebookEntries(lorebookId: string): Promise<LorebookEntry[]> {
  const data = await storageBridge.lorebookEntriesList(lorebookId);
  return z.array(LorebookEntrySchema).parse(data);
}

export async function getLorebookEntry(entryId: string): Promise<LorebookEntry | null> {
  const data = await storageBridge.lorebookEntryGet(entryId);
  return data ? LorebookEntrySchema.parse(data) : null;
}

export async function saveLorebookEntry(
  entry: Partial<LorebookEntry> & { lorebookId: string },
): Promise<LorebookEntry> {
  const timestamp = now();
  const entity = {
    id: entry.id ?? uuidv4(),
    lorebookId: entry.lorebookId,
    title: entry.title ?? "",
    enabled: entry.enabled ?? true,
    alwaysActive: entry.alwaysActive ?? false,
    keywords: entry.keywords ?? [],
    caseSensitive: entry.caseSensitive ?? false,
    content: entry.content ?? "",
    priority: entry.priority ?? 0,
    displayOrder: entry.displayOrder ?? 0,
    createdAt: entry.createdAt ?? timestamp,
    updatedAt: timestamp,
  };

  const stored = await storageBridge.lorebookEntryUpsert(entity);
  return LorebookEntrySchema.parse(stored);
}

export async function deleteLorebookEntry(entryId: string): Promise<void> {
  await storageBridge.lorebookEntryDelete(entryId);
}

export async function createBlankLorebookEntry(lorebookId: string): Promise<LorebookEntry> {
  const data = await storageBridge.lorebookEntryCreateBlank(lorebookId);
  return LorebookEntrySchema.parse(data);
}

export async function reorderLorebookEntries(updates: Array<[string, number]>): Promise<void> {
  await storageBridge.lorebookEntriesReorder(updates);
}

export async function listSessionIds(): Promise<string[]> {
  return storageBridge.sessionsListIds();
}

export async function listSessionPreviews(
  characterId?: string,
  limit?: number,
): Promise<SessionPreview[]> {
  const data = await storageBridge.sessionsListPreviews(characterId, limit);
  return z.array(SessionPreviewSchema).parse(data);
}

export async function saveAdvancedSettings(settings: Settings["advancedSettings"]): Promise<void> {
  await storageBridge.settingsSetAdvanced(settings);
  broadcastSettingsUpdated();
}

export async function getSession(id: string): Promise<Session | null> {
  const data = await storageBridge.sessionGet(id);
  return data ? SessionSchema.parse(data) : null;
}

export async function getSessionMeta(id: string): Promise<Session | null> {
  const data = await storageBridge.sessionGetMeta(id);
  return data ? SessionSchema.parse(data) : null;
}

export async function getSessionMessageCount(sessionId: string): Promise<number> {
  return storageBridge.sessionMessageCount(sessionId);
}

export async function listMessages(
  sessionId: string,
  options: { limit: number; before?: { createdAt: number; id: string } } = { limit: 120 },
): Promise<StoredMessage[]> {
  const beforeCreatedAt = options.before?.createdAt;
  const beforeId = options.before?.id;
  const data = await storageBridge.messagesList(
    sessionId,
    options.limit,
    beforeCreatedAt,
    beforeId,
  );
  return z.array(MessageSchema).parse(data);
}

export async function listPinnedMessages(sessionId: string): Promise<StoredMessage[]> {
  const data = await storageBridge.messagesListPinned(sessionId);
  return z.array(MessageSchema).parse(data);
}

export async function deleteMessage(sessionId: string, messageId: string): Promise<void> {
  await storageBridge.messageDelete(sessionId, messageId);
}

export async function deleteMessagesAfter(sessionId: string, messageId: string): Promise<void> {
  await storageBridge.messagesDeleteAfter(sessionId, messageId);
}

export async function saveSession(s: Session): Promise<void> {
  SessionSchema.parse(s);
  await storageBridge.sessionUpsert(s);
  broadcastSessionUpdated();
}

export async function archiveSession(id: string, archived = true): Promise<Session | null> {
  await storageBridge.sessionArchive(id, archived);
  broadcastSessionUpdated();
  return getSession(id);
}

export async function updateSessionTitle(id: string, title: string): Promise<Session | null> {
  await storageBridge.sessionUpdateTitle(id, title.trim());
  broadcastSessionUpdated();
  return getSession(id);
}

export async function deleteSession(id: string): Promise<void> {
  await storageBridge.sessionDelete(id);
}

export async function createSession(
  characterId: string,
  title: string,
  selectedSceneId?: string,
): Promise<Session> {
  const id = globalThis.crypto?.randomUUID?.() ?? uuidv4();
  const timestamp = now();

  const messages: StoredMessage[] = [];

  const characters = await listCharacters();
  const character = characters.find((c) => c.id === characterId);

  if (character) {
    const sceneId = selectedSceneId || character.defaultSceneId || character.scenes[0]?.id;

    if (sceneId) {
      const scene = character.scenes.find((s) => s.id === sceneId);
      if (scene) {
        const sceneContent = scene.selectedVariantId
          ? (scene.variants?.find((v) => v.id === scene.selectedVariantId)?.content ??
            scene.content)
          : scene.content;

        if (sceneContent.trim()) {
          messages.push({
            id: globalThis.crypto?.randomUUID?.() ?? uuidv4(),
            role: "scene", // Use "scene" role instead of "assistant"
            content: sceneContent.trim(),
            memoryRefs: [],
            createdAt: timestamp,
          });
        }
      }
    }
  }

  const s: Session = {
    id,
    characterId,
    title,
    selectedSceneId: selectedSceneId || character?.defaultSceneId || character?.scenes[0]?.id,
    personaDisabled: false,
    memories: [],
    memorySummaryTokenCount: 0,
    messages,
    archived: false,
    createdAt: timestamp,
    updatedAt: timestamp,
    memoryStatus: "idle",
  };
  await saveSession(s);
  broadcastSessionUpdated();
  return s;
}

export async function createBranchedSession(
  sourceSession: Session,
  branchAtMessageId: string,
): Promise<Session> {
  const messageIndex = sourceSession.messages.findIndex((m) => m.id === branchAtMessageId);
  if (messageIndex === -1) {
    throw new Error("Message not found in session");
  }

  const id = globalThis.crypto?.randomUUID?.() ?? uuidv4();
  const timestamp = now();

  const branchedMessages: StoredMessage[] = sourceSession.messages
    .slice(0, messageIndex + 1)
    .map((msg) => {
      const newVariants = msg.variants?.map((v) => ({
        ...v,
        id: globalThis.crypto?.randomUUID?.() ?? uuidv4(),
      }));

      const newSelectedVariantId =
        msg.selectedVariantId && msg.variants
          ? newVariants?.[msg.variants.findIndex((v) => v.id === msg.selectedVariantId)]?.id
          : undefined;
      return {
        ...msg,
        id: globalThis.crypto?.randomUUID?.() ?? uuidv4(),
        createdAt: msg.createdAt,
        variants: newVariants,
        selectedVariantId: newSelectedVariantId,
      };
    });

  const s: Session = {
    id,
    characterId: sourceSession.characterId,
    title: `${sourceSession.title} (branch)`,
    selectedSceneId: sourceSession.selectedSceneId,
    personaId: sourceSession.personaId,
    personaDisabled: sourceSession.personaDisabled ?? false,
    memories: [...sourceSession.memories],
    memorySummaryTokenCount: 0,
    messages: branchedMessages,
    archived: false,
    createdAt: timestamp,
    updatedAt: timestamp,
    memoryStatus: sourceSession.memoryStatus || "idle",
    memoryError: sourceSession.memoryError,
  };

  await saveSession(s);
  return s;
}

export async function createBranchedSessionToCharacter(
  sourceSession: Session,
  branchAtMessageId: string,
  targetCharacterId: string,
): Promise<Session> {
  const messageIndex = sourceSession.messages.findIndex((m) => m.id === branchAtMessageId);
  if (messageIndex === -1) {
    throw new Error("Message not found in session");
  }

  const characters = await listCharacters();
  const targetCharacter = characters.find((c) => c.id === targetCharacterId);
  const characterName = targetCharacter?.name || "Unknown";

  const id = globalThis.crypto?.randomUUID?.() ?? uuidv4();
  const timestamp = now();

  const branchedMessages: StoredMessage[] = sourceSession.messages
    .slice(0, messageIndex + 1)
    .filter((msg) => msg.role !== "scene")
    .map((msg) => {
      const newVariants = msg.variants?.map((v) => ({
        ...v,
        id: globalThis.crypto?.randomUUID?.() ?? uuidv4(),
      }));

      const newSelectedVariantId =
        msg.selectedVariantId && msg.variants
          ? newVariants?.[msg.variants.findIndex((v) => v.id === msg.selectedVariantId)]?.id
          : undefined;
      return {
        ...msg,
        id: globalThis.crypto?.randomUUID?.() ?? uuidv4(),
        createdAt: msg.createdAt,
        variants: newVariants,
        selectedVariantId: newSelectedVariantId,
      };
    });

  const s: Session = {
    id,
    characterId: targetCharacterId,
    title: `Branch to ${characterName}`,
    selectedSceneId: targetCharacter?.defaultSceneId || targetCharacter?.scenes?.[0]?.id,
    personaId: sourceSession.personaId,
    personaDisabled: sourceSession.personaDisabled ?? false,
    memories: [],
    memorySummaryTokenCount: 0,
    messages: branchedMessages,
    archived: false,
    createdAt: timestamp,
    updatedAt: timestamp,
    memoryStatus: "idle",
  };

  await saveSession(s);
  return s;
}

export async function toggleMessagePin(
  sessionId: string,
  messageId: string,
): Promise<boolean | null> {
  return storageBridge.messageTogglePin(sessionId, messageId);
}

export async function setMemoryColdState(
  sessionId: string,
  memoryIndex: number,
  isCold: boolean,
): Promise<Session | null> {
  const updated = await storageBridge.sessionSetMemoryColdState(sessionId, memoryIndex, isCold);
  broadcastSessionUpdated();
  return updated ? SessionSchema.parse(updated) : null;
}

// Helper for memory updates
export async function addMemory(
  sessionId: string,
  memory: string,
  memoryCategory?: string,
): Promise<Session | null> {
  const updated = await storageBridge.sessionAddMemory(sessionId, memory, memoryCategory);
  broadcastSessionUpdated();
  return updated ? SessionSchema.parse(updated) : null;
}

export async function removeMemory(
  sessionId: string,
  memoryIndex: number,
): Promise<Session | null> {
  const updated = await storageBridge.sessionRemoveMemory(sessionId, memoryIndex);
  broadcastSessionUpdated();
  return updated ? SessionSchema.parse(updated) : null;
}

export async function updateMemory(
  sessionId: string,
  memoryIndex: number,
  newMemory: string,
  newCategory?: string,
): Promise<Session | null> {
  const updated = await storageBridge.sessionUpdateMemory(
    sessionId,
    memoryIndex,
    newMemory,
    newCategory,
  );
  broadcastSessionUpdated();
  return updated ? SessionSchema.parse(updated) : null;
}

export async function toggleMemoryPin(
  sessionId: string,
  memoryIndex: number,
): Promise<Session | null> {
  const updated = await storageBridge.sessionToggleMemoryPin(sessionId, memoryIndex);
  broadcastSessionUpdated();
  return updated ? SessionSchema.parse(updated) : null;
}

// Group Session Memory CRUD Operations
export async function groupSessionAddMemory(
  sessionId: string,
  memory: string,
): Promise<GroupSession | null> {
  const updated = await storageBridge.groupSessionAddMemory(sessionId, memory);
  return updated ? GroupSessionSchema.parse(updated) : null;
}

export async function groupSessionRemoveMemory(
  sessionId: string,
  memoryIndex: number,
): Promise<GroupSession | null> {
  const updated = await storageBridge.groupSessionRemoveMemory(sessionId, memoryIndex);
  return updated ? GroupSessionSchema.parse(updated) : null;
}

export async function groupSessionUpdateMemory(
  sessionId: string,
  memoryIndex: number,
  newMemory: string,
): Promise<GroupSession | null> {
  const updated = await storageBridge.groupSessionUpdateMemory(sessionId, memoryIndex, newMemory);
  return updated ? GroupSessionSchema.parse(updated) : null;
}

export async function groupSessionToggleMemoryPin(
  sessionId: string,
  memoryIndex: number,
): Promise<GroupSession | null> {
  const updated = await storageBridge.groupSessionToggleMemoryPin(sessionId, memoryIndex);
  return updated ? GroupSessionSchema.parse(updated) : null;
}

export async function groupSessionSetMemoryColdState(
  sessionId: string,
  memoryIndex: number,
  isCold: boolean,
): Promise<GroupSession | null> {
  const updated = await storageBridge.groupSessionSetMemoryColdState(
    sessionId,
    memoryIndex,
    isCold,
  );
  return updated ? GroupSessionSchema.parse(updated) : null;
}

export async function getGroupSession(sessionId: string): Promise<GroupSession | null> {
  const data = await storageBridge.groupSessionGet(sessionId);
  return data ? GroupSessionSchema.parse(data) : null;
}

// Persona management functions
export async function listPersonas(): Promise<Persona[]> {
  const data = await storageBridge.personasList();
  return z.array(PersonaSchema).parse(data);
}

export async function getPersona(id: string): Promise<Persona | null> {
  const personas = await listPersonas();
  return personas.find((p) => p.id === id) || null;
}

export async function savePersona(
  p: Partial<Persona> & { id?: string; title: string; description: string },
): Promise<Persona> {
  const entity: Persona = {
    id: p.id ?? globalThis.crypto?.randomUUID?.() ?? uuidv4(),
    title: p.title,
    description: p.description,
    avatarPath: p.avatarPath,
    avatarCrop: p.avatarCrop,
    isDefault: p.isDefault ?? false,
    createdAt: p.createdAt ?? now(),
    updatedAt: now(),
  } as Persona;

  const saved = await storageBridge.personaUpsert(entity);
  return PersonaSchema.parse(saved);
}

export async function deletePersona(id: string): Promise<void> {
  await storageBridge.personaDelete(id);
}

export async function getDefaultPersona(): Promise<Persona | null> {
  const p = await storageBridge.personaDefaultGet();
  return p ? PersonaSchema.parse(p) : null;
}

export async function checkEmbeddingModel(): Promise<boolean> {
  return storageBridge.checkEmbeddingModel();
}

export async function getEmbeddingModelInfo(): Promise<{
  installed: boolean;
  version: string | null;
  sourceVersion?: string | null;
  selectedSourceVersion?: string | null;
  availableVersions?: string[];
  maxTokens: number;
}> {
  return storageBridge.getEmbeddingModelInfo();
}

export async function runEmbeddingTest(): Promise<{
  success: boolean;
  message: string;
  scores: Array<{
    pairName: string;
    textA: string;
    textB: string;
    similarityScore: number;
    expected: string;
    passed: boolean;
    category: string;
  }>;
  modelInfo: {
    version: string;
    maxTokens: number;
    embeddingDimensions: number;
  };
}> {
  return storageBridge.runEmbeddingTest();
}

export async function generateUserReply(
  sessionId: string,
  currentDraft?: string,
  requestId?: string,
  swapPlaces?: boolean,
): Promise<string> {
  return storageBridge.chatGenerateUserReply(sessionId, currentDraft, requestId, swapPlaces);
}

export async function generateGroupChatUserReply(
  sessionId: string,
  currentDraft?: string,
  requestId?: string,
): Promise<string> {
  return storageBridge.groupChatGenerateUserReply(sessionId, currentDraft, requestId);
}
