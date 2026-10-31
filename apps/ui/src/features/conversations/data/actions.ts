import type { InfiniteData, QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { isApiFailure } from "@/api/client";
import type { ConversationSettingsPatch_Deserialize, ConversationSettingsView, MessagePartView, TimelineMessage } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { candidatesQuery, settingsKey, settingsQuery, type TimelinePage } from "./queries";
import { currentRevision, trimTimeline } from "./use-generation";

export interface ConversationServices {
  api: ApiClient;
  queryClient: QueryClient;
  conversationId: string;
  isStuck: () => boolean;
}

function trimIfStuck({ queryClient, conversationId, isStuck }: ConversationServices): void {
  if (isStuck()) trimTimeline(queryClient, conversationId);
}

export function mediaIds(message: TimelineMessage): string[] {
  return message.parts.flatMap((part: MessagePartView) => (part.type === "media" ? [part.asset.asset_id] : []));
}

export async function addUserMessage(services: ConversationServices, text: string): Promise<void> {
  trimIfStuck(services);
  const { api, queryClient, conversationId } = services;
  await api.call("conversationAddUserMessage", {
    conversation_id: conversationId,
    text,
    expected_revision: currentRevision(queryClient, conversationId),
    client_operation_id: crypto.randomUUID(),
  });
  await queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) });
}

export async function setMemberMuted(services: ConversationServices, participantId: string, muted: boolean): Promise<void> {
  const { api, queryClient, conversationId } = services;
  await api.call("conversationParticipantUpdate", {
    conversation_id: conversationId,
    participant_id: participantId,
    enabled: null,
    muted,
    model: null,
    client_operation_id: crypto.randomUUID(),
  });
  await queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) });
}

export async function addSystemMessage(services: ConversationServices, text: string, media: string[]): Promise<void> {
  trimIfStuck(services);
  const { api, queryClient, conversationId } = services;
  await api.call("conversationAddSystemMessage", {
    conversation_id: conversationId,
    text,
    media,
    expected_revision: currentRevision(queryClient, conversationId),
    client_operation_id: crypto.randomUUID(),
  });
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.detail(conversationId) }),
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() }),
  ]);
}

export async function editMessage(services: ConversationServices, message: TimelineMessage, text: string): Promise<void> {
  trimIfStuck(services);
  const { api, queryClient, conversationId } = services;
  await api.call("messageEdit", {
    conversation_id: conversationId,
    message_id: message.id,
    expected_revision: currentRevision(queryClient, conversationId),
    text,
    keep_media: mediaIds(message),
    client_operation_id: crypto.randomUUID(),
  });
}

export async function pinMessage(services: ConversationServices, message: TimelineMessage, pinned: boolean): Promise<void> {
  trimIfStuck(services);
  const { api, queryClient, conversationId } = services;
  await api.call("messagePin", {
    conversation_id: conversationId,
    message_id: message.id,
    expected_revision: currentRevision(queryClient, conversationId),
    pinned,
    client_operation_id: crypto.randomUUID(),
  });
}

export async function deleteMessage(services: ConversationServices, message: TimelineMessage): Promise<void> {
  trimIfStuck(services);
  const { api, queryClient, conversationId } = services;
  await api.call("messageDelete", {
    conversation_id: conversationId,
    message_id: message.id,
    expected_revision: currentRevision(queryClient, conversationId),
    client_operation_id: crypto.randomUUID(),
  });
}

export async function rewindTo(services: ConversationServices, message: TimelineMessage): Promise<void> {
  trimIfStuck(services);
  const { api, queryClient, conversationId } = services;
  await api.call("messagesDeleteAfter", {
    conversation_id: conversationId,
    message_id: message.id,
    expected_revision: currentRevision(queryClient, conversationId),
    client_operation_id: crypto.randomUUID(),
  });
}

export async function branchToNewChat(services: ConversationServices, message: TimelineMessage, characterId: string): Promise<string> {
  trimIfStuck(services);
  const { api, queryClient } = services;
  const copy = await api.call("conversationBranchDirectToCharacter", {
    conversation_id: services.conversationId,
    message_id: message.id,
    character_id: characterId,
    client_operation_id: crypto.randomUUID(),
  });
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.home.characters() }),
  ]);
  return copy.conversation_id;
}

export async function branchGroupToCharacter(services: ConversationServices, message: TimelineMessage, characterId: string): Promise<string> {
  trimIfStuck(services);
  const { api, queryClient } = services;
  const copy = await api.call("conversationBranchToCharacterFromMessage", {
    conversation_id: services.conversationId,
    message_id: message.id,
    character_id: characterId,
    client_operation_id: crypto.randomUUID(),
  });
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.home.characters() }),
  ]);
  return copy.conversation_id;
}

function patchMessage(queryClient: QueryClient, conversationId: string, message: TimelineMessage, revision?: number): void {
  queryClient.setQueryData<InfiniteData<TimelinePage, string | null>>(queryKeys.conversations.messages(conversationId), (data) =>
    data && {
      ...data,
      pages: data.pages.map((entry, index) => ({
        view: index === 0 && entry.view && revision !== undefined ? { ...entry.view, revision } : entry.view,
        page: { ...entry.page, items: entry.page.items.map((item) => (item.id === message.id ? message : item)) },
      })),
    },
  );
}

/** Shows the variant `index` of a reply at once and confirms it with the backend; resolves false when there is no such variant. */
export async function selectCandidate(services: ConversationServices, message: TimelineMessage, index: number): Promise<boolean> {
  const { api, queryClient, conversationId } = services;
  const candidates = await queryClient.ensureQueryData(candidatesQuery(api, conversationId, message.id, message.candidate_count));
  const target = candidates.find((candidate) => candidate.index === index);
  if (!target) return false;
  const expected = currentRevision(queryClient, conversationId);
  patchMessage(queryClient, conversationId, { ...message, parts: target.parts, reasoning: target.reasoning, candidate_index: index });
  try {
    const changed = await api.call("messageCandidateSelect", {
      conversation_id: conversationId,
      message_id: message.id,
      expected_revision: expected,
      candidate_id: target.id,
      client_operation_id: crypto.randomUUID(),
    });
    patchMessage(queryClient, conversationId, changed.message, changed.revision);
  } catch (failure) {
    patchMessage(queryClient, conversationId, message);
    throw failure;
  }
  return true;
}

export async function selectScene(services: ConversationServices, message: TimelineMessage, sceneId: string): Promise<void> {
  const { api, queryClient, conversationId } = services;
  const changed = await api.call("messageSceneSelect", {
    conversation_id: conversationId,
    message_id: message.id,
    expected_revision: currentRevision(queryClient, conversationId),
    scene_id: sceneId,
    client_operation_id: crypto.randomUUID(),
  });
  patchMessage(queryClient, conversationId, changed.message, changed.revision);
  await queryClient.invalidateQueries({ queryKey: settingsKey(conversationId) });
}

export function settingsPatch(change: Partial<ConversationSettingsPatch_Deserialize>): ConversationSettingsPatch_Deserialize {
  return {
    persona: null, prompt: null, roleplay_prompt: null, lorebooks: null, model: null, background: null, scene: null, author_note: null,
    speaker_selection: null, memory: null, chat_mode: null, disable_character_lorebooks: null, ...change,
  };
}

/** Saves a settings change against the known revision; a conflict refetches the settings and tries once more. */
export async function updateSettings(services: ConversationServices, change: Partial<ConversationSettingsPatch_Deserialize>): Promise<ConversationSettingsView> {
  const { api, queryClient, conversationId } = services;
  const send = (revision: number | null) =>
    api.call("conversationSettingsUpdate", { conversation_id: conversationId, expected_settings_revision: revision, patch: settingsPatch(change) });
  let view: ConversationSettingsView;
  try {
    view = await send(queryClient.getQueryData<ConversationSettingsView>(settingsKey(conversationId))?.settings_revision ?? null);
  } catch (error) {
    if (!isApiFailure(error) || error.code !== "conflict") throw error;
    const fresh = await queryClient.fetchQuery({ ...settingsQuery(api, conversationId), staleTime: 0 });
    view = await send(fresh.settings_revision);
  }
  queryClient.setQueryData(settingsKey(conversationId), view);
  if ("persona" in change || "model" in change || "background" in change || "memory" in change) {
    trimIfStuck(services);
    await Promise.all([
      queryClient.invalidateQueries({ queryKey: queryKeys.conversations.messages(conversationId) }),
      queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() }),
    ]);
  }
  return view;
}

/** Lets the person pick an image and makes it this chat's background; resolves false when the picker was cancelled. */
export async function pickBackground(services: ConversationServices): Promise<boolean> {
  const picked = await services.api.call("filesPickOpen", { kinds: ["image"], multiple: false });
  const source = picked.sources[0];
  if (!source) return false;
  const asset = await services.api.call("assetsIngest", { source, role: "background" });
  await updateSettings(services, { background: { type: "image", asset_id: asset.asset_id } });
  return true;
}

export async function startGroupChat(api: ApiClient, queryClient: QueryClient, groupId: string): Promise<string> {
  const launched = await api.call("conversationLaunchGroup", { group_id: groupId, client_operation_id: crypto.randomUUID() });
  await queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() });
  return launched.conversation_id;
}

export async function startDirectChat(api: ApiClient, queryClient: QueryClient, characterId: string, starterId: string | null = null): Promise<string> {
  const launched = await api.call("conversationLaunchDirect", {
    character_id: characterId,
    title: null,
    scene_id: null,
    starter_id: starterId,
    client_operation_id: crypto.randomUUID(),
  });
  await Promise.all([
    queryClient.invalidateQueries({ queryKey: queryKeys.conversations.lists() }),
    queryClient.invalidateQueries({ queryKey: queryKeys.home.characters() }),
  ]);
  return launched.conversation_id;
}
