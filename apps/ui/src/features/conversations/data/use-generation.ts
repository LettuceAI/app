import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient, type InfiniteData, type QueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import { ApiFailure } from "@/api/client";
import type {
  AppStatus,
  GenerationEvent,
  RuntimeNoticeCode,
  SettingsUiAccessibilitySettingsHapticIntensity,
} from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { GenerationStore, type GenerationFailure } from "./generation-store";
import type { TimelinePage } from "./queries";
import { object, playSound, usePreferences } from "@/entities/preferences";
import { typingHaptic } from "@/api/haptics";

export interface GenerationHooks {
  onError: (failure: ApiFailure) => void;
  onNotice: (code: RuntimeNoticeCode) => void;
  isStuck: () => boolean;
}

interface Accepted {
  turn_id: string;
  user_message_id?: string;
}

interface Run {
  settled: boolean;
  stopRequested: boolean;
  controller: AbortController;
}

function untilShown(
  queryClient: QueryClient,
  conversationId: string,
  messageId: string,
  since: number,
): Promise<void> {
  const key = queryKeys.conversations.messages(conversationId);
  const shown = () => {
    const state = queryClient.getQueryState<InfiniteData<TimelinePage, string | null>>(key);
    if (!state?.data || state.status === "error") return true;
    return (
      state.dataUpdatedAt >= since &&
      state.data.pages.some((entry) => entry.page.items.some((message) => message.id === messageId))
    );
  };
  if (shown()) return Promise.resolve();
  return new Promise((resolve) => {
    const unsubscribe = queryClient.getQueryCache().subscribe(() => {
      if (!shown()) return;
      unsubscribe();
      resolve();
    });
  });
}

function requestStop(run: Run) {
  run.stopRequested = true;
}

export function currentRevision(queryClient: QueryClient, conversationId: string): number {
  const data = queryClient.getQueryData<InfiniteData<TimelinePage, string | null>>(
    queryKeys.conversations.messages(conversationId),
  );
  return data?.pages[0]?.view?.revision ?? 0;
}

/** Drops the cached older pages so the next refetch walks only the newest one. */
export function trimTimeline(queryClient: QueryClient, conversationId: string): void {
  queryClient.setQueryData<InfiniteData<TimelinePage, string | null>>(
    queryKeys.conversations.messages(conversationId),
    (data) =>
      data && data.pages.length > 1
        ? { pages: data.pages.slice(0, 1), pageParams: data.pageParams.slice(0, 1) }
        : data,
  );
}

export function useGeneration(api: ApiClient, conversationId: string, hooks: GenerationHooks) {
  const preferences = usePreferences();
  const preferencesRef = useRef(preferences.value);
  useEffect(() => {
    preferencesRef.current = preferences.value;
  }, [preferences.value]);
  const queryClient = useQueryClient();
  const [store] = useState(() => new GenerationStore());
  const hooksRef = useRef(hooks);
  const runs = useRef(new Set<Run>());

  useEffect(() => {
    hooksRef.current = hooks;
  });

  useEffect(() => {
    const live = runs.current;
    return () => {
      for (const run of live) run.controller.abort();
      live.clear();
    };
  }, []);

  const cancelTurn = useCallback(
    async (turnId: string) => {
      try {
        await api.call("generationCancel", { turn_id: turnId });
      } catch (error) {
        hooksRef.current.onError(ApiFailure.fromRejection(error));
      }
    },
    [api],
  );

  const run = useCallback(
    async (
      targetId: string | null,
      userText: string | null,
      restore: GenerationFailure | null,
      start: (onEvent: (event: GenerationEvent) => void, signal: AbortSignal) => Promise<Accepted>,
    ): Promise<boolean> => {
      if (store.getMeta().active) return false;
      const current: Run = {
        settled: false,
        stopRequested: false,
        controller: new AbortController(),
      };
      let lastHaptic = -Infinity;
      let feedbackFailed = false;
      const feedbackError = (error: unknown) => {
        if (feedbackFailed) return;
        feedbackFailed = true;
        hooksRef.current.onError(ApiFailure.fromRejection(error));
      };
      runs.current.add(current);
      if (hooksRef.current.isStuck()) trimTimeline(queryClient, conversationId);
      store.begin(targetId, userText);

      const finish = () => {
        current.controller.abort();
        runs.current.delete(current);
      };

      const learnTurn = (turnId: string) => {
        store.setTurn(turnId);
        if (current.stopRequested) {
          current.stopRequested = false;
          void cancelTurn(turnId);
        }
      };

      const settle = async (event: GenerationEvent) => {
        if (current.settled) return;
        current.settled = true;
        void playSound(
          event.type === "completed" ? "success" : "failure",
          preferencesRef.current,
        ).catch(feedbackError);
        const hadText = store.getText().text.length > 0;
        const refetch = () =>
          queryClient.invalidateQueries({
            queryKey: queryKeys.conversations.detail(conversationId),
          });
        if (event.type === "failed") {
          const failure = {
            code: event.code,
            turnId: event.turn_id,
            targetId: store.getMeta().targetId,
          };
          await refetch();
          store.fail(failure);
        } else if (event.type === "cancelled" && !hadText) {
          store.clear();
          await refetch();
        } else if (event.type === "completed") {
          const since = Date.now();
          await refetch();
          await untilShown(queryClient, conversationId, event.message_id, since);
          store.clear();
        } else {
          await refetch();
          store.clear();
        }
        finish();
      };

      const onEvent = (event: GenerationEvent) => {
        if (current.settled) return;
        switch (event.type) {
          case "started":
            learnTurn(event.turn_id);
            break;
          case "model_loading":
            store.setLoading({
              model: event.model_name,
              percent: event.percent,
              status: event.status,
            });
            break;
          case "notice":
            hooksRef.current.onNotice(event.code);
            break;
          case "delta": {
            const previousLength = store.getText().text.length;
            store.appendDelta(event.text, event.reasoning);
            if (store.getText().text.length > previousLength && !feedbackFailed) {
              const accessibility = object(preferencesRef.current.accessibility);
              const platform = queryClient.getQueryData<AppStatus>(
                queryKeys.app.status(),
              )?.platform;
              const now = performance.now();
              if (
                accessibility.haptics === true &&
                (platform === "android" || platform === "ios") &&
                now - lastHaptic >= 60
              ) {
                lastHaptic = now;
                const intensity = (accessibility.hapticIntensity ??
                  "light") as SettingsUiAccessibilitySettingsHapticIntensity;
                void typingHaptic(intensity, platform).catch(feedbackError);
              }
            }
            break;
          }
          case "completed":
          case "failed":
          case "cancelled":
            void settle(event);
            break;
          default:
            break;
        }
      };

      try {
        const accepted = await start(onEvent, current.controller.signal);
        if (accepted.user_message_id)
          void playSound("send", preferencesRef.current).catch(feedbackError);
        if (!current.settled) {
          learnTurn(accepted.turn_id);
          if (accepted.user_message_id) store.setUserMessage(accepted.user_message_id);
        }
        return true;
      } catch (error) {
        const failure = ApiFailure.fromRejection(error);
        void playSound("failure", preferencesRef.current).catch(feedbackError);
        current.settled = true;
        finish();
        if (restore) store.fail(restore);
        else store.clear();
        if (failure.code === "conflict") {
          await queryClient.invalidateQueries({
            queryKey: queryKeys.conversations.detail(conversationId),
          });
        }
        hooksRef.current.onError(failure);
        return false;
      }
    },
    [store, queryClient, conversationId, cancelTurn],
  );

  const send = useCallback(
    (text: string, media: string[], swapPlaces: boolean) =>
      run(null, text, null, (onEvent, signal) =>
        api.stream(
          "conversationSend",
          [
            {
              conversation_id: conversationId,
              expected_revision: currentRevision(queryClient, conversationId),
              text,
              media,
              swap_places: swapPlaces,
              client_operation_id: crypto.randomUUID(),
            },
          ],
          onEvent,
          { signal },
        ),
      ),
    [api, conversationId, queryClient, run],
  );

  const continueReply = useCallback(
    (swapPlaces: boolean) =>
      run(null, null, null, (onEvent, signal) =>
        api.stream(
          "conversationContinue",
          [
            {
              conversation_id: conversationId,
              expected_revision: currentRevision(queryClient, conversationId),
              client_operation_id: crypto.randomUUID(),
              forced_speaker_participant_id: null,
              swap_places: swapPlaces,
            },
          ],
          onEvent,
          { signal },
        ),
      ),
    [api, conversationId, queryClient, run],
  );

  const regenerate = useCallback(
    (messageId: string, swapPlaces: boolean) =>
      run(messageId, null, null, (onEvent, signal) =>
        api.stream(
          "conversationRegenerate",
          [
            {
              conversation_id: conversationId,
              message_id: messageId,
              expected_revision: currentRevision(queryClient, conversationId),
              client_operation_id: crypto.randomUUID(),
              guidance: null,
              model_profile_id: null,
              forced_speaker_participant_id: null,
              swap_places: swapPlaces,
            },
          ],
          onEvent,
          { signal },
        ),
      ),
    [api, conversationId, queryClient, run],
  );

  const retry = useCallback(() => {
    const failure = store.getMeta().failure;
    if (!failure) return Promise.resolve(false);
    return run(failure.targetId, null, failure, (onEvent, signal) =>
      api.stream(
        "conversationRetry",
        [
          {
            conversation_id: conversationId,
            turn_id: failure.turnId,
            client_operation_id: crypto.randomUUID(),
          },
        ],
        onEvent,
        { signal },
      ),
    );
  }, [api, conversationId, run, store]);

  const stop = useCallback(
    (pendingTurnId: string | null) => {
      const meta = store.getMeta();
      if (meta.active) {
        if (meta.turnId) void cancelTurn(meta.turnId);
        else runs.current.forEach(requestStop);
        return;
      }
      if (pendingTurnId) void cancelTurn(pendingTurnId);
    },
    [store, cancelTurn],
  );

  return { store, send, continueReply, regenerate, retry, stop };
}
