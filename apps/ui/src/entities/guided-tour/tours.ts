interface TourDefinition {
  storageKey: string;
  steps: readonly { target: string; key: string; desktopOnly?: boolean }[];
}

export const tours = {
  appShell: {
    storageKey: "app_tour_v1",
    steps: [
      { target: "nav-chats", key: "appShell.chats" },
      { target: "nav-groups", key: "appShell.groups" },
      { target: "nav-discover", key: "appShell.discover" },
      { target: "nav-library", key: "appShell.library" },
      { target: "top-settings", key: "appShell.settings" },
      { target: "top-search", key: "appShell.search" },
      { target: "nav-create", key: "appShell.create" },
    ],
  },
  speechRecognition: {
    storageKey: "asr_tour_v1",
    steps: [
      { target: "asr-active-model", key: "speechRecognition.activeModel" },
      { target: "asr-library", key: "speechRecognition.library" },
      { target: "asr-mic-test", key: "speechRecognition.micTest" },
      { target: "asr-runtime", key: "speechRecognition.runtime" },
    ],
  },
  editModelLlama: {
    storageKey: "edit_model_llama_tour_v1",
    steps: [
      { target: "model-runtime-context", key: "editModelLlama.context" },
      { target: "model-runtime-gpu", key: "editModelLlama.gpu" },
      { target: "model-runtime-report", key: "editModelLlama.report" },
    ],
  },
  runtimeDefaults: {
    storageKey: "runtime_defaults_tour_v1",
    steps: [
      { target: "runtime-defaults-storage", key: "runtimeDefaults.storage" },
      { target: "runtime-defaults-llama", key: "runtimeDefaults.llama" },
      { target: "runtime-defaults-multigpu", key: "runtimeDefaults.multiGpu" },
      { target: "runtime-defaults-civitai", key: "runtimeDefaults.civitai" },
    ],
  },
  hfBrowser: {
    storageKey: "hf_browser_tour_v1",
    steps: [
      { target: "hf-rec-panel", key: "hfBrowser.panel" },
      { target: "hf-rec-quant", key: "hfBrowser.quant" },
      { target: "hf-rec-context", key: "hfBrowser.context" },
      { target: "hf-rec-offload", key: "hfBrowser.offload" },
      { target: "hf-rec-kv-location", key: "hfBrowser.kvLocation" },
      { target: "hf-rec-tabs", key: "hfBrowser.tabs" },
    ],
  },
  groupChatDetail: {
    storageKey: "group_chat_detail_tour_v1",
    steps: [
      { target: "group-chat-title", key: "groupChatDetail.title" },
      { target: "group-chat-participants", key: "groupChatDetail.participants" },
      { target: "group-chat-memory", key: "groupChatDetail.memory" },
      { target: "group-chat-composer", key: "groupChatDetail.composer" },
    ],
  },
  dynamicMemory: {
    storageKey: "dynamic_memory_tour_v1",
    steps: [
      { target: "dynmem-enable", key: "dynamicMemory.enable" },
      { target: "dynmem-mode", key: "dynamicMemory.mode" },
      { target: "dynmem-preset", key: "dynamicMemory.preset" },
      { target: "dynmem-embedding", key: "dynamicMemory.embedding" },
    ],
  },
  imageGeneration: {
    storageKey: "image_gen_tour_v1",
    steps: [
      { target: "imagegen-playground", key: "imageGeneration.playground", desktopOnly: true },
      { target: "imagegen-engine", key: "imageGeneration.engine", desktopOnly: true },
      { target: "imagegen-avatar", key: "imageGeneration.avatar" },
      { target: "imagegen-scene", key: "imageGeneration.scene" },
      { target: "imagegen-prompting", key: "imageGeneration.prompting" },
    ],
  },
  playground: {
    storageKey: "playground_tour_v1",
    steps: [
      { target: "playground-prompt", key: "playground.prompt" },
      { target: "playground-generate", key: "playground.generate" },
      { target: "playground-feed", key: "playground.feed" },
      { target: "playground-model", key: "playground.model" },
      { target: "playground-loras", key: "playground.loras" },
    ],
  },
  loraLibrary: {
    storageKey: "lora_library_tour_v1",
    steps: [
      { target: "lora-tabs", key: "loraLibrary.tabs" },
      { target: "lora-import", key: "loraLibrary.import" },
      { target: "lora-installed", key: "loraLibrary.installed" },
      { target: "lora-tab-browse", key: "loraLibrary.browse" },
    ],
  },
  civitaiBrowse: {
    storageKey: "civitai_browse_tour_v1",
    steps: [
      { target: "civitai-search", key: "civitaiBrowse.search" },
      { target: "civitai-sort", key: "civitaiBrowse.sort" },
      { target: "civitai-filters", key: "civitaiBrowse.filters" },
      { target: "civitai-token", key: "civitaiBrowse.token" },
      { target: "civitai-results", key: "civitaiBrowse.results" },
    ],
  },
  sdEngine: {
    storageKey: "sd_engine_tour_v1",
    steps: [
      { target: "sd-installed", key: "sdEngine.installed" },
      { target: "sd-install", key: "sdEngine.install" },
      { target: "sd-upscaler", key: "sdEngine.upscaler" },
    ],
  },
} as const satisfies Record<string, TourDefinition>;

export type TourId = keyof typeof tours;
export interface TourStep {
  target: string;
  key: (typeof tours)[TourId]["steps"][number]["key"];
  desktopOnly?: boolean;
}
