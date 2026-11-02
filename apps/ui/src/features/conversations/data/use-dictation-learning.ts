import { useCallback, useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { ApiFailure, type ApiClient } from "@/api/client";
import type { AsrSuggestionView, TranscriptionView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";

const editDelayMs = 250;

interface TranscriptSource {
  conversationId: string;
  before: string;
  base: string;
  language: string | null;
}

interface Suggestions {
  source: TranscriptSource;
  draft: string;
  items: AsrSuggestionView[];
  error: ApiFailure | null;
}

export const correctionKey = (suggestion: AsrSuggestionView) => JSON.stringify([
  suggestion.wrong, suggestion.correct, suggestion.language, suggestion.scope,
]);

export function useDictationLearning(api: ApiClient, conversationId: string, draft: string) {
  const client = useQueryClient();
  const [source, setSource] = useState<TranscriptSource | null>(null);
  const current = useRef<TranscriptSource | null>(null);
  const [result, setResult] = useState<Suggestions | null>(null);
  const [writeError, setWriteError] = useState<{ source: TranscriptSource; draft: string; error: ApiFailure } | null>(null);
  const [retry, setRetry] = useState(0);
  const [busy, setBusy] = useState(false);
  const writing = useRef(false);
  const operations = useRef(new Map<string, string>());
  const resolved = useRef(new Set<string>());
  const live = useRef(true);

  useEffect(() => {
    live.current = true;
    return () => {
      live.current = false;
      current.current = null;
    };
  }, []);

  const clear = useCallback((expected?: TranscriptSource | null) => {
    if (expected !== undefined && current.current !== expected) return;
    current.current = null;
    setSource(null);
    setResult(null);
    setWriteError(null);
    operations.current.clear();
    resolved.current.clear();
  }, []);

  const begin = useCallback((text: string, transcription: TranscriptionView) => {
    clear();
    const next = { conversationId, before: transcription.raw_text.trim() || text, base: text.trim(), language: transcription.detected_language };
    current.current = next;
    setSource(next);
  }, [clear, conversationId]);

  useEffect(() => {
    if (!source || source.conversationId !== conversationId || !draft.trim() || draft.trim() === source.base) return;
    let active = true;
    const timer = window.setTimeout(() => {
      void api.call("asrSuggestions", { before: source.before, after: draft, language: source.language, scope: "conversation" }).then(
        (items) => {
          if (active && current.current === source) setResult({ source, draft, items: items.filter((item) => !resolved.current.has(correctionKey(item))), error: null });
        },
        (error: unknown) => {
          if (active && current.current === source) setResult({ source, draft, items: [], error: ApiFailure.fromRejection(error) });
        },
      );
    }, editDelayMs);
    return () => {
      active = false;
      window.clearTimeout(timer);
    };
  }, [api, conversationId, draft, retry, source]);

  const write = async (suggestion: AsrSuggestionView, action: "approve" | "ignore") => {
    const owner = current.current;
    const key = correctionKey(suggestion);
    if (!owner || owner.conversationId !== conversationId || writing.current || resolved.current.has(key)) return;
    writing.current = true;
    setBusy(true);
    setWriteError(null);
    const operation = JSON.stringify([action, suggestion]);
    let operationId = operations.current.get(operation);
    if (!operationId) {
      operationId = crypto.randomUUID();
      operations.current.set(operation, operationId);
    }
    try {
      const request = { client_operation_id: operationId, suggestion };
      if (action === "approve") await api.call("asrSuggestionApprove", request);
      else await api.call("asrSuggestionIgnore", request);
      if (live.current && current.current === owner) {
        resolved.current.add(key);
        setResult((previous) => previous && { ...previous, items: previous.items.filter((item) => correctionKey(item) !== key) });
      }
      void client.invalidateQueries({ queryKey: queryKeys.speech.all });
    } catch (error) {
      if (live.current && current.current === owner) setWriteError({ source: owner, draft, error: ApiFailure.fromRejection(error) });
    } finally {
      writing.current = false;
      if (live.current) setBusy(false);
    }
  };

  const visible = source?.conversationId === conversationId && draft.trim() && draft.trim() !== source.base;
  const matching = visible && result?.source === source && result.draft === draft ? result : null;
  return {
    source,
    begin,
    clear,
    busy,
    suggestions: matching?.items ?? [],
    error: visible && writeError?.source === source && writeError.draft === draft ? writeError.error : matching?.error ?? null,
    retry: () => setRetry((value) => value + 1),
    approve: (suggestion: AsrSuggestionView) => write(suggestion, "approve"),
    ignore: (suggestion: AsrSuggestionView) => write(suggestion, "ignore"),
  };
}
