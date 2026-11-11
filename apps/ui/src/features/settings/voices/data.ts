import { useCallback, useEffect, useRef, useState } from "react";
import { queryOptions, useQueries, useQueryClient, type QueryKey } from "@tanstack/react-query";
import { useLocation, useNavigate, useRouter } from "@tanstack/react-router";
import type { ApiClient } from "@/api/client";
import { ApiFailure } from "@/api/client";
import type { AssetRef, AudioProviderType, AudioProviderView, AudioVoiceView, KokoroInventory, TtsSynthesizeRequest_Deserialize } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { watchJob } from "@/entities/job";

export const speedRange = { min: 0.5, max: 2, step: 0.05 } as const;
export const providerVoiceLimit = 10;

export const audioProvidersQuery = (api: ApiClient) => queryOptions({ queryKey: queryKeys.audioProviders.all, queryFn: () => api.call("audioProvidersList") });
export const userVoicesQuery = (api: ApiClient) => queryOptions({ queryKey: queryKeys.voices.user(), queryFn: () => api.call("userVoicesList") });
export const cacheStatsQuery = (api: ApiClient) => queryOptions({ queryKey: queryKeys.voices.cache(), queryFn: () => api.call("ttsCacheStats") });
export const providerVoicesQuery = (api: ApiClient, providerId: string) => queryOptions({ queryKey: queryKeys.voices.providerVoices(providerId), queryFn: () => api.call("audioProviderVoices", { provider_id: providerId }) });
export const ttsModelsQuery = (api: ApiClient, providerType: AudioProviderType, design: boolean) => queryOptions({ queryKey: queryKeys.voices.models(providerType, design), queryFn: () => api.call(design ? "ttsVoiceDesignModels" : "ttsModels", { provider_type: providerType }) });
export const kokoroVariantsQuery = (api: ApiClient) => queryOptions({ queryKey: queryKeys.voices.kokoroVariants(), queryFn: () => api.call("kokoroVariants") });
export const kokoroInventoryQuery = (api: ApiClient, variant: string | null) => queryOptions({ queryKey: queryKeys.voices.kokoroInventory(variant ?? ""), queryFn: () => api.call("kokoroInventory", { variant: variant ?? "", selected_voice_id: null }), enabled: variant !== null });
export const kokoroAvailableQuery = (api: ApiClient, enabled: boolean) => queryOptions({ queryKey: queryKeys.voices.kokoroAvailable(), queryFn: () => api.call("kokoroVoicesAvailable"), enabled });

export const isKokoro = (provider: AudioProviderView) => provider.configuration.type === "kokoro";
export const kokoroVariantOf = (provider: AudioProviderView) => provider.configuration.type === "kokoro" ? provider.configuration.variant : null;
export const hasProviderVoices = (provider: AudioProviderView) => ["elevenlabs", "fish_tts", "gemini"].includes(provider.configuration.type);

/** Whether Kokoro has every file of its engine for the variant; voices are separate. */
export const kokoroInstalled = (inventory: KokoroInventory | undefined) => !!inventory?.model && !!inventory.config && !!inventory.tokenizer && !!inventory.tokenizer_config;

/** An ElevenLabs or Fish Audio voice made by the user (cloned, designed) rather than one the provider ships. */
export const isCustomVoice = (voice: AudioVoiceView) => {
  const category = (voice.labels.category ?? "").toLowerCase();
  return category !== "" && category !== "premade" && category !== "library";
};

/** The id after the language prefix with its first letter upper-case, as the Kokoro voices are named. */
export function kokoroVoiceName(id: string) {
  const name = id.includes("_") ? id.split("_").slice(1).join("_") : id;
  return name ? name.charAt(0).toUpperCase() + name.slice(1) : id;
}

/** Kokoro voice ids start with a language letter and a gender letter, like `af_heart` (American English, female). */
export const kokoroLanguages = ["a", "b", "e", "f", "h", "i", "j", "p", "z"] as const;
export type KokoroLanguage = (typeof kokoroLanguages)[number] | "other";
export const kokoroLanguageOf = (id: string): KokoroLanguage => (kokoroLanguages as readonly string[]).includes(id.charAt(0)) && id.charAt(2) === "_" ? (id.charAt(0) as KokoroLanguage) : "other";
const kokoroLanguageCodes: Record<(typeof kokoroLanguages)[number], string> = { a: "en-US", b: "en-GB", e: "es", f: "fr", h: "hi", i: "it", j: "ja", p: "pt-BR", z: "zh" };
/** The language's own name in the reader's locale, or null for ids outside the known languages. */
export const kokoroLanguageName = (language: KokoroLanguage, locale: string) => language === "other" ? null : new Intl.DisplayNames([locale], { type: "language" }).of(kokoroLanguageCodes[language]) ?? kokoroLanguageCodes[language];
export const kokoroGenderOf = (id: string): "female" | "male" | "other" => id.charAt(2) !== "_" ? "other" : id.charAt(1) === "f" ? "female" : id.charAt(1) === "m" ? "male" : "other";

export interface BlendVoice {
  id: string;
  /** 0 to 100, the voice's weight in the blend before it is stored as a share. */
  weight: number;
}

/** A stored Kokoro voice id: a plain id is one voice at full weight, a JSON list is a weighted blend. */
export function parseBlend(stored: string): BlendVoice[] {
  const value = stored.trim();
  if (!value) return [];
  if (!value.startsWith("[")) return [{ id: value, weight: 100 }];
  try {
    const parsed: unknown = JSON.parse(value);
    if (!Array.isArray(parsed)) return [];
    const seen = new Set<string>();
    const voices: BlendVoice[] = [];
    for (const entry of parsed as { voiceId?: unknown; weight?: unknown }[]) {
      if (typeof entry?.voiceId !== "string" || seen.has(entry.voiceId)) continue;
      seen.add(entry.voiceId);
      voices.push({ id: entry.voiceId, weight: typeof entry.weight === "number" ? Math.round(entry.weight * 100) : 100 });
    }
    return voices;
  } catch {
    return [];
  }
}

export const serializeBlend = (voices: readonly BlendVoice[]) => JSON.stringify(voices.filter((voice) => voice.weight > 0).map((voice) => ({ voiceId: voice.id, weight: voice.weight / 100 })));

export function parseSpeed(prompt: string | null): number {
  if (!prompt) return 1;
  try {
    const speed = (JSON.parse(prompt) as { speed?: unknown } | null)?.speed;
    return typeof speed === "number" && speed > 0 ? Math.min(speedRange.max, Math.max(speedRange.min, speed)) : 1;
  } catch {
    return 1;
  }
}

export const serializeSpeed = (speed: number) => JSON.stringify({ speed });
export const speedText = (speed: number) => `${Number(speed.toFixed(2))}x`;

/** Per-provider voice lists, in the order of the providers given; providers without a listing are skipped. */
export function useProviderVoices(api: ApiClient, providers: readonly AudioProviderView[]) {
  const listed = providers.filter(hasProviderVoices);
  const queries = useQueries({ queries: listed.map((provider) => providerVoicesQuery(api, provider.id)) });
  return listed.map((provider, index) => ({ provider, query: queries[index]! }));
}

export function useInvalidate() {
  const client = useQueryClient();
  return useCallback((...keys: QueryKey[]) => Promise.all(keys.map((queryKey) => client.invalidateQueries({ queryKey }))), [client]);
}

/** Search states of the Voices section; opening one pushes history and closing goes back to where it was opened from. */
export interface VoicesSearch {
  voiceEditor?: string;
  voiceProvider?: string;
  voiceItem?: string;
  kokoro?: string;
  kokoroBlend?: string;
  kokoroBlendVoice?: string;
  kokoroEngine?: true;
}

export function useVoicesNavigation() {
  const navigate = useNavigate();
  const router = useRouter();
  const pushed = useLocation({ select: (location) => location.state.voicesPanelPushed });
  const open = (search: VoicesSearch, replace = false) => { void navigate({ to: "/settings/$section", params: { section: "voices" }, search, state: { voicesPanelPushed: true }, replace, resetScroll: false }); };
  const close = (fallback: VoicesSearch = {}) => {
    if (pushed) router.history.back();
    else void navigate({ to: "/settings/$section", params: { section: "voices" }, search: fallback, replace: true, resetScroll: false });
  };
  return { open, close };
}

/** One client operation id per distinct payload, so a retried save is the same operation and an edited one is a new one. */
export function useOperationId() {
  const operation = useRef({ payload: "", id: "" });
  return useCallback((payload: unknown) => {
    const serialized = JSON.stringify(payload);
    if (operation.current.payload !== serialized || !operation.current.id) operation.current = { payload: serialized, id: crypto.randomUUID() };
    return operation.current.id;
  }, []);
}

export type SynthesisRequest = Omit<TtsSynthesizeRequest_Deserialize, "request_id" | "retained">;

/** Starts one synthesis at a time, follows its job, and keeps the last clip until a new one replaces it. */
export function useSynthesis(api: ApiClient) {
  const [state, setState] = useState<{ busy: boolean; asset: AssetRef | null; error: unknown }>({ busy: false, asset: null, error: null });
  const abort = useRef<AbortController | null>(null);
  useEffect(() => () => abort.current?.abort(), []);
  const run = useCallback(async (request: SynthesisRequest) => {
    abort.current?.abort();
    const controller = new AbortController();
    abort.current = controller;
    setState((previous) => ({ ...previous, busy: true, error: null }));
    try {
      const accepted = await api.call("ttsSynthesize", { ...request, request_id: crypto.randomUUID(), retained: false });
      const job = await watchJob(api, accepted.job_id, () => undefined, controller.signal);
      if (job.state !== "succeeded" || job.result?.type !== "asset") throw new ApiFailure(job.state === "cancelled" ? "cancelled" : "unavailable", "speech synthesis did not produce audio", null);
      if (!controller.signal.aborted) setState({ busy: false, asset: job.result.asset, error: null });
    } catch (error) {
      if (!controller.signal.aborted) setState({ busy: false, asset: null, error });
    }
  }, [api]);
  return { run, busy: state.busy, asset: state.asset, error: state.error };
}
