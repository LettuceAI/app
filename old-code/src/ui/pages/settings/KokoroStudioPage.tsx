import { useEffect, useMemo, useRef, useState, useCallback } from "react";
import { useNavigate, useParams } from "react-router-dom";
import {
  AlertTriangle,
  Check,
  CheckCircle2,
  ChevronDown,
  Cpu,
  Download,
  HardDrive,
  Loader2,
  MoreHorizontal,
  Pause,
  Play,
  Plus,
  RefreshCw,
  RotateCw,
  Search,
  Sparkles,
  Square,
  Trash2,
  Volume2,
  Wand2,
  X,
} from "lucide-react";

import {
  deleteUserVoice,
  kokoroDefaultAssetRoot,
  kokoroInstallModel,
  kokoroInstallVoice,
  kokoroInstallVoices,
  kokoroListAvailableVoices,
  kokoroPreview,
  kokoroStorageStats,
  kokoroSupportedVariants,
  kokoroUninstallModel,
  kokoroUninstallVoice,
  kokoroValidateAssets,
  listAudioProviders,
  listUserVoices,
  upsertAudioProvider,
  upsertUserVoice,
  type AudioProvider,
  type KokoroAssetStatus,
  type KokoroAvailableVoice,
  type KokoroInstalledVoice,
  type KokoroModelVariant,
  type KokoroStorageStats,
  type KokoroSupportedVariant,
  type UserVoice,
} from "../../../core/storage/audioProviders";
import {
  useDownloadQueue,
  type QueuedDownload,
} from "../../../core/downloads/DownloadQueueContext";
import { BottomMenu, MenuButton } from "../../components/BottomMenu";
import { InlineDownloadCards } from "./components/DownloadQueueBar";
import { useI18n, type TranslationKey } from "../../../core/i18n/context";
import { cn, typography } from "../../design-tokens";


const LOCALE_GROUPS = {
  af: { labelKey: "kokoroStudio.locale.americanFemale", order: 10 },
  am: { labelKey: "kokoroStudio.locale.americanMale", order: 11 },
  bf: { labelKey: "kokoroStudio.locale.britishFemale", order: 20 },
  bm: { labelKey: "kokoroStudio.locale.britishMale", order: 21 },
  ef: { labelKey: "kokoroStudio.locale.spanishFemale", order: 30 },
  em: { labelKey: "kokoroStudio.locale.spanishMale", order: 31 },
  ff: { labelKey: "kokoroStudio.locale.frenchFemale", order: 40 },
  fm: { labelKey: "kokoroStudio.locale.frenchMale", order: 41 },
  hf: { labelKey: "kokoroStudio.locale.hindiFemale", order: 50 },
  hm: { labelKey: "kokoroStudio.locale.hindiMale", order: 51 },
  if: { labelKey: "kokoroStudio.locale.italianFemale", order: 60 },
  im: { labelKey: "kokoroStudio.locale.italianMale", order: 61 },
  jf: { labelKey: "kokoroStudio.locale.japaneseFemale", order: 70 },
  jm: { labelKey: "kokoroStudio.locale.japaneseMale", order: 71 },
  pf: { labelKey: "kokoroStudio.locale.portugueseFemale", order: 80 },
  pm: { labelKey: "kokoroStudio.locale.portugueseMale", order: 81 },
  zf: { labelKey: "kokoroStudio.locale.mandarinFemale", order: 90 },
  zm: { labelKey: "kokoroStudio.locale.mandarinMale", order: 91 },
} satisfies Record<string, { labelKey: TranslationKey; order: number }>;

type VoiceGroup = { key: string; labelKey: TranslationKey; order: number };

function classifyVoice(id: string): VoiceGroup {
  const prefix = id.slice(0, 2).toLowerCase();
  const meta = LOCALE_GROUPS[prefix as keyof typeof LOCALE_GROUPS];
  if (meta) return { key: prefix, ...meta };
  return { key: "other", labelKey: "kokoroStudio.locale.other", order: 999 };
}

function voiceDisplayName(id: string): string {
  const stripped = id.includes("_") ? id.split("_").slice(1).join("_") : id;
  if (!stripped) return id;
  return stripped.charAt(0).toUpperCase() + stripped.slice(1);
}

const VOICE_PREVIEW_URLS: Record<string, string> = {
  af_heart: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/S_9tkA75BT_QHKOzSX6S-.wav",
  af_alloy: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/wiZ3gvlL--p5pRItO4YRE.wav",
  af_aoede: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/Nv1xMwzjTdF9MR8v0oEEJ.wav",
  af_bella: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/sWN0rnKU6TlLsVdGqRktF.wav",
  af_jessica: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/2Oa4wITWAmiCXJ_Q97-7R.wav",
  af_kore: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/AOIgyspzZWDGpn7oQgwtu.wav",
  af_nicole: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/EY_V2OGr-hzmtTGrTCTyf.wav",
  af_nova: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/X-xdEkx3GPlQG5DK8Gsqd.wav",
  af_river: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/ZqaV2-xGUZdBQmZAF1Xqy.wav",
  af_sarah: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/xzoJBl1HCvkE8Fl8Xu2R4.wav",
  af_sky: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/ubebYQoaseyQk-jDLeWX7.wav",
  am_adam: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/tvauhDVRGvGK98I-4wv3H.wav",
  am_echo: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/qy_KuUB0hXsu-u8XaJJ_Z.wav",
  am_eric: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/JhqPjbpMhraUv5nTSPpwD.wav",
  am_fenrir: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/c0R9caBdBiNjGUUalI_DQ.wav",
  am_liam: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/DFHvulaLeOjXIDKecvNG3.wav",
  am_michael: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/IPKhsnjq1tPh3JmHH8nEg.wav",
  am_onyx: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/ov0pFDfE8NNKZ80LqW6Di.wav",
  am_puck: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/MOC654sLMHWI64g8HWesV.wav",
  am_santa: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/LzA6JmHBvQlhOviy8qVfJ.wav",
  bf_alice: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/9mnYZ3JWq7f6U12plXilA.wav",
  bf_emma: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/_fvGtKMttRI0cZVGqxMh8.wav",
  bf_isabella: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/VzlcJpqGEND_Q3duYnhiu.wav",
  bf_lily: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/qZCoartohiRlVamY8Xpok.wav",
  bm_daniel: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/Eb0TLnLXHDRYOA3TJQKq3.wav",
  bm_fable: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/NT9XkmvlezQ0FJ6Th5hoZ.wav",
  bm_george: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/y6VJbCESszLZGupPoqNkF.wav",
  bm_lewis: "https://cdn-uploads.huggingface.co/production/uploads/61b253b7ac5ecaae3d1efe0c/RlB5BRvLt-IFvTjzQNxCh.wav",
};

const FEATURED_VOICE_IDS = [
  "af_heart",
  "af_alloy",
  "af_aoede",
  "af_nova",
  "am_adam",
  "am_michael",
  "bf_emma",
  "bm_george",
];

const STARTER_PACK_VOICE_IDS = ["af_heart", "am_adam", "bf_emma", "bm_george"];

const MIX_TINTS = ["bg-accent", "bg-info", "bg-warning", "bg-secondary", "bg-danger"];

function tintForVoice(voiceId: string): string {
  let hash = 0;
  for (let i = 0; i < voiceId.length; i += 1) {
    hash = (hash * 31 + voiceId.charCodeAt(i)) | 0;
  }
  return MIX_TINTS[Math.abs(hash) % MIX_TINTS.length];
}

type ParsedBlendEntry = { voiceId: string; weight: number };

function parseBlend(field: string): ParsedBlendEntry[] {
  const trimmed = field.trim();
  if (!trimmed) return [];
  if (!trimmed.startsWith("[")) {
    return [{ voiceId: trimmed, weight: 1 }];
  }
  try {
    const parsed = JSON.parse(trimmed) as Array<{ voiceId?: string; weight?: number }>;
    if (!Array.isArray(parsed)) return [];
    return parsed
      .filter((e) => e && typeof e.voiceId === "string")
      .map((e) => ({
        voiceId: e.voiceId as string,
        weight: typeof e.weight === "number" ? e.weight : 0,
      }))
      .filter((e) => e.weight > 0);
  } catch {
    return [];
  }
}

function readSpeed(prompt: string | null | undefined): number {
  if (!prompt) return 1;
  try {
    const meta = JSON.parse(prompt);
    if (meta && typeof meta.speed === "number" && meta.speed > 0) return meta.speed;
  } catch {  }
  return 1;
}

function formatBytes(bytes: number): string {
  if (!bytes || bytes <= 0) return "0 B";
  const units = ["B", "KB", "MB", "GB"];
  let value = bytes;
  let i = 0;
  while (value >= 1024 && i < units.length - 1) {
    value /= 1024;
    i += 1;
  }
  return `${value >= 100 ? value.toFixed(0) : value.toFixed(1)} ${units[i]}`;
}

function formatTime(seconds: number): string {
  if (!Number.isFinite(seconds) || seconds < 0) return "0:00";
  const m = Math.floor(seconds / 60);
  const s = Math.floor(seconds % 60);
  return `${m}:${s.toString().padStart(2, "0")}`;
}


type PlayerSource = {
  id: string;
  label: string;
  sublabel?: string;
  src: string;
  revoke?: boolean;
};

type PlayerState = {
  source: PlayerSource | null;
  playing: boolean;
  currentTime: number;
  duration: number;
};

function useStudioPlayer() {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const [state, setState] = useState<PlayerState>({
    source: null,
    playing: false,
    currentTime: 0,
    duration: 0,
  });

  const stop = useCallback(() => {
    const audio = audioRef.current;
    if (audio) {
      audio.pause();
      try {
        const wasObject = audio.src.startsWith("blob:");
        audio.removeAttribute("src");
        audio.load();
        if (wasObject) {
        }
      } catch { }
    }
    setState((prev) => {
      if (prev.source?.revoke && prev.source.src.startsWith("blob:")) {
        URL.revokeObjectURL(prev.source.src);
      }
      return { source: null, playing: false, currentTime: 0, duration: 0 };
    });
  }, []);

  const play = useCallback(
    (next: PlayerSource) => {
      const existing = audioRef.current;
      if (existing) {
        existing.pause();
      }
      setState((prev) => {
        if (prev.source?.revoke && prev.source.src !== next.src && prev.source.src.startsWith("blob:")) {
          URL.revokeObjectURL(prev.source.src);
        }
        return { source: next, playing: false, currentTime: 0, duration: 0 };
      });
      const audio = new Audio(next.src);
      audioRef.current = audio;
      audio.addEventListener("loadedmetadata", () => {
        setState((prev) => ({ ...prev, duration: audio.duration || 0 }));
      });
      audio.addEventListener("timeupdate", () => {
        setState((prev) => ({ ...prev, currentTime: audio.currentTime }));
      });
      audio.addEventListener("play", () => {
        setState((prev) => ({ ...prev, playing: true }));
      });
      audio.addEventListener("pause", () => {
        setState((prev) => ({ ...prev, playing: false }));
      });
      audio.addEventListener("ended", () => {
        setState({ source: null, playing: false, currentTime: 0, duration: 0 });
        if (next.revoke && next.src.startsWith("blob:")) {
          URL.revokeObjectURL(next.src);
        }
      });
      audio.addEventListener("error", () => {
        setState({ source: null, playing: false, currentTime: 0, duration: 0 });
      });
      void audio.play().catch(() => {
        setState({ source: null, playing: false, currentTime: 0, duration: 0 });
      });
    },
    [],
  );

  const toggle = useCallback(() => {
    const audio = audioRef.current;
    if (!audio) return;
    if (audio.paused) void audio.play().catch(() => {});
    else audio.pause();
  }, []);

  useEffect(() => {
    return () => {
      const audio = audioRef.current;
      if (audio) {
        audio.pause();
        audio.removeAttribute("src");
        audio.load();
      }
    };
  }, []);

  return { state, play, stop, toggle };
}

function base64ToBlobUrl(base64: string, format: string): string {
  const byteString = atob(base64);
  const bytes = new Uint8Array(byteString.length);
  for (let i = 0; i < byteString.length; i += 1) {
    bytes[i] = byteString.charCodeAt(i);
  }
  const blob = new Blob([bytes], { type: format });
  return URL.createObjectURL(blob);
}

export function KokoroStudioPage() {
  const { t } = useI18n();
  const navigate = useNavigate();
  const { providerId } = useParams<{ providerId: string }>();
  const [provider, setProvider] = useState<AudioProvider | null>(null);
  const [providerLoading, setProviderLoading] = useState(true);

  const [variants, setVariants] = useState<KokoroSupportedVariant[]>([]);
  const [assetStatus, setAssetStatus] = useState<KokoroAssetStatus | null>(null);
  const [storageStats, setStorageStats] = useState<KokoroStorageStats | null>(null);
  const [availableVoices, setAvailableVoices] = useState<KokoroAvailableVoice[]>([]);
  const [installedVoices, setInstalledVoices] = useState<KokoroInstalledVoice[]>([]);
  const [savedBlends, setSavedBlends] = useState<UserVoice[]>([]);

  const [voiceSearch, setVoiceSearch] = useState("");
  const [voiceFilter, setVoiceFilter] = useState<"all" | "installed">("all");
  const [expandedGroups, setExpandedGroups] = useState<Set<string>>(new Set());
  const [selectMode, setSelectMode] = useState(false);
  const [selectedVoiceIds, setSelectedVoiceIds] = useState<Set<string>>(new Set());

  const [isRefreshing, setIsRefreshing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [previewingId, setPreviewingId] = useState<string | null>(null);

  const [actionBlend, setActionBlend] = useState<UserVoice | null>(null);
  const [engineSheetOpen, setEngineSheetOpen] = useState(false);
  const [tryingVoiceId, setTryingVoiceId] = useState<string | null>(null);
  const [tryText, setTryText] = useState(t("voices.extra.kokoro.tryText"));

  const [experimentSourceId, setExperimentSourceId] = useState<string>("");
  const [experimentText, setExperimentText] = useState(
    t("voices.extra.kokoro.experimentDefaultText"),
  );
  const [experimentSpeed, setExperimentSpeed] = useState(1);
  const [isExperimenting, setIsExperimenting] = useState(false);

  const [isDeletingModel, setIsDeletingModel] = useState(false);
  const [isSwitchingVariant, setIsSwitchingVariant] = useState(false);


  const { queue, dismissItem } = useDownloadQueue();
  const prevQueueRef = useRef<QueuedDownload[]>([]);
  const { state: playerState, play, stop, toggle } = useStudioPlayer();

  const variant: KokoroModelVariant | undefined = provider?.kokoroVariant;
  const assetRoot = provider?.assetRoot?.trim() ?? "";

  const refreshAssetStatus = useCallback(async () => {
    if (!assetRoot || !variant) return;
    try {
      const status = await kokoroValidateAssets(assetRoot, variant);
      setAssetStatus(status);
      setInstalledVoices(status.installedVoices);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [assetRoot, variant]);

  const refreshVoiceCatalog = useCallback(async () => {
    if (!assetRoot) return;
    try {
      const list = await kokoroListAvailableVoices(assetRoot);
      setAvailableVoices(list);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  }, [assetRoot]);

  const refreshStorageStats = useCallback(async () => {
    if (!assetRoot) return;
    try {
      const stats = await kokoroStorageStats(assetRoot);
      setStorageStats(stats);
    } catch (e) {
      console.error("Failed to load storage stats:", e);
    }
  }, [assetRoot]);

  const refreshAll = useCallback(async () => {
    setIsRefreshing(true);
    try {
      await Promise.all([refreshAssetStatus(), refreshVoiceCatalog(), refreshStorageStats()]);
    } finally {
      setIsRefreshing(false);
    }
  }, [refreshAssetStatus, refreshVoiceCatalog, refreshStorageStats]);

  const reloadBlends = useCallback(async () => {
    if (!providerId) return;
    try {
      const all = await listUserVoices();
      setSavedBlends(all.filter((v) => v.providerId === providerId));
    } catch (e) {
      console.error("Failed to load blends:", e);
    }
  }, [providerId]);

  useEffect(() => {
    if (!providerId) return;
    void (async () => {
      setProviderLoading(true);
      try {
        const [list, supported] = await Promise.all([
          listAudioProviders(),
          kokoroSupportedVariants(),
        ]);
        setVariants(supported);
        let found = list.find((p) => p.id === providerId) ?? null;
        if (found) {
          let next = found;
          if (!next.assetRoot?.trim()) {
            const defaultRoot = await kokoroDefaultAssetRoot().catch(() => "");
            if (defaultRoot) next = { ...next, assetRoot: defaultRoot };
          }
          if (!next.kokoroVariant && supported.length > 0) {
            const preferred = supported.find((v) => v.id === "fp16") ?? supported[0];
            next = { ...next, kokoroVariant: preferred.id };
          }
          if (next !== found) {
            found = await upsertAudioProvider(next);
          }
        }
        setProvider(found);
      } catch (e) {
        setError(e instanceof Error ? e.message : String(e));
      } finally {
        setProviderLoading(false);
      }
    })();
  }, [providerId]);

  useEffect(() => {
    if (!provider) return;
    void refreshAll();
    void reloadBlends();
  }, [provider, refreshAll, reloadBlends]);

  const kokoroQueue = useMemo(
    () =>
      queue.filter(
        (item) => item.queueKind === "kokoro" && (!assetRoot || item.assetRoot === assetRoot),
      ),
    [queue, assetRoot],
  );
  const activeQueue = kokoroQueue.filter(
    (item) => item.status === "downloading" || item.status === "queued",
  );
  const failedQueue = kokoroQueue.filter((item) => item.status === "error");

  const activeInstallIds = useMemo(() => {
    const ids = new Set<string>();
    for (const item of activeQueue) {
      if (item.installId) ids.add(item.installId);
    }
    return ids;
  }, [activeQueue]);

  useEffect(() => {
    const prev = prevQueueRef.current;
    let changed = false;
    for (const item of kokoroQueue) {
      const prevItem = prev.find((c) => c.id === item.id);
      const becameComplete =
        (!prevItem || prevItem.status !== "complete") && item.status === "complete";
      if (becameComplete) {
        changed = true;
      }
    }
    if (changed) {
      void refreshAssetStatus();
      void refreshVoiceCatalog();
      void refreshStorageStats();
    }
    prevQueueRef.current = kokoroQueue;
  }, [kokoroQueue, refreshAssetStatus, refreshVoiceCatalog, refreshStorageStats]);

  const variantInfo = variants.find((v) => v.id === variant);
  const modelInstalled = Boolean(assetStatus?.resolvedModelPath);
  const isInstallingModel = activeQueue.some((item) => item.installKind === "model");
  const hasVoices = installedVoices.length > 0;

  const filteredVoices = useMemo(() => {
    const q = voiceSearch.trim().toLowerCase();
    return availableVoices.filter((v) => {
      const isInstalled = installedVoices.some((iv) => iv.id === v.id);
      if (voiceFilter === "installed" && !isInstalled) return false;
      if (!q) return true;
      const display = voiceDisplayName(v.id).toLowerCase();
      return v.id.toLowerCase().includes(q) || display.includes(q);
    });
  }, [availableVoices, voiceSearch, voiceFilter, installedVoices]);

  const groupedVoices = useMemo(() => {
    const groups = new Map<string, { meta: VoiceGroup; voices: KokoroAvailableVoice[] }>();
    for (const voice of filteredVoices) {
      const meta = classifyVoice(voice.id);
      const existing = groups.get(meta.key);
      if (existing) existing.voices.push(voice);
      else groups.set(meta.key, { meta, voices: [voice] });
    }
    return [...groups.values()].sort((a, b) => a.meta.order - b.meta.order);
  }, [filteredVoices]);

  const featuredVoices = useMemo(() => {
    return FEATURED_VOICE_IDS.map((id) => availableVoices.find((v) => v.id === id))
      .filter((v): v is KokoroAvailableVoice => Boolean(v));
  }, [availableVoices]);

  const experimentOptions = useMemo(() => {
    const voices = installedVoices.map((v) => ({
      key: `voice:${v.id}`,
      label: voiceDisplayName(v.id),
      kind: "voice" as const,
      voiceId: v.id,
    }));
    const blends = savedBlends.map((b) => ({
      key: `blend:${b.id}`,
      label: b.name,
      kind: "blend" as const,
      blendId: b.id,
    }));
    return [...blends, ...voices];
  }, [installedVoices, savedBlends]);

  useEffect(() => {
    if (experimentSourceId) return;
    if (experimentOptions.length > 0) {
      setExperimentSourceId(experimentOptions[0].key);
    }
  }, [experimentOptions, experimentSourceId]);

  const handleInstallVoice = async (voiceId: string) => {
    if (!assetRoot) return;
    setError(null);
    try {
      await kokoroInstallVoice(assetRoot, voiceId);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleInstallSelected = async () => {
    if (!assetRoot || selectedVoiceIds.size === 0) return;
    const toInstall = [...selectedVoiceIds].filter(
      (id) => !installedVoices.some((iv) => iv.id === id),
    );
    if (toInstall.length === 0) {
      setSelectedVoiceIds(new Set());
      setSelectMode(false);
      return;
    }
    setError(null);
    try {
      await kokoroInstallVoices(assetRoot, toInstall);
      setSelectedVoiceIds(new Set());
      setSelectMode(false);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleInstallGroup = async (voiceIds: string[]) => {
    if (!assetRoot || voiceIds.length === 0) return;
    setError(null);
    try {
      await kokoroInstallVoices(assetRoot, voiceIds);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleInstallStarterPack = async () => {
    if (!assetRoot) return;
    const toInstall = STARTER_PACK_VOICE_IDS.filter(
      (id) => !installedVoices.some((iv) => iv.id === id),
    );
    if (toInstall.length === 0) return;
    setError(null);
    try {
      await kokoroInstallVoices(assetRoot, toInstall);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleUninstallVoice = async (voiceId: string) => {
    if (!assetRoot) return;
    try {
      await kokoroUninstallVoice(assetRoot, voiceId);
      await Promise.all([refreshAssetStatus(), refreshStorageStats()]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDeleteModel = async () => {
    if (!variant || !assetRoot || isDeletingModel) return;
    setError(null);
    setIsDeletingModel(true);
    try {
      await kokoroUninstallModel(assetRoot, variant);
      await Promise.all([refreshAssetStatus(), refreshStorageStats()]);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsDeletingModel(false);
    }
  };

  const handleVariantChange = async (next: KokoroModelVariant) => {
    if (!provider || next === variant || isSwitchingVariant) return;
    setError(null);
    setIsSwitchingVariant(true);
    try {
      const updated = await upsertAudioProvider({ ...provider, kokoroVariant: next });
      setProvider(updated);
      await refreshAll();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setIsSwitchingVariant(false);
    }
  };

  const handleDeleteBlend = async (id: string) => {
    setActionBlend(null);
    try {
      await deleteUserVoice(id);
      await reloadBlends();
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleUpdateBlendSpeed = async (blend: UserVoice, nextSpeed: number) => {
    try {
      const promptPayload = JSON.stringify({ speed: nextSpeed });
      await upsertUserVoice({ ...blend, prompt: promptPayload });
      setSavedBlends((prev) =>
        prev.map((b) => (b.id === blend.id ? { ...b, prompt: promptPayload } : b)),
      );
    } catch (e) {
      console.error("Failed to update blend speed:", e);
    }
  };

  const playSample = (voiceId: string) => {
    const url = VOICE_PREVIEW_URLS[voiceId];
    if (!url) return;
    if (playerState.source?.id === `sample:${voiceId}`) {
      stop();
      return;
    }
    play({
      id: `sample:${voiceId}`,
      label: voiceDisplayName(voiceId),
      sublabel: t("kokoroStudio.sample"),
      src: url,
    });
  };

  const playSynth = async (
    sourceId: string,
    label: string,
    sublabel: string,
    blend: ParsedBlendEntry[],
    text: string,
    speed: number,
  ) => {
    if (!provider?.assetRoot || !provider.kokoroVariant) return;
    if (!text.trim()) return;
    if (playerState.source?.id === sourceId) {
      stop();
      return;
    }
    setError(null);
    setPreviewingId(sourceId);
    try {
      const response = await kokoroPreview(
        provider.assetRoot,
        provider.kokoroVariant,
        blend,
        text.trim(),
        speed,
      );
      const url = base64ToBlobUrl(response.audioBase64, response.format);
      play({ id: sourceId, label, sublabel, src: url, revoke: true });
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    } finally {
      setPreviewingId(null);
    }
  };

  const handleTryVoice = async (voiceId: string) => {
    await playSynth(
      `try:${voiceId}`,
      voiceDisplayName(voiceId),
      t("voices.extra.kokoro.livePreview"),
      [{ voiceId, weight: 1 }],
      tryText,
      1,
    );
    setTryingVoiceId(null);
  };

  const handlePlayBlend = async (blend: UserVoice) => {
    const entries = parseBlend(blend.voiceId);
    if (entries.length === 0) return;
    const speed = readSpeed(blend.prompt);
    await playSynth(
      `blend:${blend.id}`,
      blend.name,
      t("voices.extra.kokoro.savedBlend"),
      entries,
      t("voices.extra.kokoro.defaultPreviewText"),
      speed,
    );
  };

  const handleExperimentSpeak = async () => {
    const opt = experimentOptions.find((o) => o.key === experimentSourceId);
    if (!opt) return;
    setIsExperimenting(true);
    try {
      let blend: ParsedBlendEntry[] = [];
      let label = opt.label;
      if (opt.kind === "voice") {
        blend = [{ voiceId: opt.voiceId, weight: 1 }];
      } else {
        const target = savedBlends.find((b) => b.id === opt.blendId);
        if (!target) return;
        blend = parseBlend(target.voiceId);
        label = target.name;
      }
      await playSynth(
        `experiment:${experimentSourceId}:${Date.now()}`,
        label,
        t("voices.extra.kokoro.experiment"),
        blend,
        experimentText,
        experimentSpeed,
      );
    } finally {
      setIsExperimenting(false);
    }
  };

  const handleRetryFailed = async (item: QueuedDownload) => {
    try {
      await dismissItem(item.id);
      if (item.installKind === "model" && item.variant && assetRoot) {
        await kokoroInstallModel(assetRoot, item.variant as KokoroModelVariant);
      } else if (item.installKind === "voice" && item.voiceId && assetRoot) {
        await kokoroInstallVoice(assetRoot, item.voiceId);
      }
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

  const handleDismissFailed = async () => {
    await Promise.all(failedQueue.map((item) => dismissItem(item.id)));
  };

  if (providerLoading) {
    return (
      <div className="flex h-64 items-center justify-center">
        <Loader2 className="h-8 w-8 animate-spin text-fg/40" />
      </div>
    );
  }

  if (!provider) {
    return (
      <div className="flex h-64 flex-col items-center justify-center gap-3">
        <AlertTriangle className="h-10 w-10 text-warning/60" />
        <p className="text-sm text-fg/60">{t("voices.extra.kokoro.providerNotFound")}</p>
        <button
          onClick={() => navigate("/settings/providers?tab=audio")}
          className="rounded-lg border border-fg/10 bg-fg/5 px-4 py-2 text-sm text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
        >
          {t("voices.extra.kokoro.backToProviders")}
        </button>
      </div>
    );
  }

  const playerActive = Boolean(playerState.source);

  return (
    <div className="flex h-full flex-col">
      {/* Engine status strip */}
      <div className="border-b border-fg/10 bg-surface-el/30 px-4 py-2 backdrop-blur">
        <div className="flex flex-wrap items-center gap-2">
          <Pill icon={<Cpu className="h-3 w-3 text-fg/50" />}>
            <span>{variantInfo?.label ?? variant ?? t("voices.extra.kokoro.variantUnset")}</span>
            <span className="text-fg/30">·</span>
            <span className="text-fg/55">
              {storageStats ? formatBytes(storageStats.modelBytes) : variantInfo ? `${variantInfo.sizeMb} MB` : "—"}
            </span>
          </Pill>
          <Pill tone={modelInstalled ? "accent" : "warning"} dot={modelInstalled}>
            {modelInstalled ? t("voices.extra.kokoro.ready") : t("voices.extra.kokoro.modelNotInstalled")}
          </Pill>
          {storageStats && storageStats.voiceCount > 0 && (
            <Pill icon={<HardDrive className="h-3 w-3 text-fg/45" />}>
              <span>{t("voices.extra.kokoro.voiceCount", { count: storageStats.voiceCount })}</span>
              <span className="text-fg/30">·</span>
              <span className="text-fg/55">{formatBytes(storageStats.voicesBytes)}</span>
            </Pill>
          )}
          <div className="ml-auto flex items-center gap-1">
            <button
              onClick={() => void refreshAll()}
              disabled={isRefreshing}
              className="flex items-center gap-1 rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] font-medium text-fg/65 transition hover:border-fg/20 hover:bg-fg/10 disabled:opacity-50"
            >
              <RefreshCw className={cn("h-3 w-3", isRefreshing && "animate-spin")} />
              {t("common.buttons.refresh")}
            </button>
            <button
              onClick={() => setEngineSheetOpen(true)}
              className="flex h-7 items-center justify-center rounded-full border border-fg/10 bg-fg/5 px-2 text-fg/65 transition hover:border-fg/20 hover:bg-fg/10"
              aria-label={t("voices.extra.kokoro.engineActions")}
            >
              <MoreHorizontal className="h-3.5 w-3.5" />
            </button>
          </div>
        </div>
      </div>

      <div
        className={cn(
          "flex-1 overflow-y-auto px-4 py-4 space-y-6",
          playerActive
            ? "pb-[calc(env(safe-area-inset-bottom)+88px)]"
            : "pb-[calc(env(safe-area-inset-bottom)+24px)]",
        )}
      >
        {error && (
          <div className="flex items-start gap-2 rounded-lg border border-danger/30 bg-danger/10 px-3 py-2">
            <AlertTriangle className="mt-0.5 h-3.5 w-3.5 shrink-0 text-danger/85" />
            <p className="flex-1 text-[12px] text-danger/85">{error}</p>
            <button
              onClick={() => setError(null)}
              className="text-danger/60 transition hover:text-danger"
              aria-label={t("common.buttons.discard")}
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        )}

        {!modelInstalled && !isInstallingModel && (
          <section className="rounded-2xl border border-warning/25 bg-warning/[0.06] p-4">
            <div className="flex items-start gap-3">
              <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl bg-warning/15 text-warning">
                <AlertTriangle className="h-5 w-5" />
              </div>
              <div className="min-w-0 flex-1">
                <h3 className="text-[14px] font-semibold text-fg">
                  {t("voices.extra.kokoro.notSetUpTitle")}
                </h3>
                <p className="mt-0.5 text-[12px] leading-relaxed text-fg/60">
                  {t("voices.extra.kokoro.notSetUpHint")}
                </p>
                <button
                  onClick={() => navigate("/settings/providers?tab=audio")}
                  className="mt-3 inline-flex items-center gap-2 rounded-xl bg-accent px-4 py-2 text-[13px] font-semibold text-bg transition hover:brightness-110"
                >
                  {t("voices.extra.kokoro.goToProviders")}
                </button>
              </div>
            </div>
          </section>
        )}

        {/* Failed downloads */}
        {failedQueue.length > 0 && (
          <div className="rounded-lg border border-danger/25 bg-danger/8 px-3 py-2">
            <div className="mb-2 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="text-[13px] font-medium text-danger/90">
                  {t("voices.extra.kokoro.downloadsFailed", { count: failedQueue.length })}
                </p>
                <p className="text-[11px] text-danger/65">{t("voices.extra.kokoro.retryOrDismissAll")}</p>
              </div>
              <button
                onClick={() => void handleDismissFailed()}
                className="shrink-0 rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] text-fg/65 transition hover:border-fg/20 hover:bg-fg/10"
              >
                {t("voices.extra.kokoro.dismissAll")}
              </button>
            </div>
            <div className="space-y-1">
              {failedQueue.map((item) => (
                <div
                  key={item.id}
                  className="flex items-center gap-2 rounded-md border border-fg/10 bg-fg/5 px-2.5 py-1.5"
                >
                  <div className="min-w-0 flex-1">
                    <p className="truncate text-[11px] text-fg/85">
                      {item.installKind === "model"
                        ? `${t("voices.extra.kokoro.model")} · ${item.variant ?? ""}`
                        : `${t("voices.extra.kokoro.voice")} · ${voiceDisplayName(item.voiceId ?? "")}`}
                    </p>
                    {item.error && (
                      <p className="truncate text-[10px] text-danger/65">{item.error}</p>
                    )}
                  </div>
                  <button
                    onClick={() => void handleRetryFailed(item)}
                    className="rounded-full border border-info/25 bg-info/10 px-2 py-0.5 text-[10px] font-semibold text-info transition hover:border-info/40 hover:bg-info/15"
                  >
                    <RotateCw className="h-3 w-3" />
                  </button>
                  <button
                    onClick={() => void dismissItem(item.id)}
                    className="text-fg/40 transition hover:text-danger"
                    aria-label={t("kokoroStudio.dismiss")}
                  >
                    <X className="h-3 w-3" />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Active downloads */}
        {activeInstallIds.size > 0 && (
          <section>
            <SectionHeader label={t("voices.extra.kokoro.downloads")} />
            <InlineDownloadCards
              compact
              filter={(item) =>
                item.queueKind === "kokoro" &&
                !!item.installId &&
                activeInstallIds.has(item.installId)
              }
            />
          </section>
        )}

        <div className="grid min-h-0 gap-6 lg:grid-cols-[minmax(0,440px)_minmax(0,1fr)]">
        <div className="min-w-0 space-y-6">

        {/* Experiment */}
        {modelInstalled && experimentOptions.length > 0 && (
          <section>
            <SectionHeader label={t("voices.extra.kokoro.experiment")} />
            <div className="overflow-hidden rounded-xl border border-fg/10 bg-fg/4">
              <textarea
                value={experimentText}
                onChange={(e) => setExperimentText(e.target.value)}
                rows={3}
                placeholder={t("voices.extra.kokoro.experimentPlaceholder")}
                className="w-full resize-none border-0 bg-transparent px-4 pt-3 pb-1 text-[13px] leading-relaxed text-fg placeholder-fg/35 focus:outline-none"
              />
              <div className="flex flex-wrap items-center gap-2 border-t border-fg/10 bg-fg/2 px-3 py-2">
                <select
                  value={experimentSourceId}
                  onChange={(e) => setExperimentSourceId(e.target.value)}
                  className="min-w-0 max-w-50 truncate rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] font-medium text-fg/80 focus:border-fg/25 focus:outline-none"
                >
                  {experimentOptions
                    .filter((o) => o.kind === "blend")
                    .map((o) => (
                      <option key={o.key} value={o.key} className="bg-surface-el">
                        {t("voices.extra.kokoro.blend")} · {o.label}
                      </option>
                    ))}
                  {experimentOptions
                    .filter((o) => o.kind === "voice")
                    .map((o) => (
                      <option key={o.key} value={o.key} className="bg-surface-el">
                        {t("voices.extra.kokoro.voice")} · {o.label}
                      </option>
                    ))}
                </select>
                <div className="flex flex-1 items-center gap-2 rounded-full border border-fg/10 bg-fg/5 px-3 py-1">
                  <span className="text-[10px] uppercase tracking-wider text-fg/40">{t("voices.extra.kokoro.speed")}</span>
                  <input
                    type="range"
                    min={0.5}
                    max={2}
                    step={0.05}
                    value={experimentSpeed}
                    onChange={(e) => setExperimentSpeed(Number(e.target.value))}
                    className="h-1 min-w-20 flex-1 accent-accent"
                  />
                  <span className="w-10 text-right font-mono text-[10px] tabular-nums text-fg/60">
                    {experimentSpeed.toFixed(2)}×
                  </span>
                </div>
                <button
                  onClick={() => void handleExperimentSpeak()}
                  disabled={isExperimenting || !experimentText.trim() || !experimentSourceId}
                  className="inline-flex shrink-0 items-center gap-1.5 rounded-full bg-accent px-4 py-1.5 text-[11px] font-semibold text-bg transition hover:brightness-110 active:scale-[0.98] disabled:opacity-50"
                >
                  {isExperimenting ? (
                    <Loader2 className="h-3 w-3 animate-spin" />
                  ) : (
                    <Play className="h-3 w-3" fill="currentColor" />
                  )}
                  {t("voices.extra.kokoro.speak")}
                </button>
              </div>
            </div>
          </section>
        )}

        {/* Saved blends */}
        <section>
          <SectionHeader
            label={t("voices.extra.kokoro.yourBlends")}
            right={
              <button
                onClick={() => navigate(`/settings/voices/kokoro/${providerId}/blend`)}
                disabled={!modelInstalled || !hasVoices}
                className="flex items-center gap-1 rounded-full border border-accent/30 bg-accent/10 px-2.5 py-1 text-[11px] font-medium text-accent/85 transition hover:border-accent/50 hover:bg-accent/15 disabled:opacity-40"
              >
                <Plus className="h-3 w-3" />
                {t("voices.extra.kokoro.newBlend")}
              </button>
            }
          />
          {savedBlends.length === 0 ? (
            <div className="rounded-xl border border-dashed border-fg/15 bg-fg/2.5 px-4 py-6 text-center text-[12px] text-fg/55">
              {modelInstalled && hasVoices
                ? t("voices.extra.kokoro.noSavedBlends")
                : t("voices.extra.kokoro.installModelAndVoiceFirst")}
            </div>
          ) : (
            <div className="space-y-1.5">
              {savedBlends.map((blend) => (
                <BlendRow
                  key={blend.id}
                  blend={blend}
                  providerId={providerId!}
                  isPreviewing={previewingId === `blend:${blend.id}`}
                  isPlaying={playerState.source?.id === `blend:${blend.id}`}
                  onPlay={() => void handlePlayBlend(blend)}
                  onMore={() => setActionBlend(blend)}
                  onSpeedChange={(s) => void handleUpdateBlendSpeed(blend, s)}
                />
              ))}
            </div>
          )}
        </section>

        </div>
        <div className="min-w-0 space-y-4">

        {/* Featured voices */}
        {modelInstalled && featuredVoices.length > 0 && (
          <section>
            <SectionHeader label={t("voices.extra.kokoro.featured")} />
            <div className="-mx-4 overflow-x-auto px-4 pb-1 lg:mx-0 lg:px-0">
              <div className="flex gap-1.5">
                {featuredVoices.map((voice) => {
                  const isInstalled = installedVoices.some((iv) => iv.id === voice.id);
                  const isQueued = activeQueue.some(
                    (item) => item.installKind === "voice" && item.voiceId === voice.id,
                  );
                  const isPlayingSample = playerState.source?.id === `sample:${voice.id}`;
                  const meta = classifyVoice(voice.id);
                  return (
                    <div
                      key={voice.id}
                      className={cn(
                        "group relative flex h-14 w-56 shrink-0 items-center gap-2.5 rounded-lg border px-2 transition",
                        isPlayingSample
                          ? "border-accent/40 bg-accent/8"
                          : "border-fg/10 bg-fg/4 hover:border-fg/25 hover:bg-fg/[0.07]",
                      )}
                    >
                      <button
                        onClick={() => playSample(voice.id)}
                        className={cn(
                          "flex h-9 w-9 shrink-0 items-center justify-center rounded-full transition",
                          isPlayingSample
                            ? "bg-accent text-bg shadow-[0_0_0_3px_rgba(52,211,153,0.15)]"
                            : "bg-fg/10 text-fg/85 group-hover:bg-fg/20",
                        )}
                        aria-label={isPlayingSample ? t("voices.extra.kokoro.stop") : t("voices.extra.kokoro.sample")}
                      >
                        {isPlayingSample ? (
                          <Square className="h-3 w-3" fill="currentColor" />
                        ) : (
                          <Play className="ml-0.5 h-3 w-3" fill="currentColor" />
                        )}
                      </button>
                      <div className="min-w-0 flex-1">
                        <div className="flex items-center gap-1.5">
                          <span className="truncate text-[13px] font-semibold text-fg">
                            {voiceDisplayName(voice.id)}
                          </span>
                          {isInstalled && (
                            <CheckCircle2 className="h-3 w-3 shrink-0 text-accent/80" />
                          )}
                        </div>
                        <p className="truncate text-[10px] text-fg/45">{t(meta.labelKey)}</p>
                      </div>
                      {!isInstalled && (
                        <button
                          onClick={() => void handleInstallVoice(voice.id)}
                          disabled={isQueued}
                          className="shrink-0 rounded-full border border-info/25 bg-info/10 px-2 py-0.5 text-[10px] font-semibold text-info transition hover:border-info/40 hover:bg-info/15 disabled:opacity-60"
                        >
                          {isQueued ? <Loader2 className="h-3 w-3 animate-spin" /> : t("common.buttons.install")}
                        </button>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          </section>
        )}

        {/* Voice library */}
        <section>
          <SectionHeader
            label={t("voices.extra.kokoro.voiceLibrary")}
            right={
              modelInstalled && (
                <div className="flex items-center gap-1.5">
                  {!hasVoices && availableVoices.length > 0 && (
                    <button
                      onClick={() => void handleInstallStarterPack()}
                      className="flex items-center gap-1 rounded-full border border-accent/30 bg-accent/10 px-2.5 py-1 text-[11px] font-medium text-accent/85 transition hover:border-accent/50 hover:bg-accent/15"
                    >
                      <Sparkles className="h-3 w-3" />
                      {t("voices.extra.kokoro.starterPack")}
                    </button>
                  )}
                  {selectMode ? (
                    <button
                      onClick={() => {
                        setSelectMode(false);
                        setSelectedVoiceIds(new Set());
                      }}
                      className="rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] text-fg/65 transition hover:border-fg/20 hover:bg-fg/10"
                    >
                      {t("common.buttons.cancel")}
                    </button>
                  ) : (
                    <button
                      onClick={() => setSelectMode(true)}
                      className="rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] text-fg/65 transition hover:border-fg/20 hover:bg-fg/10"
                    >
                      {t("voices.extra.kokoro.select")}
                    </button>
                  )}
                </div>
              )
            }
          />

          <div className="mb-2.5 flex flex-wrap items-center gap-2">
            <div className="relative flex-1 min-w-45">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-fg/35" />
              <input
                value={voiceSearch}
                onChange={(e) => setVoiceSearch(e.target.value)}
                disabled={!modelInstalled}
                placeholder={t("characters.description.searchVoicesPlaceholder")}
                className="w-full rounded-full border border-fg/10 bg-fg/5 py-1.5 pl-8 pr-3 text-[12px] text-fg placeholder-fg/35 focus:border-fg/25 focus:outline-none disabled:opacity-50"
              />
            </div>
            <div className="flex items-center rounded-full border border-fg/10 bg-fg/5 p-0.5">
              <FilterPill
                active={voiceFilter === "all"}
                onClick={() => setVoiceFilter("all")}
                label={t("voices.extra.kokoro.all")}
                count={availableVoices.length}
              />
              <FilterPill
                active={voiceFilter === "installed"}
                onClick={() => setVoiceFilter("installed")}
                label={t("voices.extra.kokoro.installed")}
                count={installedVoices.length}
              />
            </div>
          </div>

          {!modelInstalled ? (
            <div className="rounded-xl border border-dashed border-fg/15 bg-fg/2.5 px-4 py-6 text-center text-[12px] text-fg/55">
              {t("voices.extra.kokoro.installModelToBrowse")}
            </div>
          ) : groupedVoices.length === 0 ? (
            <div className="rounded-xl border border-dashed border-fg/15 bg-fg/2.5 px-4 py-6 text-center text-[12px] text-fg/55">
              {availableVoices.length === 0
                ? t("voices.extra.kokoro.noVoicesInCatalog")
                : t("voices.extra.kokoro.noVoicesMatch")}
            </div>
          ) : (
            <div className="space-y-1.5">
              {groupedVoices.map((group) => {
                const installedInGroup = group.voices.filter((v) =>
                  installedVoices.some((iv) => iv.id === v.id),
                ).length;
                const isExpanded =
                  voiceSearch.trim().length > 0 ||
                  voiceFilter === "installed" ||
                  expandedGroups.has(group.meta.key);
                return (
                  <VoiceGroupBlock
                    key={group.meta.key}
                    group={group}
                    installedVoices={installedVoices}
                    activeQueue={activeQueue}
                    expanded={isExpanded}
                    installedInGroup={installedInGroup}
                    selectMode={selectMode}
                    selectedVoiceIds={selectedVoiceIds}
                    nowPlayingId={playerState.source?.id ?? null}
                    previewingId={previewingId}
                    tryingVoiceId={tryingVoiceId}
                    tryText={tryText}
                    onTryTextChange={setTryText}
                    onToggleSelect={(id) =>
                      setSelectedVoiceIds((prev) => {
                        const next = new Set(prev);
                        if (next.has(id)) next.delete(id);
                        else next.add(id);
                        return next;
                      })
                    }
                    onToggle={() =>
                      setExpandedGroups((prev) => {
                        const next = new Set(prev);
                        if (next.has(group.meta.key)) next.delete(group.meta.key);
                        else next.add(group.meta.key);
                        return next;
                      })
                    }
                    onInstall={handleInstallVoice}
                    onInstallGroup={(ids) => void handleInstallGroup(ids)}
                    onUninstall={handleUninstallVoice}
                    onPlaySample={playSample}
                    onOpenTry={(id) => setTryingVoiceId((prev) => (prev === id ? null : id))}
                    onSubmitTry={(id) => void handleTryVoice(id)}
                  />
                );
              })}
              {groupedVoices.length > 1 &&
                voiceSearch.trim().length === 0 &&
                voiceFilter === "all" && (
                  <button
                    onClick={() => {
                      const allOpen = groupedVoices.every((g) => expandedGroups.has(g.meta.key));
                      setExpandedGroups(
                        allOpen ? new Set() : new Set(groupedVoices.map((g) => g.meta.key)),
                      );
                    }}
                    className="mt-1 w-full rounded-full border border-fg/10 bg-fg/5 py-1 text-[11px] text-fg/55 transition hover:border-fg/20 hover:bg-fg/10"
                  >
                    {groupedVoices.every((g) => expandedGroups.has(g.meta.key))
                      ? t("voices.extra.kokoro.collapseAll")
                      : t("voices.extra.kokoro.expandAll")}
                  </button>
                )}
            </div>
          )}
        </section>

        </div>
        </div>
      </div>

      {/* Multi-select action bar */}
      {selectMode && selectedVoiceIds.size > 0 && (
        <div
          className={cn(
            "fixed inset-x-0 z-40 px-4 transition-all",
            playerActive
              ? "bottom-[calc(env(safe-area-inset-bottom)+72px)]"
              : "bottom-[calc(env(safe-area-inset-bottom)+12px)]",
          )}
        >
          <div className="mx-auto flex max-w-md items-center gap-2 rounded-full border border-fg/15 bg-surface-el/90 px-3 py-2 shadow-lg backdrop-blur">
            <span className="text-[12px] font-medium text-fg/85">
              {t("voices.extra.kokoro.selectedCount", { count: selectedVoiceIds.size })}
            </span>
            <span className="ml-auto text-[10px] text-fg/45">
              {[...selectedVoiceIds]
                .slice(0, 3)
                .map((id) => voiceDisplayName(id))
                .join(", ")}
              {selectedVoiceIds.size > 3 && ` +${selectedVoiceIds.size - 3}`}
            </span>
            <button
              onClick={() => void handleInstallSelected()}
              className="rounded-full border border-accent/40 bg-accent/20 px-3 py-1 text-[11px] font-semibold text-accent transition hover:border-accent/60 hover:bg-accent/30"
            >
              {t("common.buttons.install")}
            </button>
          </div>
        </div>
      )}

      {/* Mini player */}
      {playerActive && playerState.source && (
        <MiniPlayer
          source={playerState.source}
          playing={playerState.playing}
          currentTime={playerState.currentTime}
          duration={playerState.duration}
          onToggle={toggle}
          onStop={stop}
        />
      )}

      {/* Engine sheet */}
      <BottomMenu
        isOpen={engineSheetOpen}
        onClose={() => setEngineSheetOpen(false)}
        title={t("voices.extra.kokoro.engineTitle")}
      >
        <div className="space-y-3">
          <div className="flex items-center gap-3 rounded-xl border border-fg/10 bg-fg/[0.03] px-3.5 py-3">
            <div className="flex h-11 w-11 shrink-0 items-center justify-center rounded-xl bg-accent/15 text-accent">
              <Cpu className="h-5 w-5" />
            </div>
            <div className="min-w-0 flex-1">
              <div className="flex items-center gap-2">
                <span className="truncate text-[14px] font-semibold text-fg">
                  {variantInfo?.label ?? variant ?? t("voices.extra.kokoro.variantUnset")}
                </span>
                {modelInstalled ? (
                  <span className="inline-flex shrink-0 items-center gap-1 rounded-full border border-accent/30 bg-accent/10 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-accent/85">
                    <CheckCircle2 className="h-2.5 w-2.5" />
                    {t("voices.extra.kokoro.installed")}
                  </span>
                ) : (
                  <span className="inline-flex shrink-0 items-center gap-1 rounded-full border border-warning/30 bg-warning/10 px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider text-warning/85">
                    <AlertTriangle className="h-2.5 w-2.5" />
                    {t("voices.extra.kokoro.notInstalled")}
                  </span>
                )}
              </div>
              <p className="mt-0.5 truncate font-mono text-[11px] text-fg/40">
                {variantInfo?.filename ?? "—"}
              </p>
            </div>
          </div>

          {variants.length > 1 && (
            <div className="flex items-center justify-between gap-3 rounded-xl border border-fg/10 bg-fg/5 px-3.5 py-2.5">
              <span className="text-[12px] font-medium text-fg/70">
                {t("voices.extra.kokoro.variant")}
              </span>
              <select
                value={variant ?? ""}
                onChange={(e) => void handleVariantChange(e.target.value as KokoroModelVariant)}
                disabled={isSwitchingVariant}
                className="min-w-0 max-w-[60%] truncate rounded-lg border border-fg/10 bg-fg/8 px-2.5 py-1 text-right text-[12px] text-fg/85 focus:border-fg/25 focus:outline-none disabled:opacity-60"
              >
                {variants.map((v) => (
                  <option key={v.id} value={v.id} className="bg-surface-el">
                    {v.label} · {v.sizeMb} MB
                  </option>
                ))}
              </select>
            </div>
          )}

          {storageStats && (
            <div className="grid grid-cols-3 gap-2">
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-2 py-3 text-center">
                <p className="text-[14px] font-semibold tabular-nums text-fg">
                  {formatBytes(storageStats.modelBytes)}
                </p>
                <p className="mt-0.5 text-[10px] uppercase tracking-wide text-fg/40">
                  {t("voices.extra.kokoro.model")}
                </p>
              </div>
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-2 py-3 text-center">
                <p className="text-[14px] font-semibold tabular-nums text-fg">
                  {formatBytes(storageStats.voicesBytes)}
                </p>
                <p className="mt-0.5 text-[10px] uppercase tracking-wide text-fg/40">
                  {t("voices.extra.kokoro.voiceCount", { count: storageStats.voiceCount })}
                </p>
              </div>
              <div className="rounded-xl border border-fg/10 bg-fg/5 px-2 py-3 text-center">
                <p className="text-[14px] font-semibold tabular-nums text-fg">
                  {formatBytes(storageStats.totalBytes)}
                </p>
                <p className="mt-0.5 text-[10px] uppercase tracking-wide text-fg/40">
                  {t("voices.extra.kokoro.total")}
                </p>
              </div>
            </div>
          )}

          <div className="flex items-center gap-2.5 rounded-xl border border-fg/10 bg-fg/5 px-3.5 py-2.5">
            <HardDrive className="h-3.5 w-3.5 shrink-0 text-fg/40" />
            <div className="min-w-0 flex-1">
              <p className="text-[10px] uppercase tracking-wide text-fg/40">
                {t("voices.extra.kokoro.assetRoot")}
              </p>
              <p className="truncate font-mono text-[11px] text-fg/60" title={assetRoot}>
                {assetRoot || "—"}
              </p>
            </div>
          </div>

          {modelInstalled && (
            <MenuButton
              icon={Trash2}
              title={t("voices.extra.kokoro.deleteModel")}
              description={t("voices.extra.kokoro.deleteModelDescription")}
              onClick={() => {
                setEngineSheetOpen(false);
                void handleDeleteModel();
              }}
              disabled={isDeletingModel}
              color="from-danger to-danger/80"
            />
          )}
        </div>
      </BottomMenu>

      {/* Blend actions */}
      <BottomMenu
        isOpen={!!actionBlend}
        onClose={() => setActionBlend(null)}
        title={actionBlend?.name || t("voices.extra.kokoro.blend")}
      >
        {actionBlend && (
          <div className="space-y-3">
            <MenuButton
              icon={Play}
              title={t("common.labels.preview")}
              description={t("voices.extra.kokoro.previewDescription")}
              onClick={() => {
                const blend = actionBlend;
                setActionBlend(null);
                void handlePlayBlend(blend);
              }}
              color="from-accent to-accent/80"
            />
            <MenuButton
              icon={Volume2}
              title={t("voices.extra.kokoro.editBlend")}
              description={t("voices.extra.kokoro.editBlendDescription")}
              onClick={() => {
                const id = actionBlend.id;
                setActionBlend(null);
                navigate(`/settings/voices/kokoro/${providerId}/blend/${id}`);
              }}
              color="from-info to-info/80"
            />
            <MenuButton
              icon={Trash2}
              title={t("common.buttons.delete")}
              description={t("voices.extra.kokoro.deleteBlendDescription")}
              onClick={() => void handleDeleteBlend(actionBlend.id)}
              color="from-danger to-danger/80"
            />
          </div>
        )}
      </BottomMenu>
    </div>
  );
}

function Pill({
  children,
  icon,
  tone = "neutral",
  dot = false,
}: {
  children: React.ReactNode;
  icon?: React.ReactNode;
  tone?: "neutral" | "accent" | "warning" | "info";
  dot?: boolean;
}) {
  const toneClasses = {
    neutral: "border-fg/10 bg-fg/5 text-fg/70",
    accent: "border-accent/30 bg-accent/10 text-accent/85",
    warning: "border-warning/30 bg-warning/10 text-warning/85",
    info: "border-info/30 bg-info/10 text-info/85",
  } as const;
  const dotClasses = {
    neutral: "bg-fg/40",
    accent: "bg-accent",
    warning: "bg-warning",
    info: "bg-info",
  } as const;
  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[11px] font-medium",
        toneClasses[tone],
      )}
    >
      {icon}
      {dot && <span className={cn("h-1.5 w-1.5 rounded-full", dotClasses[tone])} />}
      {children}
    </span>
  );
}

function SectionHeader({
  label,
  right,
}: {
  label: string;
  right?: React.ReactNode;
}) {
  return (
    <div className="mb-2.5 flex items-center justify-between gap-3 px-1">
      <h2
        className={cn(
          typography.overline.size,
          typography.overline.weight,
          typography.overline.tracking,
          typography.overline.transform,
          "text-fg/40",
        )}
      >
        {label}
      </h2>
      {right}
    </div>
  );
}


function FilterPill({
  active,
  onClick,
  label,
  count,
}: {
  active: boolean;
  onClick: () => void;
  label: string;
  count: number;
}) {
  return (
    <button
      onClick={onClick}
      className={cn(
        "rounded-full px-2.5 py-1 text-[11px] font-medium transition",
        active ? "bg-fg/15 text-fg" : "text-fg/55 hover:text-fg/80",
      )}
    >
      {label}
      <span className={cn("ml-1.5 tabular-nums", active ? "text-fg/65" : "text-fg/35")}>
        {count}
      </span>
    </button>
  );
}

function BlendRow({
  blend,
  providerId,
  isPreviewing,
  isPlaying,
  onPlay,
  onMore,
  onSpeedChange,
}: {
  blend: UserVoice;
  providerId: string;
  isPreviewing: boolean;
  isPlaying: boolean;
  onPlay: () => void;
  onMore: () => void;
  onSpeedChange: (next: number) => void;
}) {
  const { t } = useI18n();
  const navigate = useNavigate();
  const entries = parseBlend(blend.voiceId);
  const totalWeight = entries.reduce((sum, e) => sum + e.weight, 0) || 1;
  const segments = entries.map((e) => ({
    voiceId: e.voiceId,
    pct: (e.weight / totalWeight) * 100,
    tint: tintForVoice(e.voiceId),
  }));
  const summary =
    entries.length === 0
      ? t("kokoroStudio.noVoices")
      : entries
          .slice(0, 3)
          .map((e) => voiceDisplayName(e.voiceId))
          .join(" · ") + (entries.length > 3 ? ` +${entries.length - 3}` : "");

  const initialSpeed = readSpeed(blend.prompt);
  const [speed, setSpeed] = useState(initialSpeed);
  useEffect(() => setSpeed(initialSpeed), [initialSpeed]);
  const debounceRef = useRef<ReturnType<typeof setTimeout> | null>(null);
  const updateSpeed = (next: number) => {
    setSpeed(next);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => onSpeedChange(next), 400);
  };
  useEffect(() => () => {
    if (debounceRef.current) clearTimeout(debounceRef.current);
  }, []);

  return (
    <div className="rounded-xl border border-fg/10 bg-fg/5 transition hover:border-fg/20 hover:bg-fg/10">
      <div className="flex items-start gap-3 px-3 py-2.5">
        <button
          onClick={() => navigate(`/settings/voices/kokoro/${providerId}/blend/${blend.id}`)}
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/10"
          aria-label={t("kokoroStudio.edit")}
        >
          <Volume2 className="h-3.5 w-3.5 text-fg/55" />
        </button>
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <button
              onClick={() => navigate(`/settings/voices/kokoro/${providerId}/blend/${blend.id}`)}
              className="truncate text-left text-[13px] font-semibold text-fg hover:underline"
            >
              {blend.name}
            </button>
            <span className="shrink-0 text-[10px] text-fg/35">
              {t(
                entries.length === 1
                  ? "kokoroStudio.voiceCountInline"
                  : "kokoroStudio.voiceCountInlinePlural",
                { count: entries.length },
              )}
            </span>
          </div>
          <div className="mt-0.5 truncate text-[11px] text-fg/50">{summary}</div>
          {segments.length > 0 && (
            <div className="mt-1.5 flex h-0.5 w-full overflow-hidden rounded-full bg-fg/10">
              {segments.map((seg, i) => (
                <div
                  key={`${seg.voiceId}-${i}`}
                  className={cn("h-full", seg.tint)}
                  style={{ width: `${seg.pct}%`, opacity: 0.7 }}
                />
              ))}
            </div>
          )}
          <div className="mt-2 flex items-center gap-2">
            <span className="text-[10px] text-fg/40">{t("voices.extra.kokoro.speed")}</span>
            <input
              type="range"
              min={0.5}
              max={2}
              step={0.05}
              value={speed}
              onChange={(e) => updateSpeed(Number(e.target.value))}
              className="h-1 flex-1 accent-accent"
            />
            <span className="font-mono text-[10px] tabular-nums text-fg/55">
              {speed.toFixed(2)}×
            </span>
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-1">
          <button
            onClick={onPlay}
            disabled={isPreviewing && !isPlaying}
            className={cn(
              "flex h-7 w-7 items-center justify-center rounded-full border transition",
              isPlaying
                ? "border-accent/40 bg-accent/15 text-accent"
                : "border-fg/10 bg-fg/5 text-fg/65 hover:border-accent/30 hover:bg-accent/10 hover:text-accent",
              isPreviewing && !isPlaying && "opacity-50",
            )}
            title={t("kokoroStudio.preview")}
          >
            {isPreviewing && !isPlaying ? (
              <Loader2 className="h-3 w-3 animate-spin" />
            ) : isPlaying ? (
              <Square className="h-3 w-3" />
            ) : (
              <Play className="h-3 w-3" />
            )}
          </button>
          <button
            onClick={onMore}
            className="flex h-7 w-7 items-center justify-center rounded-full border border-fg/10 bg-fg/5 text-fg/55 transition hover:border-fg/25 hover:bg-fg/10"
            title={t("kokoroStudio.more")}
          >
            <MoreHorizontal className="h-3 w-3" />
          </button>
        </div>
      </div>
    </div>
  );
}

function VoiceGroupBlock({
  group,
  installedVoices,
  activeQueue,
  expanded,
  installedInGroup,
  selectMode,
  selectedVoiceIds,
  nowPlayingId,
  previewingId,
  tryingVoiceId,
  tryText,
  onTryTextChange,
  onToggleSelect,
  onToggle,
  onInstall,
  onInstallGroup,
  onUninstall,
  onPlaySample,
  onOpenTry,
  onSubmitTry,
}: {
  group: { meta: VoiceGroup; voices: KokoroAvailableVoice[] };
  installedVoices: KokoroInstalledVoice[];
  activeQueue: QueuedDownload[];
  expanded: boolean;
  installedInGroup: number;
  selectMode: boolean;
  selectedVoiceIds: Set<string>;
  nowPlayingId: string | null;
  previewingId: string | null;
  tryingVoiceId: string | null;
  tryText: string;
  onTryTextChange: (text: string) => void;
  onToggleSelect: (voiceId: string) => void;
  onToggle: () => void;
  onInstall: (voiceId: string) => void;
  onInstallGroup: (voiceIds: string[]) => void;
  onUninstall: (voiceId: string) => void;
  onPlaySample: (voiceId: string) => void;
  onOpenTry: (voiceId: string) => void;
  onSubmitTry: (voiceId: string) => void;
}) {
  const { t } = useI18n();
  const uninstalledIds = group.voices
    .filter((v) => !installedVoices.some((iv) => iv.id === v.id))
    .map((v) => v.id);
  const groupQueuing = group.voices.some((v) =>
    activeQueue.some((item) => item.installKind === "voice" && item.voiceId === v.id),
  );
  const showInstallAll = !selectMode && uninstalledIds.length > 0;

  return (
    <div className="overflow-hidden rounded-lg border border-fg/10 bg-fg/3">
      <div className="group/row flex items-center transition hover:bg-fg/6">
        <button
          onClick={onToggle}
          className="flex min-w-0 flex-1 items-center gap-2 px-3 py-2 text-left"
        >
          <ChevronDown
            className={cn(
              "h-3 w-3 shrink-0 text-fg/40 transition-transform",
              !expanded && "-rotate-90",
            )}
          />
          <span
            className={cn(
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              typography.overline.transform,
              "flex-1 truncate text-fg/55",
            )}
          >
            {t(group.meta.labelKey)}
          </span>
          {installedInGroup > 0 && (
            <span className="rounded-full border border-accent/25 bg-accent/10 px-1.5 py-0.5 text-[9px] font-semibold tabular-nums text-accent/85">
              {installedInGroup}
            </span>
          )}
          <span className="text-[10px] tabular-nums text-fg/35">{group.voices.length}</span>
        </button>
        {showInstallAll && (
          <button
            onClick={(e) => {
              e.stopPropagation();
              onInstallGroup(uninstalledIds);
            }}
            disabled={groupQueuing}
            title={t(
              uninstalledIds.length === 1
                ? "kokoroStudio.installVoiceTooltip"
                : "kokoroStudio.installVoiceTooltipPlural",
              { count: uninstalledIds.length },
            )}
            className="mr-2 flex shrink-0 items-center gap-1 rounded-full border border-info/25 bg-info/10 px-2 py-0.5 text-[9px] font-semibold tabular-nums text-info opacity-70 transition hover:border-info/40 hover:bg-info/15 hover:opacity-100 group-hover/row:opacity-100 disabled:opacity-50"
          >
            {groupQueuing ? (
              <Loader2 className="h-2.5 w-2.5 animate-spin" />
            ) : (
              <Download className="h-2.5 w-2.5" />
            )}
            +{uninstalledIds.length}
          </button>
        )}
      </div>
      {expanded && (
        <div className="border-t border-fg/10">
          {group.voices.map((voice) => {
            const installed = installedVoices.some((iv) => iv.id === voice.id);
            const queued = activeQueue.some(
              (item) => item.installKind === "voice" && item.voiceId === voice.id,
            );
            const hasPreview = Boolean(VOICE_PREVIEW_URLS[voice.id]);
            const isPlayingSample = nowPlayingId === `sample:${voice.id}`;
            const isTryOpen = tryingVoiceId === voice.id;
            const isTrying = previewingId === `try:${voice.id}`;
            const isPlayingTry = nowPlayingId === `try:${voice.id}`;
            const isSelected = selectedVoiceIds.has(voice.id);

            return (
              <div
                key={voice.id}
                className={cn(
                  "border-t border-fg/5 first:border-t-0",
                  selectMode && installed && "opacity-50",
                )}
              >
                <div className="flex items-center gap-2 bg-surface px-3 py-1.5">
                  {selectMode &&
                    (installed ? (
                      <CheckCircle2
                        className="h-4 w-4 shrink-0 text-accent/70"
                        aria-label={t("kokoroStudio.alreadyInstalled")}
                      />
                    ) : (
                      <button
                        onClick={() => onToggleSelect(voice.id)}
                        className={cn(
                          "flex h-4 w-4 shrink-0 items-center justify-center rounded border transition",
                          isSelected
                            ? "border-accent/60 bg-accent/30 text-accent"
                            : "border-fg/20 bg-transparent",
                        )}
                        aria-label={isSelected ? t("kokoroStudio.deselect") : t("kokoroStudio.select")}
                      >
                        {isSelected && <Check className="h-2.5 w-2.5" strokeWidth={3} />}
                      </button>
                    ))}
                  <div className="min-w-0 flex-1">
                    <div className="flex items-center gap-1.5">
                      {installed && !selectMode && (
                        <CheckCircle2 className="h-3 w-3 shrink-0 text-accent/80" />
                      )}
                      <span className="truncate text-[12px] font-medium text-fg/90">
                        {voiceDisplayName(voice.id)}
                      </span>
                    </div>
                    <div className="truncate font-mono text-[9.5px] text-fg/30">{voice.id}</div>
                  </div>

                  {!selectMode && hasPreview && (
                    <button
                      onClick={() => onPlaySample(voice.id)}
                      title={isPlayingSample ? t("kokoroStudio.stopSample") : t("kokoroStudio.playSample")}
                      className={cn(
                        "flex h-6 w-6 shrink-0 items-center justify-center rounded-full border transition",
                        isPlayingSample
                          ? "border-accent/40 bg-accent/15 text-accent"
                          : "border-fg/10 bg-fg/5 text-fg/55 hover:border-fg/25 hover:bg-fg/10 hover:text-fg/85",
                      )}
                    >
                      {isPlayingSample ? (
                        <Square className="h-2.5 w-2.5" />
                      ) : (
                        <Play className="h-2.5 w-2.5" />
                      )}
                    </button>
                  )}

                  {!selectMode && installed && (
                    <button
                      onClick={() => onOpenTry(voice.id)}
                      title={t("kokoroStudio.tryWithCustomText")}
                      className={cn(
                        "flex h-6 w-6 shrink-0 items-center justify-center rounded-full border transition",
                        isTryOpen || isPlayingTry
                          ? "border-info/40 bg-info/15 text-info"
                          : "border-fg/10 bg-fg/5 text-fg/55 hover:border-fg/25 hover:bg-fg/10 hover:text-fg/85",
                      )}
                    >
                      <Wand2 className="h-2.5 w-2.5" />
                    </button>
                  )}

                  {!selectMode &&
                    (installed ? (
                      <VoiceRowActions
                        onUninstall={() => onUninstall(voice.id)}
                        onReinstall={() => onInstall(voice.id)}
                      />
                    ) : (
                      <button
                        onClick={() => onInstall(voice.id)}
                        disabled={queued}
                        className="shrink-0 rounded-full border border-info/25 bg-info/10 px-2 py-0.5 text-[10px] font-semibold text-info transition hover:border-info/40 hover:bg-info/15 disabled:opacity-50"
                      >
                        {queued ? <Loader2 className="h-3 w-3 animate-spin" /> : t("kokoroStudio.install")}
                      </button>
                    ))}
                </div>

                {!selectMode && installed && isTryOpen && (
                  <div className="flex items-center gap-2 border-t border-fg/5 bg-fg/3 px-3 py-2">
                    <input
                      value={tryText}
                      onChange={(e) => onTryTextChange(e.target.value)}
                      placeholder={t("kokoroStudio.tryWithCustomText")}
                      className="min-w-0 flex-1 rounded-md border border-fg/10 bg-surface-el/30 px-2 py-1 text-[11px] text-fg placeholder-fg/35 focus:border-fg/25 focus:outline-none"
                    />
                    <button
                      onClick={() => onSubmitTry(voice.id)}
                      disabled={isTrying || !tryText.trim()}
                      className="flex shrink-0 items-center gap-1 rounded-full border border-accent/30 bg-accent/10 px-2.5 py-1 text-[10px] font-semibold text-accent transition hover:border-accent/50 hover:bg-accent/15 disabled:opacity-50"
                    >
                      {isTrying ? (
                        <Loader2 className="h-3 w-3 animate-spin" />
                      ) : (
                        <Sparkles className="h-3 w-3" />
                      )}
                      {t("kokoroStudio.speak")}
                    </button>
                  </div>
                )}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

function VoiceRowActions({
  onReinstall,
  onUninstall,
}: {
  onReinstall: () => void;
  onUninstall: () => void;
}) {
  const { t } = useI18n();
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
        className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/5 text-fg/55 transition hover:border-fg/25 hover:bg-fg/10"
        aria-label={t("kokoroStudio.more")}
      >
        <MoreHorizontal className="h-3 w-3" />
      </button>
      {open && (
        <div className="absolute right-0 top-full z-20 mt-1 w-32 overflow-hidden rounded-lg border border-fg/15 bg-surface-el/95 shadow-lg backdrop-blur">
          <button
            onMouseDown={(e) => {
              e.preventDefault();
              setOpen(false);
              onReinstall();
            }}
            className="flex w-full items-center gap-2 px-3 py-2 text-left text-[11px] text-fg/85 transition hover:bg-fg/10"
          >
            <RotateCw className="h-3 w-3" />
            {t("kokoroStudio.reinstall")}
          </button>
          <button
            onMouseDown={(e) => {
              e.preventDefault();
              setOpen(false);
              onUninstall();
            }}
            className="flex w-full items-center gap-2 px-3 py-2 text-left text-[11px] text-danger/85 transition hover:bg-danger/10"
          >
            <Trash2 className="h-3 w-3" />
            {t("kokoroStudio.delete")}
          </button>
        </div>
      )}
    </div>
  );
}

function MiniPlayer({
  source,
  playing,
  currentTime,
  duration,
  onToggle,
  onStop,
}: {
  source: PlayerSource;
  playing: boolean;
  currentTime: number;
  duration: number;
  onToggle: () => void;
  onStop: () => void;
}) {
  const { t } = useI18n();
  const pct = duration > 0 ? (currentTime / duration) * 100 : 0;
  return (
    <div className="fixed inset-x-0 bottom-0 z-50 border-t border-fg/10 bg-surface-el/85 px-4 py-2 pb-[calc(env(safe-area-inset-bottom)+8px)] backdrop-blur lg:left-[var(--settings-sidebar-w,0px)]">
      <div className="mx-auto flex max-w-2xl items-center gap-3">
        <button
          onClick={onToggle}
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-accent/40 bg-accent/15 text-accent transition hover:border-accent/60 hover:bg-accent/25"
          aria-label={playing ? t("kokoroStudio.pause") : t("kokoroStudio.play")}
        >
          {playing ? <Pause className="h-3.5 w-3.5" /> : <Play className="h-3.5 w-3.5" />}
        </button>
        <div className="min-w-0 flex-1">
          <div className="flex items-baseline gap-2">
            <span className="truncate text-[12px] font-medium text-fg">{source.label}</span>
            {source.sublabel && (
              <span className="shrink-0 text-[10px] text-fg/45">{source.sublabel}</span>
            )}
          </div>
          <div className="mt-0.5 flex items-center gap-2">
            <span className="font-mono text-[10px] tabular-nums text-fg/45">
              {formatTime(currentTime)}
            </span>
            <div className="h-0.5 flex-1 overflow-hidden rounded-full bg-fg/10">
              <div
                className="h-full bg-accent/70 transition-all"
                style={{ width: `${Math.min(100, pct)}%` }}
              />
            </div>
            <span className="font-mono text-[10px] tabular-nums text-fg/45">
              {formatTime(duration)}
            </span>
          </div>
        </div>
        <button
          onClick={onStop}
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/5 text-fg/65 transition hover:border-fg/25 hover:bg-fg/10"
          aria-label={t("kokoroStudio.stop")}
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}
