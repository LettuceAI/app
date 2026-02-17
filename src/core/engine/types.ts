// ── Health ──────────────────────────────────────────────────────────────────

export type HealthResponse = {
  status: string;
  version?: string;
};

// ── Setup ───────────────────────────────────────────────────────────────────

export type SetupStatusResponse = {
  needs_setup: boolean;
  configured_providers: string[];
  has_api_key?: boolean;
};

// ── Config ──────────────────────────────────────────────────────────────────

export type ConfigLlmRequest = {
  model: string;
  api_key?: string;
  base_url?: string;
  max_tokens?: number;
  temperature?: number;
};

export type ConfigLlmDefaultRequest = {
  provider: string;
};

export type ConfigEngineRequest = {
  data_dir?: string;
  log_level?: string;
  max_history?: number;
};

export type ConfigBackgroundRequest = {
  synthesis_interval_minutes?: number;
  consolidation_interval_minutes?: number;
  bm25_rebuild_interval_minutes?: number;
  drip_research_interval_minutes?: number;
};

export type ConfigMemoryRequest = {
  embedding_model?: string;
  max_retrieval_results?: number;
  dense_weight?: number;
  bm25_weight?: number;
  graph_weight?: number;
  recency_boost_hours?: number;
  random_surface_probability?: number;
};

export type ConfigSafetyRequest = {
  honesty_section?: boolean;
  user_data_deletion?: boolean;
};

export type ConfigResearchRequest = {
  initial_scrape_on_boot?: boolean;
  periodic_interval_hours?: number;
};

// ── Characters ──────────────────────────────────────────────────────────────

export type CharacterSummary = {
  slug: string;
  name: string;
  role?: string;
  era?: string;
  loaded: boolean;
  memory_count?: number;
  turn_count?: number;
  primary_emotion?: string;
};

// ── Usage ───────────────────────────────────────────────────────────────────

export type UsageByModel = {
  model: string;
  backend: string;
  calls: number;
  input_tokens: number;
  output_tokens: number;
};

export type UsageBySource = {
  source: string;
  calls: number;
  input_tokens: number;
  output_tokens: number;
};

export type CharacterUsage = {
  character: string;
  total_calls: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_tokens: number;
  by_model: UsageByModel[];
  by_source: UsageBySource[];
};

export type UsageResponse = {
  characters: CharacterUsage[];
  total_calls: number;
  total_input_tokens: number;
  total_output_tokens: number;
  total_tokens: number;
};

// ── Activity ────────────────────────────────────────────────────────────────

export type LoopStatus = {
  last_run: string | null;
  interval_minutes: number;
  status: "running" | "stopped";
};

export type CharacterActivity = {
  loops_running: boolean;
  synthesis: LoopStatus;
  consolidation: LoopStatus;
  bm25_rebuild: LoopStatus;
  drip_research: LoopStatus;
};

// ── Character Creation ──────────────────────────────────────────────────────

export type SpeechPatterns = {
  formality?: "formal" | "casual" | "texting";
  verbosity?: "terse" | "medium" | "verbose";
  text_style?: "formal" | "casual" | "texting";
  dialect?: string;
  catchphrases?: string[];
  vocabulary_preferences?: string[];
  vocabulary_avoidances?: string[];
  filler_words?: string[];
  example_quotes?: string[];
};

export type TimeBehaviors = {
  early_morning?: string;
  morning?: string;
  afternoon?: string;
  evening?: string;
  night?: string;
};

export type BaselineEmotions = {
  joy?: number;
  trust?: number;
  fear?: number;
  surprise?: number;
  sadness?: number;
  disgust?: number;
  anger?: number;
  anticipation?: number;
};

export type CharacterCreateRequest = {
  name: string;
  era?: string;
  setting?: string;
  role?: string;
  core_identity?: string;
  backstory?: string;
  personality_traits?: string[];
  speech_patterns?: SpeechPatterns;
  knowledge_domains?: string[];
  knowledge_boundaries?: string[];
  research_seeds?: string[];
  research_enabled?: boolean;
  physical_description?: string;
  physical_habits?: string[];
  idle_behaviors?: string[];
  time_behaviors?: TimeBehaviors;
  baseline_emotions?: BaselineEmotions;
  backend?: string;
  model?: string;
  temperature?: number;
  is_seed?: boolean;
  seed_prompt?: string;
};

export type CharacterBoostRequest = {
  name?: string;
  seed: string;
  era?: string;
};

// ── Chat ────────────────────────────────────────────────────────────────────

export type EngineChatResponse = {
  response: string;
  character: string;
  emotion?: string;
  emotion_intensity?: number;
};

export type EngineHistoryMessage = {
  id: string;
  user_id: string;
  user_name?: string;
  role: "user" | "assistant";
  content: string;
  timestamp?: string;
  entities_mentioned?: string[];
};

// ── Engine LLM Providers ────────────────────────────────────────────────────

export const ENGINE_LLM_PROVIDERS = [
  { id: "anthropic", name: "Anthropic", requiresKey: true },
  { id: "openai", name: "OpenAI", requiresKey: true },
  { id: "openrouter", name: "OpenRouter", requiresKey: true },
  { id: "ollama", name: "Ollama", requiresKey: false },
] as const;

export type EngineLlmProviderId = (typeof ENGINE_LLM_PROVIDERS)[number]["id"];
