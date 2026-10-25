import type { AppEvent, LibraryInteractionMode, PromptKind } from "./generated/bindings";

export const queryKeys = {
  app: {
    all: ["app"] as const,
    status: () => [...queryKeys.app.all, "status"] as const,
  },
  conversations: {
    all: ["conversations"] as const,
    lists: () => [...queryKeys.conversations.all, "list"] as const,
    detail: (conversationId: string) =>
      [...queryKeys.conversations.all, "detail", conversationId] as const,
    messages: (conversationId: string) =>
      [...queryKeys.conversations.detail(conversationId), "messages"] as const,
  },
  memory: {
    all: ["memory"] as const,
    detail: (conversationId: string) => [...queryKeys.memory.all, conversationId] as const,
  },
  characters: {
    all: ["characters"] as const,
    list: () => [...queryKeys.characters.all, "list"] as const,
    draftDefault: (mode: LibraryInteractionMode = "roleplay") =>
      [...queryKeys.characters.all, "draft-default", mode] as const,
  },
  prompts: {
    all: ["prompts"] as const,
    list: (kind: PromptKind) => [...queryKeys.prompts.all, "list", kind] as const,
  },
  library: { all: ["library"] as const, counts: () => ["library", "counts"] as const },
  personas: { all: ["personas"] as const, list: () => ["personas", "list"] as const },
  lorebooks: { all: ["lorebooks"] as const, list: () => ["lorebooks", "list"] as const },
  home: {
    all: ["home"] as const,
    characters: () => [...queryKeys.home.all, "characters"] as const,
  },
  media: {
    all: ["media"] as const,
    library: () => [...queryKeys.media.all, "library"] as const,
    avatarGradient: (assetId: string) =>
      [...queryKeys.media.all, "avatar-gradient", assetId] as const,
  },
  settings: {
    all: ["settings"] as const,
    detail: () => ["settings", "detail"] as const,
    filterLog: () => ["settings", "content-filter"] as const,
  },
  providers: {
    all: ["providers"] as const,
    catalog: () => [...queryKeys.providers.all, "catalog"] as const,
    accounts: () => [...queryKeys.providers.all, "accounts"] as const,
    models: (accountId: string) => [...queryKeys.providers.all, "models", accountId] as const,
  },
  models: {
    all: ["models"] as const,
    list: () => [...queryKeys.models.all, "list"] as const,
  },
  audioProviders: { all: ["audio-providers"] as const },
  companions: { all: ["companions"] as const, emotion: () => ["companions", "emotion"] as const },
  companionNotes: {
    all: ["companion-notes"] as const,
    list: (characterId: string) => ["companion-notes", "list", characterId] as const,
  },
  localModels: { all: ["local-models"] as const },
  loras: { all: ["loras"] as const },
  localImages: { all: ["local-images"] as const },
  civitai: {
    all: ["civitai"] as const,
    auth: () => ["civitai", "auth"] as const,
    search: (query: string, sort: string, period: string, base: string) =>
      ["civitai", "search", query, sort, period, base] as const,
    model: (modelId: number) => ["civitai", "model", modelId] as const,
  },
  speech: { all: ["speech"] as const },
  usage: {
    all: ["usage"] as const,
    time: () => ["usage", "time"] as const,
    clock: () => ["usage", "clock"] as const,
    nano: (id: string) => ["usage", "nano", id] as const,
  },
  backups: { all: ["backups"] as const, list: () => ["backups", "list"] as const },
  embedding: {
    all: ["embedding"] as const,
    status: () => [...queryKeys.embedding.all, "status"] as const,
  },
  downloads: { all: ["downloads"] as const, list: () => ["downloads", "list"] as const },
  jobs: {
    all: ["jobs"] as const,
    lists: () => [...queryKeys.jobs.all, "list"] as const,
    detail: (jobId: string) => [...queryKeys.jobs.all, "detail", jobId] as const,
  },
};

export type QueryKeyPrefix = readonly unknown[];

type AppEventOf<T extends AppEvent["type"]> = Extract<AppEvent, { type: T }>;

type AppEventInvalidations = {
  [T in AppEvent["type"]]?: (event: AppEventOf<T>) => readonly QueryKeyPrefix[];
};

/** Which cached queries an application event makes stale; each key invalidates everything under it. */
export const appEventInvalidations: AppEventInvalidations = {
  generation_settled: (event) => [
    queryKeys.usage.all,
    queryKeys.conversations.lists(),
    queryKeys.home.characters(),
    queryKeys.media.library(),
    queryKeys.library.counts(),
    queryKeys.conversations.detail(event.conversation_id),
  ],
  conversation_changed: (event) => [
    queryKeys.conversations.lists(),
    queryKeys.home.characters(),
    queryKeys.conversations.detail(event.conversation_id),
  ],
  conversation_removed: () => [queryKeys.conversations.lists(), queryKeys.home.characters()],
  memory_changed: (event) => [queryKeys.memory.detail(event.conversation_id)],
  required_models_changed: () => [queryKeys.conversations.all],
  onnx_models_unloaded: () => [
    queryKeys.conversations.all,
    queryKeys.embedding.all,
    queryKeys.speech.all,
  ],
  character_changed: () => [
    queryKeys.characters.all,
    queryKeys.home.characters(),
    queryKeys.library.all,
    queryKeys.media.library(),
    queryKeys.memory.all,
  ],
  persona_changed: () => [
    queryKeys.personas.all,
    queryKeys.conversations.all,
    queryKeys.library.all,
    queryKeys.media.library(),
  ],
  lorebooks_changed: () => [
    queryKeys.lorebooks.all,
    queryKeys.conversations.all,
    queryKeys.library.all,
    queryKeys.media.library(),
  ],
  models_changed: () => [
    queryKeys.companions.all,
    queryKeys.models.all,
    queryKeys.providers.all,
    queryKeys.settings.all,
    queryKeys.localModels.all,
    queryKeys.localImages.all,
    queryKeys.speech.all,
  ],
  database_reloaded: () => [[]],
  settings_changed: (event) =>
    event.section === "ui_state"
      ? [queryKeys.settings.all, queryKeys.app.status()]
      : [
          queryKeys.settings.all,
          queryKeys.providers.all,
          queryKeys.audioProviders.all,
          queryKeys.models.all,
          ...(event.section === "ui_preferences" ? [queryKeys.conversations.all] : []),
          ...(event.section === "general" ? [queryKeys.localImages.all] : []),
        ],
  content_filter_hit: () => [queryKeys.settings.filterLog()],
  app_usage_changed: () => [queryKeys.usage.time(), queryKeys.usage.clock()],
  provider_quota: (event) => [queryKeys.usage.nano(event.account_id)],
  prompts_changed: () => [queryKeys.prompts.all],
  job_updated: (event) => [
    queryKeys.jobs.lists(),
    queryKeys.jobs.detail(event.job.id),
    ...(["succeeded", "failed", "cancelled", "interrupted"].includes(event.job.state)
      ? [queryKeys.usage.all]
      : []),
    ...(event.job.state === "succeeded"
      ? [queryKeys.media.library(), queryKeys.library.counts()]
      : []),
    ...(event.job.kind === "speech_transcribe" &&
    ["succeeded", "failed", "cancelled", "interrupted"].includes(event.job.state)
      ? [queryKeys.speech.all]
      : []),
    ...((event.job.kind === "artifact_install" || event.job.kind === "maintenance") &&
    ["succeeded", "failed", "cancelled", "interrupted"].includes(event.job.state)
      ? [
          queryKeys.localImages.all,
          queryKeys.loras.all,
          queryKeys.speech.all,
          queryKeys.companions.all,
        ]
      : []),
  ],
};

/** The keys to invalidate for `event`; an event type the table does not know invalidates nothing. */
export function invalidationsFor(event: AppEvent): readonly QueryKeyPrefix[] {
  const table = appEventInvalidations as Record<
    string,
    ((event: AppEvent) => readonly QueryKeyPrefix[]) | undefined
  >;
  const rule = Object.hasOwn(table, event.type) ? table[event.type] : undefined;
  return rule ? rule(event) : [];
}
