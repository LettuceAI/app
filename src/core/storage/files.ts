import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open } from "@tauri-apps/plugin-dialog";

async function readJsonCommand<T>(
  command: string,
  args?: Record<string, unknown>,
  fallback?: T,
): Promise<T | null> {
  try {
    const result = await invoke<string | null>(command, args ?? {});
    if (!result || result.length === 0) {
      return fallback ?? null;
    }
    return JSON.parse(result) as T;
  } catch (error) {
    console.warn(`Failed to invoke ${command}:`, error);
    return fallback ?? null;
  }
}

async function writeJsonCommand(
  command: string,
  data: unknown,
  args?: Record<string, unknown>,
): Promise<void> {
  const payload = JSON.stringify(data, null, 2);
  await invoke(command, { data: payload, ...(args ?? {}) });
}

export const storageBridge = {
  readSettings: <T>(fallback: T) =>
    readJsonCommand<T>("storage_read_settings", undefined, fallback).then((res) => res ?? fallback),
  writeSettings: (value: unknown) => writeJsonCommand("storage_write_settings", value),
  // New granular commands (phase 1): providers/models/defaults/advanced settings
  settingsSetDefaults: (
    defaultProviderCredentialId: string | null,
    defaultModelId: string | null,
  ) =>
    invoke("settings_set_defaults", {
      defaultProviderCredentialId,
      defaultModelId,
    }) as Promise<void>,
  settingsSetDefaultProvider: (id: string | null) =>
    invoke("settings_set_default_provider", { id }) as Promise<void>,
  settingsSetDefaultModel: (id: string | null) =>
    invoke("settings_set_default_model", { id }) as Promise<void>,
  settingsSetAppState: (state: unknown) =>
    invoke("settings_set_app_state", { stateJson: JSON.stringify(state) }) as Promise<void>,
  analyticsIsAvailable: () => invoke<boolean>("analytics_is_available"),
  settingsSetPromptTemplate: (id: string | null) =>
    invoke("settings_set_prompt_template", { id }) as Promise<void>,
  settingsSetSystemPrompt: (prompt: string | null) =>
    invoke("settings_set_system_prompt", { prompt }) as Promise<void>,
  settingsSetMigrationVersion: (version: number) =>
    invoke("settings_set_migration_version", { version }) as Promise<void>,
  settingsSetAdvancedModelSettings: (advanced: unknown | null) =>
    invoke("settings_set_advanced_model_settings", {
      advancedJson: advanced == null ? "null" : JSON.stringify(advanced),
    }) as Promise<void>,
  settingsSetAdvanced: (advanced: unknown | null) =>
    invoke("settings_set_advanced", {
      advancedJson: advanced == null ? "null" : JSON.stringify(advanced),
    }) as Promise<void>,
  abortRequest: (requestId: string) => invoke("abort_request", { requestId }) as Promise<void>,

  // Embedding model download
  checkEmbeddingModel: () => invoke<boolean>("check_embedding_model"),
  getEmbeddingModelInfo: () =>
    invoke<{
      installed: boolean;
      version: string | null;
      sourceVersion?: string | null;
      selectedSourceVersion?: string | null;
      availableVersions?: string[];
      maxTokens: number;
    }>("get_embedding_model_info"),
  startEmbeddingDownload: (version?: string) =>
    invoke("start_embedding_download", { version: version ?? null }) as Promise<void>,
  getEmbeddingDownloadProgress: () =>
    invoke<{
      downloaded: number;
      total: number;
      status: string;
      currentFileIndex: number;
      totalFiles: number;
      currentFileName: string;
    }>("get_embedding_download_progress"),
  listenToEmbeddingDownloadProgress: (
    callback: (progress: {
      downloaded: number;
      total: number;
      status: string;
      currentFileIndex: number;
      totalFiles: number;
      currentFileName: string;
    }) => void,
  ) =>
    listen<{
      downloaded: number;
      total: number;
      status: string;
      currentFileIndex: number;
      totalFiles: number;
      currentFileName: string;
    }>("embedding_download_progress", (event) => callback(event.payload)),
  cancelEmbeddingDownload: () => invoke("cancel_embedding_download") as Promise<void>,
  computeEmbedding: (text: string) => invoke<number[]>("compute_embedding", { text }),
  initializeEmbeddingModel: () => invoke("initialize_embedding_model") as Promise<void>,
  clearEmbeddingRuntimeCache: () => invoke("clear_embedding_runtime_cache") as Promise<void>,
  runEmbeddingTest: () =>
    invoke<{
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
    }>("run_embedding_test"),
  runEmbeddingDevBenchmark: () =>
    invoke<{
      maxTokensUsed: number;
      v2: {
        version: string;
        sampleCount: number;
        averageMs: number;
        p95Ms: number;
        minMs: number;
        maxMs: number;
      };
      v3: {
        version: string;
        sampleCount: number;
        averageMs: number;
        p95Ms: number;
        minMs: number;
        maxMs: number;
      };
      pairDeltas: Array<{
        pairName: string;
        v2Similarity: number;
        v3Similarity: number;
        delta: number;
      }>;
      averageSpeedupV3VsV2: number;
    }>("run_embedding_dev_benchmark"),
  compareCustomTexts: (textA: string, textB: string) =>
    invoke<number>("compare_custom_texts", { textA, textB }),
  deleteEmbeddingModel: () => invoke("delete_embedding_model") as Promise<void>,
  deleteEmbeddingModelVersion: (version: "v1" | "v2" | "v3") =>
    invoke("delete_embedding_model_version", { version }) as Promise<void>,

  providerUpsert: (cred: unknown) =>
    invoke<string>("provider_upsert", { credentialJson: JSON.stringify(cred) }).then((s) =>
      JSON.parse(s),
    ),
  providerDelete: (id: string) => invoke("provider_delete", { id }) as Promise<void>,

  modelUpsert: (model: unknown) =>
    invoke<string>("model_upsert", { modelJson: JSON.stringify(model) }).then((s) => JSON.parse(s)),
  modelDelete: (id: string) => invoke("model_delete", { id }) as Promise<void>,

  // Characters
  charactersList: () => invoke<string>("characters_list").then((s) => JSON.parse(s) as any[]),
  characterUpsert: (character: unknown) =>
    invoke<string>("character_upsert", { characterJson: JSON.stringify(character) }).then((s) =>
      JSON.parse(s),
    ),
  characterDelete: (id: string) => invoke("character_delete", { id }) as Promise<void>,

  // Lorebook
  lorebooksList: () => invoke<string>("lorebooks_list").then((s) => JSON.parse(s) as any[]),
  lorebookUpsert: (lorebook: unknown) =>
    invoke<string>("lorebook_upsert", { lorebookJson: JSON.stringify(lorebook) }).then((s) =>
      JSON.parse(s),
    ),
  lorebookDelete: (lorebookId: string) =>
    invoke("lorebook_delete", { lorebookId }) as Promise<void>,
  characterLorebooksList: (characterId: string) =>
    invoke<string>("character_lorebooks_list", { characterId }).then((s) => JSON.parse(s) as any[]),
  characterLorebooksSet: (characterId: string, lorebookIds: string[]) =>
    invoke("character_lorebooks_set", {
      characterId,
      lorebookIdsJson: JSON.stringify(lorebookIds),
    }) as Promise<void>,

  lorebookEntriesList: (lorebookId: string) =>
    invoke<string>("lorebook_entries_list", { lorebookId }).then((s) => JSON.parse(s) as any[]),
  lorebookEntryGet: (entryId: string) =>
    invoke<string | null>("lorebook_entry_get", { entryId }).then((s) =>
      typeof s === "string" ? JSON.parse(s) : null,
    ),
  lorebookEntryUpsert: (entry: unknown) =>
    invoke<string>("lorebook_entry_upsert", { entryJson: JSON.stringify(entry) }).then((s) =>
      JSON.parse(s),
    ),
  lorebookEntryDelete: (entryId: string) =>
    invoke("lorebook_entry_delete", { entryId }) as Promise<void>,
  lorebookEntryCreateBlank: (lorebookId: string) =>
    invoke<string>("lorebook_entry_create_blank", { lorebookId }).then((s) => JSON.parse(s)),
  lorebookEntriesReorder: (updates: Array<[string, number]>) =>
    invoke("lorebook_entries_reorder", { updatesJson: JSON.stringify(updates) }) as Promise<void>,

  // Personas
  personasList: () => invoke<string>("personas_list").then((s) => JSON.parse(s) as any[]),
  personaUpsert: (persona: unknown) =>
    invoke<string>("persona_upsert", { personaJson: JSON.stringify(persona) }).then((s) =>
      JSON.parse(s),
    ),
  personaDelete: (id: string) => invoke("persona_delete", { id }) as Promise<void>,
  personaDefaultGet: () =>
    invoke<string | null>("persona_default_get").then((s) =>
      typeof s === "string" ? (JSON.parse(s) as any) : null,
    ),

  // Sessions
  sessionsListIds: () => invoke<string>("sessions_list_ids").then((s) => JSON.parse(s) as string[]),
  sessionsListPreviews: (characterId?: string, limit?: number) =>
    invoke<string>("sessions_list_previews", {
      characterId: characterId ?? null,
      limit: limit ?? null,
    }).then((s) => JSON.parse(s) as any[]),
  sessionGet: (id: string) =>
    invoke<string | null>("session_get", { id }).then((s) =>
      typeof s === "string" ? JSON.parse(s) : null,
    ),
  sessionGetMeta: (id: string) =>
    invoke<string | null>("session_get_meta", { id }).then((s) =>
      typeof s === "string" ? JSON.parse(s) : null,
    ),
  sessionMessageCount: (sessionId: string) =>
    invoke<number>("session_message_count", { sessionId }),
  sessionUpsert: (session: unknown) =>
    invoke("session_upsert", { sessionJson: JSON.stringify(session) }) as Promise<void>,
  sessionDelete: (id: string) => invoke("session_delete", { id }) as Promise<void>,
  sessionArchive: (id: string, archived: boolean) =>
    invoke("session_archive", { id, archived }) as Promise<void>,
  sessionUpdateTitle: (id: string, title: string) =>
    invoke("session_update_title", { id, title }) as Promise<void>,
  messageTogglePin: (sessionId: string, messageId: string) =>
    invoke<boolean | null>("message_toggle_pin_state", { sessionId, messageId }),
  sessionAddMemory: (sessionId: string, memory: string, memoryCategory?: string) =>
    invoke<string | null>("session_add_memory", {
      sessionId,
      memory,
      memoryCategory: memoryCategory ?? null,
    }).then((s) => (typeof s === "string" ? JSON.parse(s) : null)),
  sessionRemoveMemory: (sessionId: string, memoryIndex: number) =>
    invoke<string | null>("session_remove_memory", { sessionId, memoryIndex }).then((s) =>
      typeof s === "string" ? JSON.parse(s) : null,
    ),
  sessionUpdateMemory: (
    sessionId: string,
    memoryIndex: number,
    newMemory: string,
    newCategory?: string,
  ) =>
    invoke<string | null>("session_update_memory", {
      sessionId,
      memoryIndex,
      newMemory,
      newCategory: newCategory ?? null,
    }).then((s) => (typeof s === "string" ? JSON.parse(s) : null)),
  sessionToggleMemoryPin: (sessionId: string, memoryIndex: number) =>
    invoke<string | null>("session_toggle_memory_pin", { sessionId, memoryIndex }).then((s) =>
      typeof s === "string" ? JSON.parse(s) : null,
    ),
  sessionSetMemoryColdState: (sessionId: string, memoryIndex: number, isCold: boolean) =>
    invoke<string | null>("session_set_memory_cold_state", { sessionId, memoryIndex, isCold }).then(
      (s) => (typeof s === "string" ? JSON.parse(s) : null),
    ),

  // Messages (paged)
  messagesList: (sessionId: string, limit: number, beforeCreatedAt?: number, beforeId?: string) =>
    invoke<string>("messages_list", {
      sessionId,
      limit,
      beforeCreatedAt: beforeCreatedAt ?? null,
      beforeId: beforeId ?? null,
    }).then((s) => JSON.parse(s) as any[]),
  messagesListPinned: (sessionId: string) =>
    invoke<string>("messages_list_pinned", { sessionId }).then((s) => JSON.parse(s) as any[]),
  messageDelete: (sessionId: string, messageId: string) =>
    invoke("message_delete", { sessionId, messageId }) as Promise<void>,
  messagesDeleteAfter: (sessionId: string, messageId: string) =>
    invoke("messages_delete_after", { sessionId, messageId }) as Promise<void>,

  clearAll: () => invoke("storage_clear_all"),
  resetDatabase: () => invoke("storage_reset_database") as Promise<void>,
  retryDynamicMemory: (sessionId: string, modelId?: string, updateDefault?: boolean) =>
    invoke("retry_dynamic_memory", {
      sessionId,
      modelId: modelId ?? null,
      updateDefault: updateDefault ?? null,
    }) as Promise<void>,
  triggerDynamicMemory: (sessionId: string) =>
    invoke("trigger_dynamic_memory", { sessionId }) as Promise<void>,
  usageSummary: () =>
    invoke("storage_usage_summary") as Promise<{
      fileCount: number;
      estimatedSessions: number;
      lastUpdatedMs: number | null;
    }>,

  // Search
  searchMessages: (sessionId: string, query: string) =>
    invoke<
      {
        messageId: string;
        content: string;
        createdAt: number;
        role: string;
      }[]
    >("search_messages", { sessionId, query }),

  chatGenerateUserReply: (
    sessionId: string,
    currentDraft?: string,
    requestId?: string,
    swapPlaces?: boolean,
  ) =>
    invoke<string>("chat_generate_user_reply", {
      sessionId,
      currentDraft: currentDraft ?? null,
      requestId: requestId ?? null,
      swapPlaces: swapPlaces ?? null,
    }),

  groupChatGenerateUserReply: (sessionId: string, currentDraft?: string, requestId?: string) =>
    invoke<string>("group_chat_generate_user_reply", {
      sessionId,
      currentDraft: currentDraft ?? null,
      requestId: requestId ?? null,
    }),

  dbCheckpoint: () => invoke("db_checkpoint") as Promise<void>,
  dbOptimize: () => invoke("db_optimize") as Promise<void>,

  // Full app backup/restore
  backupExport: (password?: string) =>
    invoke<string>("backup_export", { password: password ?? null }),
  backupImport: (backupPath: string, password?: string) =>
    invoke("backup_import", { backupPath, password: password ?? null }) as Promise<void>,
  backupCheckEncrypted: (backupPath: string) =>
    invoke<boolean>("backup_check_encrypted", { backupPath }),
  backupVerifyPassword: (backupPath: string, password: string) =>
    invoke<boolean>("backup_verify_password", { backupPath, password }),
  backupGetInfo: (backupPath: string) =>
    invoke<{
      version: number;
      createdAt: number;
      appVersion: string;
      encrypted: boolean;
      totalFiles: number;
      imageCount: number;
      avatarCount: number;
      attachmentCount: number;
    }>("backup_get_info", { backupPath }),
  backupList: () =>
    invoke<
      Array<{
        version: number;
        createdAt: number;
        appVersion: string;
        encrypted: boolean;
        totalFiles: number;
        imageCount: number;
        avatarCount: number;
        attachmentCount: number;
        path: string;
        filename: string;
      }>
    >("backup_list"),
  backupDelete: (backupPath: string) => invoke("backup_delete", { backupPath }) as Promise<void>,

  // Byte-based operations for Android content URI support
  backupGetInfoFromBytes: (data: Uint8Array) =>
    invoke<{
      version: number;
      createdAt: number;
      appVersion: string;
      encrypted: boolean;
      totalFiles: number;
      imageCount: number;
      avatarCount: number;
      attachmentCount: number;
    }>("backup_get_info_from_bytes", { data: Array.from(data) }),
  backupCheckEncryptedFromBytes: (data: Uint8Array) =>
    invoke<boolean>("backup_check_encrypted_from_bytes", { data: Array.from(data) }),
  backupVerifyPasswordFromBytes: (data: Uint8Array, password: string) =>
    invoke<boolean>("backup_verify_password_from_bytes", { data: Array.from(data), password }),
  backupImportFromBytes: (data: Uint8Array, password?: string) =>
    invoke("backup_import_from_bytes", {
      data: Array.from(data),
      password: password ?? null,
    }) as Promise<void>,
  backupCheckDynamicMemory: (backupPath: string, password?: string) =>
    invoke<boolean>("backup_check_dynamic_memory", { backupPath, password: password ?? null }),
  backupCheckDynamicMemoryFromBytes: (data: Uint8Array, password?: string) =>
    invoke<boolean>("backup_check_dynamic_memory_from_bytes", {
      data: Array.from(data),
      password: password ?? null,
    }),
  backupDisableDynamicMemory: () => invoke("backup_disable_dynamic_memory") as Promise<void>,

  // Get the storage root path for temp file operations
  getStorageRoot: () => invoke<string>("get_storage_root"),

  // ============================================================================
  // Group Chat Storage Bridge
  // ============================================================================

  // Group Sessions
  groupSessionsList: () =>
    invoke<string>("group_sessions_list").then((s) => JSON.parse(s) as any[]),
  groupSessionsListAll: () =>
    invoke<string>("group_sessions_list_all").then((s) => JSON.parse(s) as any[]),
  groupSessionCreate: (
    name: string,
    characterIds: string[],
    personaId?: string | null,
    chatType?: "conversation" | "roleplay",
    startingScene?: any | null,
    backgroundImagePath?: string | null,
    speakerSelectionMethod?: "llm" | "heuristic" | "round_robin" | null,
  ) =>
    invoke<string>("group_session_create", {
      name,
      characterIdsJson: JSON.stringify(characterIds),
      personaId: personaId ?? null,
      chatType: chatType ?? "conversation",
      startingSceneJson: startingScene ? JSON.stringify(startingScene) : null,
      backgroundImagePath: backgroundImagePath ?? null,
      speakerSelectionMethod: speakerSelectionMethod ?? "llm",
    }).then((s) => JSON.parse(s)),
  groupSessionGet: (id: string) =>
    invoke<string | null>("group_session_get", { id }).then((s) =>
      typeof s === "string" ? JSON.parse(s) : null,
    ),
  groupSessionUpdate: (
    id: string,
    name: string,
    characterIds: string[],
    personaId?: string | null,
  ) =>
    invoke<string>("group_session_update", {
      id,
      name,
      characterIdsJson: JSON.stringify(characterIds),
      personaId: personaId ?? null,
    }).then((s) => JSON.parse(s)),
  groupSessionDelete: (id: string) => invoke("group_session_delete", { id }) as Promise<void>,
  groupSessionArchive: (id: string, archived: boolean) =>
    invoke("group_session_archive", { id, archived }) as Promise<void>,
  groupSessionUpdateTitle: (id: string, title: string) =>
    invoke("group_session_update_title", { id, title }) as Promise<void>,
  groupSessionDuplicate: (sourceId: string, newName?: string | null) =>
    invoke<string>("group_session_duplicate", {
      sourceId,
      newName: newName ?? null,
    }).then((s) => JSON.parse(s)),
  groupSessionDuplicateWithMessages: (
    sourceId: string,
    includeMessages: boolean,
    newName?: string | null,
  ) =>
    invoke<string>("group_session_duplicate_with_messages", {
      sourceId,
      includeMessages,
      newName: newName ?? null,
    }).then((s) => JSON.parse(s)),
  groupSessionBranchToCharacter: (sourceId: string, characterId: string, newName?: string | null) =>
    invoke<string>("group_session_branch_to_character", {
      sourceId,
      characterId,
      newName: newName ?? null,
    }).then((s) => JSON.parse(s)),
  groupSessionAddCharacter: (sessionId: string, characterId: string) =>
    invoke<string>("group_session_add_character", { sessionId, characterId }).then((s) =>
      JSON.parse(s),
    ),
  groupSessionRemoveCharacter: (sessionId: string, characterId: string) =>
    invoke<string>("group_session_remove_character", { sessionId, characterId }).then((s) =>
      JSON.parse(s),
    ),
  groupSessionUpdateStartingScene: (sessionId: string, startingScene: any | null) =>
    invoke<string>("group_session_update_starting_scene", {
      sessionId,
      startingSceneJson: startingScene ? JSON.stringify(startingScene) : null,
    }).then((s) => JSON.parse(s)),
  groupSessionUpdateBackgroundImage: (sessionId: string, backgroundImagePath: string | null) =>
    invoke<string>("group_session_update_background_image", {
      sessionId,
      backgroundImagePath,
    }).then((s) => JSON.parse(s)),
  groupSessionUpdateChatType: (sessionId: string, chatType: "conversation" | "roleplay") =>
    invoke<string>("group_session_update_chat_type", {
      sessionId,
      chatType,
    }).then((s) => JSON.parse(s)),
  groupSessionUpdateSpeakerSelectionMethod: (
    sessionId: string,
    speakerSelectionMethod: "llm" | "heuristic" | "round_robin",
  ) =>
    invoke<string>("group_session_update_speaker_selection_method", {
      sessionId,
      speakerSelectionMethod,
    }).then((s) => JSON.parse(s)),

  // Group Participation
  groupParticipationStats: (sessionId: string) =>
    invoke<string>("group_participation_stats", { sessionId }).then((s) => JSON.parse(s) as any[]),
  groupParticipationIncrement: (sessionId: string, characterId: string, turnNumber: number) =>
    invoke("group_participation_increment", {
      sessionId,
      characterId,
      turnNumber,
    }) as Promise<void>,

  // Group Messages
  groupMessagesList: (
    sessionId: string,
    limit: number,
    beforeCreatedAt?: number,
    beforeId?: string,
  ) =>
    invoke<string>("group_messages_list", {
      sessionId,
      limit,
      beforeCreatedAt: beforeCreatedAt ?? null,
      beforeId: beforeId ?? null,
    }).then((s) => JSON.parse(s) as any[]),
  groupMessageUpsert: (sessionId: string, message: unknown) =>
    invoke<string>("group_message_upsert", {
      sessionId,
      messageJson: JSON.stringify(message),
    }).then((s) => JSON.parse(s)),
  groupMessageDelete: (sessionId: string, messageId: string) =>
    invoke("group_message_delete", { sessionId, messageId }) as Promise<void>,
  groupMessagesDeleteAfter: (sessionId: string, messageId: string) =>
    invoke("group_messages_delete_after", { sessionId, messageId }) as Promise<void>,
  groupMessageAddVariant: (messageId: string, variant: unknown) =>
    invoke<string>("group_message_add_variant", {
      messageId,
      variantJson: JSON.stringify(variant),
    }).then((s) => JSON.parse(s)),
  groupMessageSelectVariant: (messageId: string, variantId: string) =>
    invoke("group_message_select_variant", { messageId, variantId }) as Promise<void>,
  groupMessageCount: (sessionId: string) => invoke<number>("group_message_count", { sessionId }),

  // Group Chat (high-level operations)
  groupChatSend: (sessionId: string, userMessage: string, stream?: boolean, requestId?: string) =>
    invoke<string>("group_chat_send", {
      sessionId,
      userMessage,
      stream: stream ?? true,
      requestId: requestId ?? null,
    }).then((s) => JSON.parse(s)),
  groupChatRegenerate: (
    sessionId: string,
    messageId: string,
    forceCharacterId?: string | null,
    requestId?: string,
  ) =>
    invoke<string>("group_chat_regenerate", {
      sessionId,
      messageId,
      forceCharacterId: forceCharacterId ?? null,
      requestId: requestId ?? null,
    }).then((s) => JSON.parse(s)),
  groupChatContinue: (sessionId: string, forceCharacterId?: string | null, requestId?: string) =>
    invoke<string>("group_chat_continue", {
      sessionId,
      forceCharacterId: forceCharacterId ?? null,
      requestId: requestId ?? null,
    }).then((s) => JSON.parse(s)),
  groupChatGetSelectionPrompt: (sessionId: string, userMessage: string) =>
    invoke<string>("group_chat_get_selection_prompt", { sessionId, userMessage }),
  groupChatRetryDynamicMemory: (sessionId: string) =>
    invoke("group_chat_retry_dynamic_memory", { sessionId }) as Promise<void>,

  // Group Session Memory Operations
  groupSessionUpdateMemories: (
    sessionId: string,
    memoryEmbeddings: unknown[],
    memorySummary?: string | null,
    memorySummaryTokenCount?: number | null,
  ) =>
    invoke("group_session_update_memories", {
      sessionId,
      memoryEmbeddingsJson: JSON.stringify(memoryEmbeddings),
      memorySummary: memorySummary ?? null,
      memorySummaryTokenCount: memorySummaryTokenCount ?? null,
    }) as Promise<void>,
  groupSessionUpdateManualMemories: (sessionId: string, memories: string[]) =>
    invoke("group_session_update_manual_memories", {
      sessionId,
      memoriesJson: JSON.stringify(memories),
    }) as Promise<void>,
  groupSessionAddMemory: (sessionId: string, memory: string) =>
    invoke<string | null>("group_session_add_memory", {
      sessionId,
      memory,
    }).then((s) => (typeof s === "string" ? JSON.parse(s) : null)),
  groupSessionRemoveMemory: (sessionId: string, memoryIndex: number) =>
    invoke<string | null>("group_session_remove_memory", {
      sessionId,
      memoryIndex,
    }).then((s) => (typeof s === "string" ? JSON.parse(s) : null)),
  groupSessionUpdateMemory: (sessionId: string, memoryIndex: number, newMemory: string) =>
    invoke<string | null>("group_session_update_memory", {
      sessionId,
      memoryIndex,
      newMemory,
    }).then((s) => (typeof s === "string" ? JSON.parse(s) : null)),
  groupSessionToggleMemoryPin: (sessionId: string, memoryIndex: number) =>
    invoke<string | null>("group_session_toggle_memory_pin", {
      sessionId,
      memoryIndex,
    }).then((s) => (typeof s === "string" ? JSON.parse(s) : null)),
  groupSessionSetMemoryColdState: (sessionId: string, memoryIndex: number, isCold: boolean) =>
    invoke<string | null>("group_session_set_memory_cold_state", {
      sessionId,
      memoryIndex,
      isCold,
    }).then((s) => (typeof s === "string" ? JSON.parse(s) : null)),

  backupPickFile: async (): Promise<{ path: string; filename: string } | null> => {
    try {
      const selected = await open({
        multiple: false,
      });

      if (!selected || typeof selected !== "string") return null;

      console.log("[backupPickFile] Selected file:", selected);

      const isContentUri = selected.startsWith("content://");

      let filename: string;
      const parts = selected.split("/");
      filename = parts[parts.length - 1] || "backup.lettuce";
      if (filename.startsWith("content:") || filename.includes("%")) {
        filename = "backup.lettuce";
      }

      if (!filename.endsWith(".lettuce") && !filename.endsWith(".zip")) {
        filename = filename + ".lettuce";
      }

      if (isContentUri) {
        console.log(
          "[backupPickFile] Android content URI detected, passing URI to backend:",
          selected,
        );
        return { path: selected, filename };
      } else {
        console.log("[backupPickFile] Desktop path, using directly:", selected);
        return { path: selected, filename };
      }
    } catch (error) {
      console.error("[backupPickFile] Error:", error);
      throw error;
    }
  },
};
