import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import type { ApiClient } from "@/api/client";
import type { MicrophoneDevices, AsrLearningFilter, AsrSuggestionView, AsrCorrectionView, AsrIgnoredSuggestionView } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { whisperLanguages } from "./languages";

export const allScopes = ["global", "project", "character", "conversation"];
export const libraryFilter: AsrLearningFilter = { language: null, scopes: allScopes, user_approved_only: null };
export const languageName = (code: string | null) => whisperLanguages.find((language) => language.code === code)?.name ?? code;
export const modelName = (id: string) => id.replace(/^ggml-/, "").replace(/\.bin$/, "");
export const runOptions = (gpu: boolean) => ({ use_gpu: gpu, force_cpu: !gpu, flash_attention: true, gpu_device: 0 });
export const asSuggestion = (value: AsrCorrectionView | AsrIgnoredSuggestionView): AsrSuggestionView => ({ wrong: value.wrong, correct: value.correct, language: value.language, scope: value.scope, confidence: "confidence" in value ? value.confidence : null, accepted_count: "accepted_count" in value ? value.accepted_count : 0, rejected_count: "rejected_count" in value ? value.rejected_count : value.ignored_count, seen_count: "seen_count" in value ? value.seen_count : value.ignored_count });

export function useSpeechData(api: ApiClient, scope: string) {
  const filter = { ...libraryFilter, scopes: scope === "all" ? allScopes : [scope] };
  const models = useQuery({ queryKey: [...queryKeys.speech.all, "models"], queryFn: () => api.call("whisperModelsList") });
  const words = useQuery({ queryKey: [...queryKeys.speech.all, "words", filter], queryFn: () => api.call("asrVocabularyList", filter) });
  const corrections = useQuery({ queryKey: [...queryKeys.speech.all, "corrections", filter], queryFn: () => api.call("asrCorrectionsList", filter) });
  const ignored = useQuery({ queryKey: [...queryKeys.speech.all, "ignored", filter], queryFn: () => api.call("asrIgnoredSuggestionsList", filter) });
  const examples = useQuery({ queryKey: [...queryKeys.speech.all, "examples", filter], queryFn: () => api.call("asrVoiceExamplesList", filter) });
  return { models, words, corrections, ignored, examples };
}

export function useSpeechAction() {
  const client = useQueryClient();
  return useMutation({ mutationFn: (run: () => Promise<unknown>) => run(), onSuccess: async () => {
    await Promise.all([queryKeys.speech.all, queryKeys.app.status(), queryKeys.downloads.all, queryKeys.media.all].map((queryKey) => client.invalidateQueries({ queryKey })));
  } });
}

export const recordingTime = (ms: number) => `${Math.floor(ms / 60000)}:${String(Math.floor(ms / 1000) % 60).padStart(2, "0")}`;
export function microphoneOptions(devices: MicrophoneDevices | undefined, selected: string | null, defaultLabel: string, unavailableLabel: string, ambiguousLabel: (name: string) => string) {
  const options = [{ value: "default", label: defaultLabel, disabled: false }, ...(devices?.devices ?? []).map((device) => ({ value: device.id, label: device.selectable ? device.label : ambiguousLabel(device.label), disabled: !device.selectable }))];
  if (selected && !options.some((item) => item.value === selected)) options.push({ value: selected, label: unavailableLabel, disabled: true });
  return options;
}
