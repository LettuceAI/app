import { queryOptions } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { queryKeys } from "@/api/query-keys";

export function companionStateQuery(api: ApiClient, conversationId: string) {
  return queryOptions({
    queryKey: queryKeys.companions.state(conversationId),
    queryFn: () => api.call("companionStateGet", { conversation_id: conversationId }),
  });
}

export function groupCompanionStateQuery(api: ApiClient, conversationId: string) {
  return queryOptions({
    queryKey: queryKeys.companions.groupState(conversationId),
    queryFn: () => api.call("groupCompanionStateGet", { conversation_id: conversationId }),
  });
}

export function groupCompanionEffectsQuery(api: ApiClient, conversationId: string, messageId: string) {
  return queryOptions({
    queryKey: queryKeys.companions.groupEffects(conversationId, messageId),
    queryFn: () => api.call("groupCompanionMessageEffects", { conversation_id: conversationId, message_id: messageId }),
  });
}

export function groupMessageAudienceQuery(api: ApiClient, conversationId: string, messageId: string) {
  return queryOptions({
    queryKey: queryKeys.companions.groupAudience(conversationId, messageId),
    queryFn: () => api.call("groupMessageAudienceGet", { conversation_id: conversationId, message_id: messageId }),
  });
}

export function companionSoulQuery(api: ApiClient, conversationId: string, characterId: string | null) {
  return queryOptions({
    queryKey: queryKeys.companions.soul(conversationId, characterId ?? ""),
    queryFn: () => api.call("companionSoulGet", { character_id: characterId ?? "", conversation_id: conversationId }),
    enabled: characterId !== null,
  });
}

export function companionEffectQuery(api: ApiClient, conversationId: string, messageId: string) {
  return queryOptions({
    queryKey: queryKeys.companions.effect(conversationId, messageId),
    queryFn: () => api.call("messageCompanionEffect", { message_id: messageId }),
  });
}

export { emotionStatusQuery } from "@/entities/download";
