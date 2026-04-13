import { z } from "zod";
import { storageBridge } from "./files";
import { getDefaultCharacterRules } from "./defaults";
import {
  CharacterSchema,
  LorebookSchema,
  LorebookEntrySchema,
  SessionSchema,
  SettingsSchema,
  ProviderCredentialSchema,
  ModelSchema,
  AppStateSchema,
  PersonaSchema,
  MessageSchema,
  GroupMessageSchema,
  GroupSchema,
  GroupSessionSchema,
  type Character,
  type Session,
  type Settings,
  type Persona,
  type StoredMessage,
  type Scene,
  type ProviderCredential,
  type Model,
  type AppState,
  type Lorebook,
  type LorebookEntry,
  type Group,
  type GroupSession,
  type GroupMessage,
  createDefaultSettings,
  createDefaultAccessibilitySettings,
} from "./schemas";
import { setDeveloperModeOverride } from "../utils/env";

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

const ImageLibraryItemSchema = z.object({
  id: z.string(),
  bucket: z.string(),
  filePath: z.string(),
  storagePath: z.string(),
  filename: z.string(),
  mimeType: z.string(),
  sizeBytes: z.number().int().nonnegative(),
  updatedAt: z.number().int(),
  width: z.number().int().positive().nullable().optional(),
  height: z.number().int().positive().nullable().optional(),
  entityType: z.string().nullable().optional(),
  entityId: z.string().nullable().optional(),
  variant: z.string().nullable().optional(),
  characterId: z.string().nullable().optional(),
  sessionId: z.string().nullable().optional(),
  role: z.string().nullable().optional(),
});

export type ImageLibraryItem = z.infer<typeof ImageLibraryItemSchema>;
const BackgroundImageRefSchema = z.object({
  backgroundImagePath: z.string().nullish().optional(),
});

export const SETTINGS_UPDATED_EVENT = "lettuceai:settings-updated";
export const SESSION_UPDATED_EVENT = "lettuceai:session-updated";
export const LLAMA_RUNTIME_REPORT_UPDATED_EVENT = "llama-runtime-report-updated";
const UUID_RE = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i;
let lastKnownGoodSettings: Settings | null = null;

function cloneSettingsSnapshot(settings: Settings): Settings {
  return JSON.parse(JSON.stringify(settings)) as Settings;
}

function rememberSettings(settings: Settings): Settings {
  lastKnownGoodSettings = cloneSettingsSnapshot(settings);
  return settings;
}

function updateCachedSettings(mutator: (draft: Settings) => void): void {
  if (!lastKnownGoodSettings) return;
  const draft = cloneSettingsSnapshot(lastKnownGoodSettings);
  mutator(draft);
  lastKnownGoodSettings = draft;
}

function cloneSerializable<T>(value: T): T {
  return JSON.parse(JSON.stringify(value)) as T;
}

function repairSettingsReferentialIntegrity(input: unknown): { next: unknown; changed: boolean } {
  if (!input || typeof input !== "object") {
    return { next: input, changed: false };
  }

  const root = JSON.parse(JSON.stringify(input)) as any;
  let changed = false;
  const providerCredentials = Array.isArray(root?.providerCredentials)
    ? root.providerCredentials
    : [];
  const providerCredentialIdMap = new Map<string, string>();

  for (const credential of providerCredentials) {
    if (!credential || typeof credential !== "object") continue;
    const id = credential.id;
    if (typeof id === "string" && id.length > 0 && !UUID_RE.test(id)) {
      const nextId = uuidv4();
      providerCredentialIdMap.set(id, nextId);
      credential.id = nextId;
      changed = true;
    }
  }

  const defaultProviderCredentialId = root?.defaultProviderCredentialId;
  if (typeof defaultProviderCredentialId === "string" && defaultProviderCredentialId.length > 0) {
    const mappedId = providerCredentialIdMap.get(defaultProviderCredentialId);
    if (mappedId) {
      root.defaultProviderCredentialId = mappedId;
      changed = true;
    } else if (!UUID_RE.test(defaultProviderCredentialId)) {
      root.defaultProviderCredentialId = null;
      changed = true;
    }
  }

  const defaultModelId = root?.defaultModelId;
  if (
    typeof defaultModelId === "string" &&
    defaultModelId.length > 0 &&
    !UUID_RE.test(defaultModelId)
  ) {
    root.defaultModelId = null;
    changed = true;
  }

  const models = Array.isArray(root?.models) ? root.models : [];
  for (const model of models) {
    if (!model || typeof model !== "object") continue;
    const providerCredentialId = model.providerCredentialId;
    if (
      typeof providerCredentialId === "string" &&
      providerCredentialId.length > 0 &&
      providerCredentialIdMap.has(providerCredentialId)
    ) {
      model.providerCredentialId = providerCredentialIdMap.get(providerCredentialId);
      changed = true;
      continue;
    }

    if (
      typeof providerCredentialId === "string" &&
      providerCredentialId.length > 0 &&
      !UUID_RE.test(providerCredentialId)
    ) {
      model.providerCredentialId = null;
      changed = true;
    }
  }

  return { next: root, changed };
}

function salvageSettingsPayload(input: unknown): Settings | null {
  if (!input || typeof input !== "object") {
    return null;
  }

  const root = JSON.parse(JSON.stringify(input)) as Record<string, unknown>;
  const defaults = createDefaultSettings();
  const providerCredentials = Array.isArray(root.providerCredentials)
    ? root.providerCredentials
        .map((value) => ProviderCredentialSchema.safeParse(value))
        .flatMap((result) => (result.success ? [result.data] : []))
    : [];

  const providerById = new Map(providerCredentials.map((provider) => [provider.id, provider]));
  const models = Array.isArray(root.models)
    ? root.models
        .map((value) => {
          if (!value || typeof value !== "object") {
            return null;
          }

          const candidate = { ...(value as Record<string, unknown>) };
          if (
            (!candidate.providerLabel || String(candidate.providerLabel).trim().length === 0) &&
            typeof candidate.providerCredentialId === "string"
          ) {
            const provider = providerById.get(candidate.providerCredentialId);
            if (provider) {
              candidate.providerLabel = provider.label;
            }
          }
          if (candidate.inputScopes == null) {
            candidate.inputScopes = ["text"];
          }
          if (candidate.outputScopes == null) {
            candidate.outputScopes = ["text"];
          }

          const parsed = ModelSchema.safeParse(candidate);
          if (!parsed.success) {
            return null;
          }

          if (
            parsed.data.providerCredentialId &&
            !providerById.has(parsed.data.providerCredentialId)
          ) {
            return null;
          }

          return parsed.data;
        })
        .flatMap((value) => (value ? [value] : []))
    : [];

  const appStateResult = AppStateSchema.safeParse(root.appState);
  const advancedSettingsResult = SettingsSchema.shape.advancedSettings.safeParse(
    root.advancedSettings,
  );
  const defaultProviderCredentialId =
    typeof root.defaultProviderCredentialId === "string" &&
    providerById.has(root.defaultProviderCredentialId)
      ? root.defaultProviderCredentialId
      : null;
  const modelIdSet = new Set(models.map((model) => model.id));
  const defaultModelId =
    typeof root.defaultModelId === "string" && modelIdSet.has(root.defaultModelId)
      ? root.defaultModelId
      : null;

  return {
    $version: 2,
    defaultProviderCredentialId,
    defaultModelId,
    providerCredentials,
    models,
    appState: appStateResult.success ? appStateResult.data : defaults.appState,
    advancedSettings: advancedSettingsResult.success
      ? advancedSettingsResult.data
      : defaults.advancedSettings,
    promptTemplateId: typeof root.promptTemplateId === "string" ? root.promptTemplateId : null,
    systemPrompt: typeof root.systemPrompt === "string" ? root.systemPrompt : null,
    migrationVersion:
      typeof root.migrationVersion === "number" && Number.isInteger(root.migrationVersion)
        ? root.migrationVersion
        : 0,
  };
}

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

type SessionMemoryEmbedding = NonNullable<Session["memoryEmbeddings"]>[number];
type SessionMemoryToolEvent = NonNullable<Session["memoryToolEvents"]>[number];
type SessionMemoryToolAction = NonNullable<SessionMemoryToolEvent["actions"]>[number];

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

function cloneSessionMemoryEmbedding(memory: SessionMemoryEmbedding): SessionMemoryEmbedding {
  return {
    ...memory,
    embedding: [...memory.embedding],
  };
}

function cloneSessionMemoryToolEvent(event: SessionMemoryToolEvent): SessionMemoryToolEvent {
  return {
    ...event,
    windowMessageIds: event.windowMessageIds ? [...event.windowMessageIds] : undefined,
    actions: event.actions.map((action) => ({
      ...action,
      updatedMemories: action.updatedMemories ? [...action.updatedMemories] : undefined,
    })),
  };
}

function remapSessionMemoryToolEvents(
  events: SessionMemoryToolEvent[],
  messageIdMap: Map<string, string>,
): SessionMemoryToolEvent[] {
  return events.map((event) => {
    const remappedWindowMessageIds = event.windowMessageIds
      ?.map((messageId) => messageIdMap.get(messageId))
      .filter((messageId): messageId is string => typeof messageId === "string");

    return {
      ...event,
      windowMessageIds:
        remappedWindowMessageIds && remappedWindowMessageIds.length > 0
          ? remappedWindowMessageIds
          : undefined,
    };
  });
}

function cloneBranchedMessages(
  sourceMessages: StoredMessage[],
  options?: { excludeRoles?: string[] },
): { messages: StoredMessage[]; messageIdMap: Map<string, string> } {
  const excludedRoles = new Set(options?.excludeRoles ?? []);
  const messageIdMap = new Map<string, string>();

  const messages = sourceMessages
    .filter((msg) => !excludedRoles.has(msg.role))
    .map((msg) => {
      const newVariants = msg.variants?.map((v) => ({
        ...v,
        id: globalThis.crypto?.randomUUID?.() ?? uuidv4(),
      }));

      const newSelectedVariantId =
        msg.selectedVariantId && msg.variants
          ? newVariants?.[msg.variants.findIndex((v) => v.id === msg.selectedVariantId)]?.id
          : undefined;
      const newMessageId = globalThis.crypto?.randomUUID?.() ?? uuidv4();
      messageIdMap.set(msg.id, newMessageId);

      return {
        ...msg,
        id: newMessageId,
        createdAt: msg.createdAt,
        variants: newVariants,
        selectedVariantId: newSelectedVariantId,
      };
    });

  return { messages, messageIdMap };
}

function countConversationMessagesUpTo(messages: StoredMessage[], messageIndex: number): number {
  return messages
    .slice(0, messageIndex + 1)
    .filter((message) => message.role === "user" || message.role === "assistant").length;
}

function resolveMemoryIdFromAction(
  action: SessionMemoryToolAction,
  activeMemories: Map<string, SessionMemoryEmbedding>,
): string | null {
  const args = (action.arguments ?? {}) as Record<string, unknown>;
  const explicitMemoryId =
    "memoryId" in action && typeof (action as { memoryId?: unknown }).memoryId === "string"
      ? String((action as { memoryId?: unknown }).memoryId)
      : null;
  if (explicitMemoryId) {
    return explicitMemoryId;
  }

  const idArg = typeof args.id === "string" ? args.id.trim() : "";
  if (idArg) {
    return idArg;
  }

  const textArg = typeof args.text === "string" ? args.text : "";
  if (/^\d{6}$/.test(textArg) && activeMemories.has(textArg)) {
    return textArg;
  }

  for (const [memoryId, memory] of activeMemories) {
    if (memory.text === textArg) {
      return memoryId;
    }
  }

  return null;
}

function buildMemoryFromCreateAction(
  action: SessionMemoryToolAction,
  sourceMemoryById: Map<string, SessionMemoryEmbedding>,
): SessionMemoryEmbedding | null {
  const args = (action.arguments ?? {}) as Record<string, unknown>;
  const memoryId = resolveMemoryIdFromAction(action, sourceMemoryById);
  const text = typeof args.text === "string" ? args.text : "";
  if (!memoryId || !text) {
    return null;
  }

  const sourceMemory = sourceMemoryById.get(memoryId);
  if (sourceMemory) {
    return {
      ...cloneSessionMemoryEmbedding(sourceMemory),
      text,
      isCold: false,
      importanceScore: sourceMemory.importanceScore ?? 1,
      lastAccessedAt: sourceMemory.lastAccessedAt ?? sourceMemory.createdAt,
      isPinned:
        typeof args.important === "boolean" ? Boolean(args.important) : sourceMemory.isPinned,
      category: typeof args.category === "string" ? args.category : (sourceMemory.category ?? null),
    };
  }

  const createdAt = typeof action.timestamp === "number" ? action.timestamp : 0;
  return {
    id: memoryId,
    text,
    embedding: [],
    createdAt,
    tokenCount: 0,
    isCold: false,
    importanceScore: 1,
    lastAccessedAt: createdAt,
    isPinned: Boolean(args.important),
    category: typeof args.category === "string" ? args.category : null,
  };
}

function resolveBranchedDynamicMemoryState(
  sourceSession: Session,
  branchMessageIndex: number,
  messageIdMap?: Map<string, string>,
): Pick<
  Session,
  | "memoryEmbeddings"
  | "memorySummary"
  | "memorySummaryTokenCount"
  | "memoryToolEvents"
  | "memoryStatus"
  | "memoryError"
> {
  const sourceEvents = sourceSession.memoryToolEvents ?? [];
  if (sourceEvents.length === 0) {
    return {
      memoryEmbeddings: (sourceSession.memoryEmbeddings ?? []).map(cloneSessionMemoryEmbedding),
      memorySummary: sourceSession.memorySummary ?? "",
      memorySummaryTokenCount: sourceSession.memorySummaryTokenCount ?? 0,
      memoryToolEvents: [],
      memoryStatus: sourceSession.memoryStatus ?? "idle",
      memoryError: sourceSession.memoryError,
    };
  }

  const branchConversationCount = countConversationMessagesUpTo(
    sourceSession.messages,
    branchMessageIndex,
  );
  const keptEvents = sourceEvents
    .filter((event) => (event.windowEnd ?? 0) <= branchConversationCount)
    .map(cloneSessionMemoryToolEvent);
  const remappedEvents =
    messageIdMap && keptEvents.length > 0
      ? remapSessionMemoryToolEvents(keptEvents, messageIdMap)
      : keptEvents;

  if (remappedEvents.length === 0) {
    return {
      memoryEmbeddings: [],
      memorySummary: "",
      memorySummaryTokenCount: 0,
      memoryToolEvents: [],
      memoryStatus: "idle",
      memoryError: undefined,
    };
  }

  const sourceMemoryById = new Map(
    (sourceSession.memoryEmbeddings ?? []).map((memory) => [memory.id, memory]),
  );
  const activeMemories = new Map<string, SessionMemoryEmbedding>();

  for (const event of remappedEvents) {
    for (const action of event.actions ?? []) {
      if (action.name === "create_memory") {
        const memory = buildMemoryFromCreateAction(action, sourceMemoryById);
        if (memory) {
          activeMemories.set(memory.id, memory);
        }
        continue;
      }

      if (action.name === "delete_memory") {
        const memoryId = resolveMemoryIdFromAction(action, activeMemories);
        if (!memoryId) {
          continue;
        }

        const args = (action.arguments ?? {}) as Record<string, unknown>;
        const confidence = typeof args.confidence === "number" ? args.confidence : undefined;
        const shouldSoftDelete =
          "softDelete" in action
            ? Boolean((action as { softDelete?: unknown }).softDelete)
            : confidence !== undefined && confidence < 0.7;

        if (shouldSoftDelete) {
          const memory = activeMemories.get(memoryId);
          if (memory) {
            activeMemories.set(memoryId, { ...memory, isCold: true });
          }
        } else {
          activeMemories.delete(memoryId);
        }
        continue;
      }

      if (action.name === "pin_memory" || action.name === "unpin_memory") {
        const memoryId = resolveMemoryIdFromAction(action, activeMemories);
        if (!memoryId) {
          continue;
        }

        const memory = activeMemories.get(memoryId);
        if (!memory) {
          continue;
        }

        activeMemories.set(memoryId, {
          ...memory,
          isPinned: action.name === "pin_memory",
        });
      }
    }
  }

  const lastKeptEvent = remappedEvents[remappedEvents.length - 1];
  const memorySummary = lastKeptEvent.summary ?? "";
  const memorySummaryTokenCount =
    memorySummary === (sourceSession.memorySummary ?? "")
      ? (sourceSession.memorySummaryTokenCount ?? 0)
      : 0;

  return {
    memoryEmbeddings: Array.from(activeMemories.values()),
    memorySummary,
    memorySummaryTokenCount,
    memoryToolEvents: remappedEvents,
    memoryStatus: lastKeptEvent.error ? "failed" : "idle",
    memoryError: lastKeptEvent.error,
  };
}

function hasDynamicMemoryState(
  state: Pick<
    Session,
    "memoryEmbeddings" | "memorySummary" | "memorySummaryTokenCount" | "memoryToolEvents"
  >,
): boolean {
  return (
    (state.memoryEmbeddings?.length ?? 0) > 0 ||
    (state.memoryToolEvents?.length ?? 0) > 0 ||
    (state.memorySummary?.trim().length ?? 0) > 0 ||
    (state.memorySummaryTokenCount ?? 0) > 0
  );
}

/**
 * Return the last successfully loaded settings snapshot, or null if none exists yet.
 * Use this to render immediately on page mount, then call readSettings() to refresh.
 */
export function readSettingsCached(): Settings | null {
  return lastKnownGoodSettings ? cloneSettingsSnapshot(lastKnownGoodSettings) : null;
}

export async function readSettings(): Promise<Settings> {
  const fallback = lastKnownGoodSettings
    ? cloneSettingsSnapshot(lastKnownGoodSettings)
    : createDefaultSettings();
  const data = await storageBridge.readSettings<Settings | null>(null);

  if (data == null) {
    if (lastKnownGoodSettings) {
      console.warn("Falling back to last known good settings after read failure.");
      return cloneSettingsSnapshot(lastKnownGoodSettings);
    }
    return fallback;
  }

  const parsed = SettingsSchema.safeParse(data);
  if (parsed.success) {
    const settings = parsed.data;

    const modelsWithoutProviderLabel = settings.models.filter((m) => !m.providerLabel);
    const modelsWithoutProviderCredentialId = settings.models.filter(
      (m) => !m.providerCredentialId,
    );
    const missingAccessibility = !settings.advancedSettings?.accessibility;

    for (const model of modelsWithoutProviderLabel) {
      const providerCred = settings.providerCredentials.find(
        (p) => p.providerId === model.providerId,
      );
      if (providerCred) {
        (model as any).providerLabel = providerCred.label;
      }
    }

    for (const model of modelsWithoutProviderCredentialId) {
      const byLabel = settings.providerCredentials.find(
        (p) => p.providerId === model.providerId && p.label === model.providerLabel,
      );
      if (byLabel) {
        (model as any).providerCredentialId = byLabel.id;
        continue;
      }
      const candidates = settings.providerCredentials.filter(
        (p) => p.providerId === model.providerId,
      );
      if (candidates.length === 1) {
        (model as any).providerCredentialId = candidates[0].id;
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

    return rememberSettings(settings);
  }

  const repaired = repairSettingsReferentialIntegrity(data);
  const repairedParsed = SettingsSchema.safeParse(repaired.next);
  if (repaired.changed && repairedParsed.success) {
    await writeSettings(repairedParsed.data, true);
    return rememberSettings(repairedParsed.data);
  }

  const salvaged = salvageSettingsPayload(repaired.next);
  if (salvaged) {
    console.warn("Salvaged settings payload after validation failure.");
    return rememberSettings(salvaged);
  }

  if (lastKnownGoodSettings) {
    console.warn("Falling back to last known good settings after validation failure.");
    return cloneSettingsSnapshot(lastKnownGoodSettings);
  }

  await storageBridge.settingsSetDefaults(null, null);
  return fallback;
}

export async function writeSettings(s: Settings, suppressBroadcast = false): Promise<void> {
  SettingsSchema.parse(s);
  await storageBridge.writeSettings(s);
  rememberSettings(s);
  if (!suppressBroadcast) {
    broadcastSettingsUpdated();
  }
}

// Granular update functions
export async function setDefaultProvider(id: string | null): Promise<void> {
  await storageBridge.settingsSetDefaultProvider(id);
  updateCachedSettings((settings) => {
    settings.defaultProviderCredentialId = id;
  });
  broadcastSettingsUpdated();
}

export async function setDefaultModel(id: string | null): Promise<void> {
  await storageBridge.settingsSetDefaultModel(id);
  updateCachedSettings((settings) => {
    settings.defaultModelId = id;
  });
  broadcastSettingsUpdated();
}

export async function setAppState(state: AppState): Promise<void> {
  await storageBridge.settingsSetAppState(state);
  updateCachedSettings((settings) => {
    settings.appState = cloneSerializable(state);
  });
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
  updateCachedSettings((settings) => {
    const index = settings.providerCredentials.findIndex((provider) => provider.id === entity.id);
    if (index >= 0) {
      settings.providerCredentials[index] = cloneSerializable(entity);
      return;
    }
    settings.providerCredentials.push(cloneSerializable(entity));
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
  updateCachedSettings((settings) => {
    settings.providerCredentials = settings.providerCredentials.filter((provider) => provider.id !== id);
  });
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
  updateCachedSettings((settings) => {
    const index = settings.models.findIndex((existingModel) => existingModel.id === entity.id);
    if (index >= 0) {
      settings.models[index] = cloneSerializable(entity);
      return;
    }
    settings.models.push(cloneSerializable(entity));
  });
  const current = await readSettings();
  if (!current.defaultModelId) {
    await setDefaultModel(entity.id);
  }
  broadcastSettingsUpdated();
  return entity;
}

export async function removeModel(id: string): Promise<void> {
  await storageBridge.modelDelete(id);
  updateCachedSettings((settings) => {
    settings.models = settings.models.filter((model) => model.id !== id);
  });
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

export async function listImageLibraryItems(): Promise<ImageLibraryItem[]> {
  const data = await storageBridge.imageLibraryList();
  return z.array(ImageLibraryItemSchema).parse(data);
}

export async function downloadImageLibraryItem(
  item: Pick<ImageLibraryItem, "filePath" | "filename">,
): Promise<string> {
  return storageBridge.imageLibraryDownloadToDownloads(item.filePath, item.filename);
}

export async function deleteImageLibraryItem(
  item: Pick<ImageLibraryItem, "storagePath">,
): Promise<void> {
  await storageBridge.imageLibraryDeleteItem(item.storagePath);
}

export async function listReferencedBackgroundImagePaths(): Promise<string[]> {
  const [characters, groups, groupSessions] = await Promise.all([
    listCharacters(),
    storageBridge.groupsList(),
    storageBridge.groupSessionsListAll(),
  ]);

  return [
    ...characters.map((item) => item.backgroundImagePath),
    ...z
      .array(BackgroundImageRefSchema)
      .parse(groups)
      .map((item) => item.backgroundImagePath),
    ...z
      .array(BackgroundImageRefSchema)
      .parse(groupSessions)
      .map((item) => item.backgroundImagePath),
  ].filter((value): value is string => typeof value === "string" && value.length > 0);
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
    designDescription: c.designDescription,
    designReferenceImageIds: c.designReferenceImageIds ?? [],
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
    groupChatPromptTemplateId: c.groupChatPromptTemplateId ?? null,
    groupChatRoleplayPromptTemplateId: c.groupChatRoleplayPromptTemplateId ?? null,
    disableAvatarGradient: c.disableAvatarGradient ?? false,
    customGradientEnabled: c.customGradientEnabled ?? false,
    customGradientColors: c.customGradientColors,
    customTextColor: c.customTextColor,
    customTextSecondary: c.customTextSecondary,
    voiceConfig: c.voiceConfig,
    voiceAutoplay: c.voiceAutoplay ?? false,
    chatAppearance: c.chatAppearance,
    chatTemplates: c.chatTemplates ?? [],
    defaultChatTemplateId: c.defaultChatTemplateId ?? null,
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

export async function listGroups(): Promise<Group[]> {
  const data = await storageBridge.groupsList();
  return z.array(GroupSchema).parse(data);
}

export async function listAllGroupSessions(): Promise<GroupSession[]> {
  const data = await storageBridge.groupSessionsListAll();
  return z.array(GroupSessionSchema).parse(data);
}

export async function saveLorebook(
  lorebook: Partial<Lorebook> & { name: string },
): Promise<Lorebook> {
  const timestamp = now();
  const entity = {
    id: lorebook.id ?? uuidv4(),
    name: lorebook.name,
    avatarPath: lorebook.avatarPath,
    keywordDetectionMode: lorebook.keywordDetectionMode ?? "recentMessageWindow",
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

export async function getGroup(groupId: string): Promise<Group | null> {
  const data = await storageBridge.groupGet(groupId);
  return data ? GroupSchema.parse(data) : null;
}

export async function listGroupLorebooks(groupId: string): Promise<Lorebook[]> {
  const data = await storageBridge.groupLorebooksList(groupId);
  return z.array(LorebookSchema).parse(data);
}

export async function setGroupLorebooks(groupId: string, lorebookIds: string[]): Promise<Group> {
  const data = await storageBridge.groupLorebooksSet(groupId, lorebookIds);
  broadcastSessionUpdated();
  return GroupSchema.parse(data);
}

export async function updateGroupDisableCharacterLorebooks(
  groupId: string,
  disableCharacterLorebooks: boolean,
): Promise<Group> {
  const data = await storageBridge.groupUpdateDisableCharacterLorebooks(
    groupId,
    disableCharacterLorebooks,
  );
  broadcastSessionUpdated();
  return GroupSchema.parse(data);
}

export async function listGroupSessionLorebooks(sessionId: string): Promise<Lorebook[]> {
  const data = await storageBridge.groupSessionLorebooksList(sessionId);
  return z.array(LorebookSchema).parse(data);
}

export async function setGroupSessionLorebooks(
  sessionId: string,
  lorebookIds: string[],
): Promise<GroupSession> {
  const data = await storageBridge.groupSessionLorebooksSet(sessionId, lorebookIds);
  broadcastSessionUpdated();
  return GroupSessionSchema.parse(data);
}

export async function updateGroupSessionDisableCharacterLorebooks(
  sessionId: string,
  disableCharacterLorebooks: boolean,
): Promise<GroupSession> {
  const data = await storageBridge.groupSessionUpdateDisableCharacterLorebooks(
    sessionId,
    disableCharacterLorebooks,
  );
  broadcastSessionUpdated();
  return GroupSessionSchema.parse(data);
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
  updateCachedSettings((current) => {
    current.advancedSettings = settings ? cloneSerializable(settings) : settings;
  });
  setDeveloperModeOverride(settings?.developerModeEnabled === true);
  broadcastSettingsUpdated();
}

export interface HostApiStatus {
  running: boolean;
  bindAddress?: string | null;
  port?: number | null;
  baseUrl?: string | null;
}

export async function getHostApiStatus(): Promise<HostApiStatus> {
  return storageBridge.hostApiGetStatus();
}

export async function startHostApi(): Promise<HostApiStatus> {
  return storageBridge.hostApiStart();
}

export async function stopHostApi(): Promise<HostApiStatus> {
  return storageBridge.hostApiStop();
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

interface SaveSessionOptions {
  preserveDynamicMemory?: boolean;
}

function mergePreservedDynamicMemoryState(latest: Session, next: Session): Session {
  return {
    ...next,
    memories: cloneSerializable(latest.memories ?? []),
    memoryEmbeddings: cloneSerializable(latest.memoryEmbeddings ?? []),
    memorySummary: latest.memorySummary ?? "",
    memorySummaryTokenCount: latest.memorySummaryTokenCount ?? 0,
    memoryToolEvents: cloneSerializable(latest.memoryToolEvents ?? []),
    memoryStatus: latest.memoryStatus ?? "idle",
    memoryError: latest.memoryError,
    memoryProgressStep: latest.memoryProgressStep ?? null,
  };
}

export async function saveSession(
  s: Session,
  options: SaveSessionOptions = {},
): Promise<void> {
  SessionSchema.parse(s);

  let sessionToSave = s;
  if (options.preserveDynamicMemory !== false) {
    const latest = await getSessionMeta(s.id).catch(() => null);
    if (latest) {
      sessionToSave = mergePreservedDynamicMemoryState(latest, s);
    }
  }

  await storageBridge.sessionUpsert(sessionToSave);
  broadcastSessionUpdated();
}

export async function updateSessionBackgroundImage(
  id: string,
  backgroundImagePath: string | null,
): Promise<Session | null> {
  const session = await getSessionMeta(id);
  if (!session) return null;

  const next: Session = {
    ...session,
    backgroundImagePath: backgroundImagePath ?? undefined,
    updatedAt: now(),
  };

  SessionSchema.parse(next);
  await storageBridge.sessionUpsertMeta(next);
  broadcastSessionUpdated();
  return getSessionMeta(id);
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
  templateId?: string,
): Promise<Session> {
  const id = globalThis.crypto?.randomUUID?.() ?? uuidv4();
  const timestamp = now();

  const messages: StoredMessage[] = [];
  let sessionPromptTemplateId: string | null | undefined = undefined;

  const characters = await listCharacters();
  const character = characters.find((c) => c.id === characterId);

  const fallbackSceneId = character
    ? (selectedSceneId ?? character.defaultSceneId ?? character.scenes[0]?.id)
    : selectedSceneId;
  let sessionSceneId = fallbackSceneId;

  if (character && templateId) {
    const template = character.chatTemplates?.find((t) => t.id === templateId);
    if (template) {
      sessionSceneId = template.sceneId ?? undefined;
      sessionPromptTemplateId = template.promptTemplateId ?? character.promptTemplateId ?? null;

      for (let i = 0; i < template.messages.length; i++) {
        const msg = template.messages[i];
        messages.push({
          id: globalThis.crypto?.randomUUID?.() ?? uuidv4(),
          role: msg.role === "user" ? "user" : "assistant",
          content: msg.content,
          memoryRefs: [],
          createdAt: timestamp + i + 1,
        });
      }
    }
  }
  if (sessionPromptTemplateId === undefined) {
    sessionPromptTemplateId = character?.promptTemplateId ?? null;
  }

  if (character && sessionSceneId) {
    const scene = character.scenes.find((s) => s.id === sessionSceneId);
    if (scene) {
      const variantContent = scene.selectedVariantId
        ? (scene.variants?.find((v) => v.id === scene.selectedVariantId)?.content ?? scene.content)
        : undefined;
      const sceneContent =
        variantContent?.trim() || scene.content?.trim() || scene.direction?.trim() || "";

      if (sceneContent) {
        messages.unshift({
          id: globalThis.crypto?.randomUUID?.() ?? uuidv4(),
          role: "scene",
          content: sceneContent,
          memoryRefs: [],
          createdAt: timestamp,
        });
      }
    }
  }

  const s: Session = {
    id,
    characterId,
    title,
    selectedSceneId: sessionSceneId,
    promptTemplateId: sessionPromptTemplateId,
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

  const { messages: branchedMessages, messageIdMap } = cloneBranchedMessages(
    sourceSession.messages.slice(0, messageIndex + 1),
  );
  const branchedDynamicMemoryState = resolveBranchedDynamicMemoryState(
    sourceSession,
    messageIndex,
    messageIdMap,
  );

  const s: Session = {
    id,
    characterId: sourceSession.characterId,
    title: `${sourceSession.title} (branch)`,
    backgroundImagePath: sourceSession.backgroundImagePath,
    selectedSceneId: sourceSession.selectedSceneId,
    promptTemplateId: sourceSession.promptTemplateId,
    personaId: sourceSession.personaId,
    personaDisabled: sourceSession.personaDisabled ?? false,
    memories: [...sourceSession.memories],
    memoryEmbeddings: branchedDynamicMemoryState.memoryEmbeddings,
    memorySummary: branchedDynamicMemoryState.memorySummary,
    memorySummaryTokenCount: branchedDynamicMemoryState.memorySummaryTokenCount,
    memoryToolEvents: branchedDynamicMemoryState.memoryToolEvents,
    messages: branchedMessages,
    archived: false,
    createdAt: timestamp,
    updatedAt: timestamp,
    memoryStatus: branchedDynamicMemoryState.memoryStatus,
    memoryError: branchedDynamicMemoryState.memoryError,
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

  const { messages: branchedMessages, messageIdMap } = cloneBranchedMessages(
    sourceSession.messages.slice(0, messageIndex + 1),
    { excludeRoles: ["scene"] },
  );
  const branchedDynamicMemoryState = resolveBranchedDynamicMemoryState(
    sourceSession,
    messageIndex,
    messageIdMap,
  );

  const s: Session = {
    id,
    characterId: targetCharacterId,
    title: `Branch to ${characterName}`,
    backgroundImagePath: undefined,
    selectedSceneId: targetCharacter?.defaultSceneId ?? targetCharacter?.scenes?.[0]?.id,
    promptTemplateId: targetCharacter?.promptTemplateId ?? null,
    personaId: sourceSession.personaId,
    personaDisabled: sourceSession.personaDisabled ?? false,
    memories: [...sourceSession.memories],
    memoryEmbeddings: branchedDynamicMemoryState.memoryEmbeddings,
    memorySummary: branchedDynamicMemoryState.memorySummary,
    memorySummaryTokenCount: branchedDynamicMemoryState.memorySummaryTokenCount,
    memoryToolEvents: branchedDynamicMemoryState.memoryToolEvents,
    messages: branchedMessages,
    archived: false,
    createdAt: timestamp,
    updatedAt: timestamp,
    memoryStatus: branchedDynamicMemoryState.memoryStatus,
    memoryError: branchedDynamicMemoryState.memoryError,
  };

  await saveSession(s);
  return s;
}

export async function createBranchedGroupSession(
  sourceSession: Session,
  branchAtMessageId: string,
  options: {
    name: string;
    characterIds: string[];
    ownerCharacterId: string;
    personaId?: string | null;
    startingScene?: Scene | null;
    backgroundImagePath?: string | null;
  },
): Promise<GroupSession> {
  const messageIndex = sourceSession.messages.findIndex((m) => m.id === branchAtMessageId);
  if (messageIndex === -1) {
    throw new Error("Message not found in session");
  }

  const group = await storageBridge.groupCreate(
    options.name,
    options.characterIds,
    options.personaId ?? null,
    "roleplay",
    options.startingScene ?? null,
    options.backgroundImagePath ?? null,
  );
  const groupSession = GroupSessionSchema.parse(await storageBridge.groupCreateSession(group.id));

  const messagesToCopy = sourceSession.messages.slice(0, messageIndex + 1);
  const messageIdMap = new Map<string, string>();
  for (const message of messagesToCopy) {
    if (message.role !== "user" && message.role !== "assistant") continue;
    const newMessageId = globalThis.crypto?.randomUUID?.() ?? uuidv4();
    messageIdMap.set(message.id, newMessageId);

    await storageBridge.groupMessageUpsert(groupSession.id, {
      id: newMessageId,
      sessionId: groupSession.id,
      role: message.role,
      content: message.content,
      speakerCharacterId: message.role === "assistant" ? options.ownerCharacterId : null,
      turnNumber: 0,
      createdAt: message.createdAt,
      usage: message.usage ?? null,
      selectedVariantId: null,
      isPinned: Boolean(message.isPinned),
      attachments: message.attachments ?? [],
      reasoning: message.reasoning ?? null,
      selectionReasoning: null,
    });
  }

  const branchedDynamicMemoryState = resolveBranchedDynamicMemoryState(
    sourceSession,
    messageIndex,
    messageIdMap,
  );
  const shouldUseDynamicMemory = hasDynamicMemoryState(branchedDynamicMemoryState);

  if (shouldUseDynamicMemory) {
    await storageBridge.groupSessionUpdateMemoryState(
      groupSession.id,
      sourceSession.memories,
      (branchedDynamicMemoryState.memoryEmbeddings ?? []).map((memory) => ({
        ...memory,
        accessCount: 0,
      })),
      branchedDynamicMemoryState.memorySummary ?? "",
      branchedDynamicMemoryState.memorySummaryTokenCount ?? 0,
      branchedDynamicMemoryState.memoryToolEvents ?? [],
      branchedDynamicMemoryState.memoryStatus ?? "idle",
      branchedDynamicMemoryState.memoryError ?? null,
    );
    await storageBridge.groupSessionUpdateMemoryType(groupSession.id, "dynamic");
  } else {
    await storageBridge.groupSessionUpdateManualMemories(groupSession.id, sourceSession.memories);
  }

  const updatedGroupSession = await storageBridge.groupSessionGet(groupSession.id);
  if (!updatedGroupSession) {
    throw new Error("Failed to load branched group session.");
  }
  broadcastSessionUpdated();
  return GroupSessionSchema.parse(updatedGroupSession);
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
  broadcastSessionUpdated();
  return updated ? GroupSessionSchema.parse(updated) : null;
}

export async function groupSessionRemoveMemory(
  sessionId: string,
  memoryIndex: number,
): Promise<GroupSession | null> {
  const updated = await storageBridge.groupSessionRemoveMemory(sessionId, memoryIndex);
  broadcastSessionUpdated();
  return updated ? GroupSessionSchema.parse(updated) : null;
}

export async function groupSessionUpdateMemory(
  sessionId: string,
  memoryIndex: number,
  newMemory: string,
): Promise<GroupSession | null> {
  const updated = await storageBridge.groupSessionUpdateMemory(sessionId, memoryIndex, newMemory);
  broadcastSessionUpdated();
  return updated ? GroupSessionSchema.parse(updated) : null;
}

export async function groupSessionToggleMemoryPin(
  sessionId: string,
  memoryIndex: number,
): Promise<GroupSession | null> {
  const updated = await storageBridge.groupSessionToggleMemoryPin(sessionId, memoryIndex);
  broadcastSessionUpdated();
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
  broadcastSessionUpdated();
  return updated ? GroupSessionSchema.parse(updated) : null;
}

export async function getGroupSession(sessionId: string): Promise<GroupSession | null> {
  const data = await storageBridge.groupSessionGet(sessionId);
  return data ? GroupSessionSchema.parse(data) : null;
}

export async function listPinnedGroupMessages(sessionId: string): Promise<GroupMessage[]> {
  const data = await storageBridge.groupMessagesListPinned(sessionId);
  return z.array(GroupMessageSchema).parse(data);
}

export async function toggleGroupMessagePin(
  sessionId: string,
  messageId: string,
): Promise<boolean | null> {
  const nextPinned = await storageBridge.groupMessageTogglePin(sessionId, messageId);
  broadcastSessionUpdated();
  return nextPinned;
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
    nickname: p.nickname,
    avatarPath: p.avatarPath,
    avatarCrop: p.avatarCrop,
    designDescription: p.designDescription,
    designReferenceImageIds: p.designReferenceImageIds ?? [],
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
