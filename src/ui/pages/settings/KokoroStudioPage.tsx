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
import { cn, typography } from "../../design-tokens";


const LOCALE_GROUPS: Record<string, { label: string; order: number }> = {
  af: { label: "American · Female", order: 10 },
  am: { label: "American · Male", order: 11 },
  bf: { label: "British · Female", order: 20 },
  bm: { label: "British · Male", order: 21 },
  ef: { label: "Spanish · Female", order: 30 },
  em: { label: "Spanish · Male", order: 31 },
  ff: { label: "French · Female", order: 40 },
  fm: { label: "French · Male", order: 41 },
  hf: { label: "Hindi · Female", order: 50 },
  hm: { label: "Hindi · Male", order: 51 },
  if: { label: "Italian · Female", order: 60 },
  im: { label: "Italian · Male", order: 61 },
  jf: { label: "Japanese · Female", order: 70 },
  jm: { label: "Japanese · Male", order: 71 },
  pf: { label: "Portuguese · Female", order: 80 },
  pm: { label: "Portuguese · Male", order: 81 },
  zf: { label: "Mandarin · Female", order: 90 },
  zm: { label: "Mandarin · Male", order: 91 },
};

type VoiceGroup = { key: string; label: string; order: number };

function classifyVoice(id: string): VoiceGroup {
  const prefix = id.slice(0, 2).toLowerCase();
  const meta = LOCALE_GROUPS[prefix];
  if (meta) return { key: prefix, ...meta };
  return { key: "other", label: "Other", order: 999 };
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
  const [tryText, setTryText] = useState("Hello! This is a quick test of how I sound.");

  const [experimentSourceId, setExperimentSourceId] = useState<string>("");
  const [experimentText, setExperimentText] = useState(
    "Hello! This is how I sound when speaking. I can read longer passages with warmth, clarity, and emotion.",
  );
  const [experimentSpeed, setExperimentSpeed] = useState(1);
  const [isExperimenting, setIsExperimenting] = useState(false);

  const [isDeletingModel, setIsDeletingModel] = useState(false);
  const [isSwitchingVariant, setIsSwitchingVariant] = useState(false);

  const [setupOpen, setSetupOpen] = useState(false);
  const initialSetupShownRef = useRef(false);

  const { queue, cancelItem, dismissItem } = useDownloadQueue();
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
        setProvider(list.find((p) => p.id === providerId) ?? null);
        setVariants(supported);
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

  const groupedActiveQueue = useMemo(() => {
    const groups = new Map<string, QueuedDownload[]>();
    for (const item of activeQueue) {
      const key = item.installId ?? item.id;
      const list = groups.get(key);
      if (list) list.push(item);
      else groups.set(key, [item]);
    }
    return [...groups.entries()].map(([id, items]) => ({ id, items }));
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
  const isInstallingAnyVoice = activeQueue.some((item) => item.installKind === "voice");
  const hasVoices = installedVoices.length > 0;
  const setupNeeded = !modelInstalled || !hasVoices;
  const modelDownload = useMemo(
    () => activeQueue.find((item) => item.installKind === "model"),
    [activeQueue],
  );

  useEffect(() => {
    if (providerLoading || !provider) return;
    if (initialSetupShownRef.current) return;
    if (setupNeeded) {
      setSetupOpen(true);
      initialSetupShownRef.current = true;
    } else {
      initialSetupShownRef.current = true;
    }
  }, [providerLoading, provider, setupNeeded]);

  useEffect(() => {
    if (setupOpen && modelInstalled && hasVoices) {
      setSetupOpen(false);
    }
  }, [setupOpen, modelInstalled, hasVoices]);

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

  const handleInstallModel = async () => {
    if (!variant || !assetRoot) return;
    setError(null);
    try {
      await kokoroInstallModel(assetRoot, variant);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
    }
  };

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

  const handleInstallVariant = async (next: KokoroModelVariant) => {
    if (!provider || !assetRoot) return;
    setError(null);
    try {
      if (next !== variant) {
        const updated = await upsertAudioProvider({ ...provider, kokoroVariant: next });
        setProvider(updated);
      }
      await kokoroInstallModel(assetRoot, next);
    } catch (e) {
      setError(e instanceof Error ? e.message : String(e));
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
      sublabel: "Sample",
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
      "Live preview",
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
      "Saved blend",
      entries,
      "Hello! This is a quick preview of how this voice sounds.",
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
        "Experiment",
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
        <p className="text-sm text-fg/60">Kokoro provider not found</p>
        <button
          onClick={() => navigate("/settings/providers?tab=audio")}
          className="rounded-lg border border-fg/10 bg-fg/5 px-4 py-2 text-sm text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
        >
          Back to providers
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
            <span>{variantInfo?.label ?? variant ?? "Variant unset"}</span>
            <span className="text-fg/30">·</span>
            <span className="text-fg/55">
              {storageStats ? formatBytes(storageStats.modelBytes) : variantInfo ? `${variantInfo.sizeMb} MB` : "—"}
            </span>
          </Pill>
          <Pill tone={modelInstalled ? "accent" : "warning"} dot={modelInstalled}>
            {modelInstalled ? "Ready" : "Model not installed"}
          </Pill>
          {storageStats && storageStats.voiceCount > 0 && (
            <Pill icon={<HardDrive className="h-3 w-3 text-fg/45" />}>
              <span>
                {storageStats.voiceCount} voice{storageStats.voiceCount === 1 ? "" : "s"}
              </span>
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
              Refresh
            </button>
            <button
              onClick={() => setEngineSheetOpen(true)}
              className="flex h-7 items-center justify-center rounded-full border border-fg/10 bg-fg/5 px-2 text-fg/65 transition hover:border-fg/20 hover:bg-fg/10"
              aria-label="Engine actions"
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
              aria-label="Dismiss"
            >
              <X className="h-3.5 w-3.5" />
            </button>
          </div>
        )}

        {setupNeeded && (
          <button
            onClick={() => setSetupOpen(true)}
            className={cn(
              "group flex w-full items-center gap-3 rounded-xl border px-4 py-3 text-left transition",
              !modelInstalled
                ? "border-warning/25 bg-warning/[0.06] hover:border-warning/40 hover:bg-warning/[0.1]"
                : "border-info/25 bg-info/[0.06] hover:border-info/40 hover:bg-info/[0.1]",
            )}
          >
            <div
              className={cn(
                "flex h-8 w-8 shrink-0 items-center justify-center rounded-full",
                !modelInstalled
                  ? "bg-warning/15 text-warning"
                  : "bg-info/15 text-info",
              )}
            >
              {!modelInstalled ? (
                <AlertTriangle className="h-4 w-4" />
              ) : (
                <Download className="h-4 w-4" />
              )}
            </div>
            <div className="min-w-0 flex-1">
              <p className="text-[13px] font-semibold text-fg">
                {!modelInstalled ? "Engine not installed" : "Install at least one voice"}
              </p>
              <p className="mt-0.5 truncate text-[11px] text-fg/55">
                {!modelInstalled
                  ? "Continue setup to install the Kokoro model."
                  : "Pick a voice or grab the starter pack to begin."}
              </p>
            </div>
            <span className="shrink-0 text-[11px] font-semibold text-fg/65 group-hover:text-fg/90">
              Continue →
            </span>
          </button>
        )}

        {/* Failed downloads */}
        {failedQueue.length > 0 && (
          <div className="rounded-lg border border-danger/25 bg-danger/8 px-3 py-2">
            <div className="mb-2 flex items-center justify-between gap-3">
              <div className="min-w-0">
                <p className="text-[13px] font-medium text-danger/90">
                  {failedQueue.length} download{failedQueue.length === 1 ? "" : "s"} failed
                </p>
                <p className="text-[11px] text-danger/65">Retry individually or dismiss all.</p>
              </div>
              <button
                onClick={() => void handleDismissFailed()}
                className="shrink-0 rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] text-fg/65 transition hover:border-fg/20 hover:bg-fg/10"
              >
                Dismiss all
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
                        ? `Model · ${item.variant ?? ""}`
                        : `Voice · ${voiceDisplayName(item.voiceId ?? "")}`}
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
                    aria-label="Dismiss"
                  >
                    <X className="h-3 w-3" />
                  </button>
                </div>
              ))}
            </div>
          </div>
        )}

        {/* Active downloads (grouped) */}
        {groupedActiveQueue.length > 0 && (
          <section>
            <SectionHeader
              label="Downloads"
              right={
                <button
                  onClick={() => void Promise.all(activeQueue.map((i) => cancelItem(i.id)))}
                  className="rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] text-fg/65 transition hover:border-fg/20 hover:bg-fg/10"
                >
                  Cancel all
                </button>
              }
            />
            <div className="space-y-1.5">
              {groupedActiveQueue.map((group) => (
                <DownloadGroupCard
                  key={group.id}
                  items={group.items}
                  onCancelItem={cancelItem}
                />
              ))}
            </div>
          </section>
        )}

        <div className="grid min-h-0 gap-6 lg:grid-cols-[minmax(0,440px)_minmax(0,1fr)]">
        <div className="min-w-0 space-y-6">

        {/* Experiment */}
        {modelInstalled && experimentOptions.length > 0 && (
          <section>
            <SectionHeader label="Experiment" />
            <div className="overflow-hidden rounded-xl border border-fg/10 bg-fg/[0.04]">
              <textarea
                value={experimentText}
                onChange={(e) => setExperimentText(e.target.value)}
                rows={3}
                placeholder="Type a phrase to hear it spoken…"
                className="w-full resize-none border-0 bg-transparent px-4 pt-3 pb-1 text-[13px] leading-relaxed text-fg placeholder-fg/35 focus:outline-none"
              />
              <div className="flex flex-wrap items-center gap-2 border-t border-fg/10 bg-fg/[0.02] px-3 py-2">
                <select
                  value={experimentSourceId}
                  onChange={(e) => setExperimentSourceId(e.target.value)}
                  className="min-w-0 max-w-[200px] truncate rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] font-medium text-fg/80 focus:border-fg/25 focus:outline-none"
                >
                  {experimentOptions
                    .filter((o) => o.kind === "blend")
                    .map((o) => (
                      <option key={o.key} value={o.key} className="bg-surface-el">
                        Blend · {o.label}
                      </option>
                    ))}
                  {experimentOptions
                    .filter((o) => o.kind === "voice")
                    .map((o) => (
                      <option key={o.key} value={o.key} className="bg-surface-el">
                        Voice · {o.label}
                      </option>
                    ))}
                </select>
                <div className="flex flex-1 items-center gap-2 rounded-full border border-fg/10 bg-fg/5 px-3 py-1">
                  <span className="text-[10px] uppercase tracking-wider text-fg/40">Speed</span>
                  <input
                    type="range"
                    min={0.5}
                    max={2}
                    step={0.05}
                    value={experimentSpeed}
                    onChange={(e) => setExperimentSpeed(Number(e.target.value))}
                    className="h-1 min-w-[80px] flex-1 accent-accent"
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
                  Speak
                </button>
              </div>
            </div>
          </section>
        )}

        {/* Saved blends */}
        <section>
          <SectionHeader
            label="Your blends"
            right={
              <button
                onClick={() => navigate(`/settings/voices/kokoro/${providerId}/blend`)}
                disabled={!modelInstalled || !hasVoices}
                className="flex items-center gap-1 rounded-full border border-accent/30 bg-accent/10 px-2.5 py-1 text-[11px] font-medium text-accent/85 transition hover:border-accent/50 hover:bg-accent/15 disabled:opacity-40"
              >
                <Plus className="h-3 w-3" />
                New blend
              </button>
            }
          />
          {savedBlends.length === 0 ? (
            <div className="rounded-xl border border-dashed border-fg/15 bg-fg/[0.025] px-4 py-6 text-center text-[12px] text-fg/55">
              {modelInstalled && hasVoices
                ? "No saved blends yet."
                : "Install the model and a voice first."}
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
            <SectionHeader label="Featured" />
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
                          ? "border-accent/40 bg-accent/[0.08]"
                          : "border-fg/10 bg-fg/[0.04] hover:border-fg/25 hover:bg-fg/[0.07]",
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
                        aria-label={isPlayingSample ? "Stop" : "Sample"}
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
                        <p className="truncate text-[10px] text-fg/45">{meta.label}</p>
                      </div>
                      {!isInstalled && (
                        <button
                          onClick={() => void handleInstallVoice(voice.id)}
                          disabled={isQueued}
                          className="shrink-0 rounded-full border border-info/25 bg-info/10 px-2 py-0.5 text-[10px] font-semibold text-info transition hover:border-info/40 hover:bg-info/15 disabled:opacity-60"
                        >
                          {isQueued ? <Loader2 className="h-3 w-3 animate-spin" /> : "Install"}
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
            label="Voice library"
            right={
              modelInstalled && (
                <div className="flex items-center gap-1.5">
                  {!hasVoices && availableVoices.length > 0 && (
                    <button
                      onClick={() => void handleInstallStarterPack()}
                      className="flex items-center gap-1 rounded-full border border-accent/30 bg-accent/10 px-2.5 py-1 text-[11px] font-medium text-accent/85 transition hover:border-accent/50 hover:bg-accent/15"
                    >
                      <Sparkles className="h-3 w-3" />
                      Starter pack
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
                      Cancel
                    </button>
                  ) : (
                    <button
                      onClick={() => setSelectMode(true)}
                      className="rounded-full border border-fg/10 bg-fg/5 px-2.5 py-1 text-[11px] text-fg/65 transition hover:border-fg/20 hover:bg-fg/10"
                    >
                      Select
                    </button>
                  )}
                </div>
              )
            }
          />

          <div className="mb-2.5 flex flex-wrap items-center gap-2">
            <div className="relative flex-1 min-w-[180px]">
              <Search className="pointer-events-none absolute left-3 top-1/2 h-3.5 w-3.5 -translate-y-1/2 text-fg/35" />
              <input
                value={voiceSearch}
                onChange={(e) => setVoiceSearch(e.target.value)}
                disabled={!modelInstalled}
                placeholder="Search voices"
                className="w-full rounded-full border border-fg/10 bg-fg/5 py-1.5 pl-8 pr-3 text-[12px] text-fg placeholder-fg/35 focus:border-fg/25 focus:outline-none disabled:opacity-50"
              />
            </div>
            <div className="flex items-center rounded-full border border-fg/10 bg-fg/5 p-0.5">
              <FilterPill
                active={voiceFilter === "all"}
                onClick={() => setVoiceFilter("all")}
                label="All"
                count={availableVoices.length}
              />
              <FilterPill
                active={voiceFilter === "installed"}
                onClick={() => setVoiceFilter("installed")}
                label="Installed"
                count={installedVoices.length}
              />
            </div>
          </div>

          {!modelInstalled ? (
            <div className="rounded-xl border border-dashed border-fg/15 bg-fg/[0.025] px-4 py-6 text-center text-[12px] text-fg/55">
              Install the model to browse voices.
            </div>
          ) : groupedVoices.length === 0 ? (
            <div className="rounded-xl border border-dashed border-fg/15 bg-fg/[0.025] px-4 py-6 text-center text-[12px] text-fg/55">
              {availableVoices.length === 0
                ? "No voices in catalog. Tap Refresh."
                : "No voices match your filters."}
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
                      ? "Collapse all"
                      : "Expand all"}
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
              {selectedVoiceIds.size} selected
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
              Install
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
        title="Kokoro engine"
      >
        <div className="space-y-3">
          <div className="space-y-1.5">
            <div className="flex items-center justify-between gap-3 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2">
              <span className="text-[11px] text-fg/45">Variant</span>
              <select
                value={variant ?? ""}
                onChange={(e) =>
                  void handleVariantChange(e.target.value as KokoroModelVariant)
                }
                disabled={isSwitchingVariant || variants.length <= 1}
                className="min-w-0 max-w-[60%] truncate rounded-md border border-fg/10 bg-fg/[0.06] px-2 py-1 text-right text-[12px] text-fg/85 focus:border-fg/25 focus:outline-none disabled:opacity-60"
              >
                {variants.map((v) => (
                  <option key={v.id} value={v.id} className="bg-surface-el">
                    {v.label} · {v.sizeMb} MB
                  </option>
                ))}
              </select>
            </div>
            <DetailRow
              label="Status"
              value={
                modelInstalled ? (
                  <span className="inline-flex items-center gap-1 text-accent/85">
                    <CheckCircle2 className="h-3 w-3" />
                    Installed
                  </span>
                ) : (
                  <span className="inline-flex items-center gap-1 text-warning/85">
                    <AlertTriangle className="h-3 w-3" />
                    Not installed
                  </span>
                )
              }
            />
            <DetailRow label="File" value={variantInfo?.filename ?? "—"} mono />
            {storageStats && (
              <>
                <DetailRow
                  label="Model size"
                  value={formatBytes(storageStats.modelBytes)}
                />
                <DetailRow
                  label="Voices size"
                  value={`${storageStats.voiceCount} · ${formatBytes(storageStats.voicesBytes)}`}
                />
                <DetailRow label="Total" value={formatBytes(storageStats.totalBytes)} />
              </>
            )}
            <DetailRow label="Asset root" value={assetRoot || "—"} mono />
          </div>
          <MenuButton
            icon={Download}
            title={modelInstalled ? "Reinstall model" : "Install model"}
            description={variantInfo ? `${variantInfo.label} · ${variantInfo.sizeMb} MB` : ""}
            onClick={() => {
              setEngineSheetOpen(false);
              void handleInstallModel();
            }}
            disabled={isInstallingModel || !variant || !assetRoot}
            color="from-info to-info/80"
          />
          {modelInstalled && (
            <MenuButton
              icon={Trash2}
              title="Delete model"
              description="Frees up disk; voices are kept."
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
        title={actionBlend?.name || "Blend"}
      >
        {actionBlend && (
          <div className="space-y-3">
            <MenuButton
              icon={Play}
              title="Preview"
              description="Quick listen with default text"
              onClick={() => {
                const blend = actionBlend;
                setActionBlend(null);
                void handlePlayBlend(blend);
              }}
              color="from-accent to-accent/80"
            />
            <MenuButton
              icon={Volume2}
              title="Edit blend"
              description="Adjust voices, weights, and speed"
              onClick={() => {
                const id = actionBlend.id;
                setActionBlend(null);
                navigate(`/settings/voices/kokoro/${providerId}/blend/${id}`);
              }}
              color="from-info to-info/80"
            />
            <MenuButton
              icon={Trash2}
              title="Delete"
              description="Remove this saved voice"
              onClick={() => void handleDeleteBlend(actionBlend.id)}
              color="from-danger to-danger/80"
            />
          </div>
        )}
      </BottomMenu>

      {/* Setup wizard */}
      <BottomMenu
        isOpen={setupOpen}
        onClose={() => setSetupOpen(false)}
        title="Set up Kokoro"
      >
        {!modelInstalled ? (
          <SetupModelStep
            variants={variants}
            currentVariant={variant}
            isInstalling={isInstallingModel}
            modelDownload={modelDownload}
            onInstall={(v) => void handleInstallVariant(v)}
            onCancel={(id) => void cancelItem(id)}
            onBack={() => {
              setSetupOpen(false);
              navigate("/settings/providers?tab=audio");
            }}
          />
        ) : !hasVoices ? (
          <SetupVoicesStep
            isInstalling={isInstallingAnyVoice}
            onInstallStarter={() => void handleInstallStarterPack()}
            onBrowse={() => setSetupOpen(false)}
          />
        ) : (
          <div className="flex flex-col items-center gap-3 py-4 text-center">
            <div className="flex h-12 w-12 items-center justify-center rounded-full border border-accent/30 bg-accent/15">
              <CheckCircle2 className="h-6 w-6 text-accent" />
            </div>
            <p className="text-[14px] font-semibold text-fg">All set</p>
            <p className="text-[12px] text-fg/55">
              Browse voices, design blends, or test in the experiment area.
            </p>
            <button
              onClick={() => setSetupOpen(false)}
              className="mt-1 rounded-full bg-accent px-5 py-1.5 text-[12px] font-semibold text-bg transition hover:brightness-110"
            >
              Done
            </button>
          </div>
        )}
      </BottomMenu>
    </div>
  );
}

const VARIANT_COPY: Record<
  KokoroModelVariant,
  { headline: string; description: string; tag: string }
> = {
  int8: {
    headline: "Phone-friendly",
    description:
      "Best when you're chatting on the go. Light on battery, quick to start a conversation, and natural enough for most character voices.",
    tag: "Mobile",
  },
  fp16: {
    headline: "Recommended for most",
    description:
      "The sweet spot for daily roleplay on a desktop or laptop. Noticeably warmer voices than int8 with no real wait between replies.",
    tag: "Recommended",
  },
  fp32: {
    headline: "Studio quality",
    description:
      "The most lifelike characters, with the richest emotion and detail. Slower to synthesize and heavier on memory; best saved for a capable computer.",
    tag: "High-end",
  },
};

function SetupModelStep({
  variants,
  currentVariant,
  isInstalling,
  modelDownload,
  onInstall,
  onCancel,
  onBack,
}: {
  variants: KokoroSupportedVariant[];
  currentVariant?: KokoroModelVariant;
  isInstalling: boolean;
  modelDownload?: QueuedDownload;
  onInstall: (variant: KokoroModelVariant) => void;
  onCancel: (id: string) => void;
  onBack: () => void;
}) {
  const installingVariant = (modelDownload?.variant ?? null) as KokoroModelVariant | null;

  return (
    <div className="space-y-4">
      <div className="flex items-start gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-accent/30 bg-accent/15">
          <Download className="h-5 w-5 text-accent" />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-[14px] font-semibold text-fg">Install the engine</p>
          <p className="mt-0.5 text-[12px] text-fg/55">
            Kokoro runs entirely on this device. Pick the variant that fits your hardware — you can
            switch later.
          </p>
        </div>
      </div>

      <div className="space-y-2">
        {variants.map((v) => {
          const copy = VARIANT_COPY[v.id];
          const isThisInstalling = isInstalling && installingVariant === v.id;
          const pct =
            isThisInstalling && modelDownload && modelDownload.total > 0
              ? (modelDownload.downloaded / modelDownload.total) * 100
              : 0;
          const isSelected = currentVariant === v.id;
          const disabled = isInstalling && !isThisInstalling;
          return (
            <button
              key={v.id}
              onClick={() => onInstall(v.id)}
              disabled={disabled}
              className={cn(
                "group relative w-full overflow-hidden rounded-xl border px-4 py-3 text-left transition disabled:opacity-50",
                isThisInstalling
                  ? "border-info/40 bg-info/[0.08]"
                  : isSelected
                    ? "border-accent/35 bg-accent/[0.08] hover:border-accent/55"
                    : "border-fg/10 bg-fg/[0.04] hover:border-fg/25 hover:bg-fg/[0.07]",
              )}
            >
              <div className="flex items-start justify-between gap-3">
                <div className="min-w-0">
                  <div className="flex items-center gap-2">
                    <span className="text-[13px] font-semibold text-fg">{v.label}</span>
                    {copy && (
                      <span
                        className={cn(
                          "rounded-full border px-1.5 py-0.5 text-[9px] font-bold uppercase tracking-wider",
                          v.id === "fp16"
                            ? "border-accent/30 bg-accent/10 text-accent/85"
                            : "border-fg/15 bg-fg/5 text-fg/55",
                        )}
                      >
                        {copy.tag}
                      </span>
                    )}
                  </div>
                  {copy && (
                    <p className="mt-1 text-[11px] leading-relaxed text-fg/55">
                      {copy.description}
                    </p>
                  )}
                </div>
                <div className="shrink-0 text-right">
                  <span className="text-[12px] font-semibold tabular-nums text-fg/85">
                    {v.sizeMb} MB
                  </span>
                  <p className="font-mono text-[9px] text-fg/35">{v.filename}</p>
                </div>
              </div>

              {isThisInstalling && (
                <div className="mt-2.5">
                  <div className="flex items-center justify-between gap-2 text-[10px]">
                    <span className="flex items-center gap-1 text-info/80">
                      <Loader2 className="h-2.5 w-2.5 animate-spin" />
                      Installing
                    </span>
                    <span className="tabular-nums text-fg/55">{pct.toFixed(0)}%</span>
                  </div>
                  <div className="mt-1 h-0.5 overflow-hidden rounded-full bg-fg/10">
                    <div
                      className="h-full bg-info/80 transition-all"
                      style={{ width: `${Math.max(2, pct)}%` }}
                    />
                  </div>
                </div>
              )}
            </button>
          );
        })}
      </div>

      <div className="flex gap-2 pt-1">
        <button
          onClick={onBack}
          className="flex-1 rounded-full border border-fg/10 bg-fg/5 px-4 py-2 text-[12px] font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
        >
          Cancel
        </button>
        {isInstalling && modelDownload && (
          <button
            onClick={() => onCancel(modelDownload.id)}
            className="flex-1 rounded-full border border-danger/25 bg-danger/10 px-4 py-2 text-[12px] font-semibold text-danger transition hover:border-danger/40 hover:bg-danger/15"
          >
            Cancel install
          </button>
        )}
      </div>
    </div>
  );
}

function SetupVoicesStep({
  isInstalling,
  onInstallStarter,
  onBrowse,
}: {
  isInstalling: boolean;
  onInstallStarter: () => void;
  onBrowse: () => void;
}) {
  return (
    <div className="space-y-4">
      <div className="flex items-start gap-3">
        <div className="flex h-10 w-10 shrink-0 items-center justify-center rounded-xl border border-info/30 bg-info/15">
          <Sparkles className="h-5 w-5 text-info" />
        </div>
        <div className="min-w-0 flex-1">
          <p className="text-[14px] font-semibold text-fg">Pick your first voices</p>
          <p className="mt-0.5 text-[12px] text-fg/55">
            You need at least one voice to start synthesizing.
          </p>
        </div>
      </div>

      <div className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-3">
        <p className="text-[12px] font-semibold text-fg">Starter pack</p>
        <p className="mt-0.5 text-[11px] text-fg/55">
          Heart, Adam, Emma, George — a balanced set of US and UK voices.
        </p>
        <button
          onClick={onInstallStarter}
          disabled={isInstalling}
          className="mt-3 inline-flex w-full items-center justify-center gap-2 rounded-full bg-accent px-4 py-2 text-[12px] font-semibold text-bg transition hover:brightness-110 disabled:opacity-50"
        >
          {isInstalling ? (
            <Loader2 className="h-3 w-3 animate-spin" />
          ) : (
            <Download className="h-3 w-3" />
          )}
          Install starter pack
        </button>
      </div>

      <button
        onClick={onBrowse}
        className="w-full rounded-full border border-fg/10 bg-fg/5 px-4 py-2 text-[12px] font-medium text-fg/70 transition hover:border-fg/20 hover:bg-fg/10"
      >
        Browse voices manually
      </button>
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

function DetailRow({
  label,
  value,
  mono,
}: {
  label: string;
  value: React.ReactNode;
  mono?: boolean;
}) {
  return (
    <div className="flex items-center justify-between gap-3 rounded-lg border border-fg/10 bg-fg/5 px-3 py-2">
      <span className="text-[11px] text-fg/45">{label}</span>
      <span
        className={cn(
          "min-w-0 truncate text-right text-[12px] text-fg/85",
          mono && "font-mono text-[11px]",
        )}
      >
        {value}
      </span>
    </div>
  );
}

function DownloadGroupCard({
  items,
  onCancelItem,
}: {
  items: QueuedDownload[];
  onCancelItem: (id: string) => Promise<void>;
}) {
  const aggDownloaded = items.reduce((sum, i) => sum + i.downloaded, 0);
  const aggTotal = items.reduce((sum, i) => sum + i.total, 0);
  const aggPct = aggTotal > 0 ? (aggDownloaded / aggTotal) * 100 : 0;
  const completed = items.filter((i) => i.status === "complete").length;

  const head = items[0];
  const headLabel =
    head.installKind === "model"
      ? `Model · ${head.variant ?? ""}`.trim()
      : items.length > 1
        ? `Voices · ${items.length}`
        : `Voice · ${voiceDisplayName(head.voiceId ?? "")}`.trim();

  const [expanded, setExpanded] = useState(false);

  return (
    <div className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-2">
      <button
        onClick={() => setExpanded((v) => !v)}
        className="flex w-full items-center justify-between gap-3 text-left"
      >
        <div className="min-w-0 flex-1">
          <div className="truncate text-[11px] text-fg/85">{headLabel}</div>
          <div className="text-[10px] text-fg/45">
            {completed}/{items.length} files
          </div>
        </div>
        <div className="flex shrink-0 items-center gap-2">
          <span className="text-[11px] tabular-nums text-fg/50">{aggPct.toFixed(0)}%</span>
          <ChevronDown
            className={cn(
              "h-3 w-3 text-fg/40 transition-transform",
              expanded && "rotate-180",
            )}
          />
        </div>
      </button>
      <div className="mt-1.5 h-0.5 overflow-hidden rounded-full bg-fg/10">
        <div
          className="h-full bg-info/70 transition-all"
          style={{ width: `${Math.max(2, aggPct)}%` }}
        />
      </div>
      {expanded && (
        <div className="mt-2 space-y-1.5 border-t border-fg/10 pt-2">
          {items.map((item) => {
            const pct = item.total > 0 ? (item.downloaded / item.total) * 100 : 0;
            return (
              <div key={item.id} className="space-y-1">
                <div className="flex items-center justify-between gap-2 text-[10px]">
                  <span className="truncate font-mono text-fg/55">{item.filename}</span>
                  <div className="flex items-center gap-1.5">
                    <span className="tabular-nums text-fg/45">{pct.toFixed(0)}%</span>
                    {(item.status === "downloading" || item.status === "queued") && (
                      <button
                        onClick={() => void onCancelItem(item.id)}
                        className="text-fg/40 transition hover:text-danger"
                        aria-label="Cancel"
                      >
                        <X className="h-3 w-3" />
                      </button>
                    )}
                  </div>
                </div>
                <div className="h-px overflow-hidden rounded-full bg-fg/10">
                  <div
                    className="h-full bg-info/60 transition-all"
                    style={{ width: `${Math.max(2, pct)}%` }}
                  />
                </div>
              </div>
            );
          })}
        </div>
      )}
    </div>
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
      ? "No voices"
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
          aria-label="Edit"
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
              {entries.length} voice{entries.length === 1 ? "" : "s"}
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
            <span className="text-[10px] text-fg/40">Speed</span>
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
            title="Preview"
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
            title="More"
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
  const uninstalledIds = group.voices
    .filter((v) => !installedVoices.some((iv) => iv.id === v.id))
    .map((v) => v.id);
  const groupQueuing = group.voices.some((v) =>
    activeQueue.some((item) => item.installKind === "voice" && item.voiceId === v.id),
  );
  const showInstallAll = !selectMode && uninstalledIds.length > 0;

  return (
    <div className="overflow-hidden rounded-lg border border-fg/10 bg-fg/[0.03]">
      <div className="group/row flex items-center transition hover:bg-fg/[0.06]">
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
            {group.meta.label}
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
            title={`Install ${uninstalledIds.length} voice${uninstalledIds.length === 1 ? "" : "s"}`}
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
                        aria-label="Already installed"
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
                        aria-label={isSelected ? "Deselect" : "Select"}
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
                      title={isPlayingSample ? "Stop sample" : "Play sample"}
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
                      title="Try with custom text"
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
                        {queued ? <Loader2 className="h-3 w-3 animate-spin" /> : "Install"}
                      </button>
                    ))}
                </div>

                {!selectMode && installed && isTryOpen && (
                  <div className="flex items-center gap-2 border-t border-fg/5 bg-fg/[0.03] px-3 py-2">
                    <input
                      value={tryText}
                      onChange={(e) => onTryTextChange(e.target.value)}
                      placeholder="Try with custom text"
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
                      Speak
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
  const [open, setOpen] = useState(false);
  return (
    <div className="relative">
      <button
        onClick={() => setOpen((v) => !v)}
        onBlur={() => setTimeout(() => setOpen(false), 150)}
        className="flex h-6 w-6 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/5 text-fg/55 transition hover:border-fg/25 hover:bg-fg/10"
        aria-label="More"
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
            Reinstall
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
            Delete
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
  const pct = duration > 0 ? (currentTime / duration) * 100 : 0;
  return (
    <div className="fixed inset-x-0 bottom-0 z-50 border-t border-fg/10 bg-surface-el/85 px-4 py-2 pb-[calc(env(safe-area-inset-bottom)+8px)] backdrop-blur">
      <div className="mx-auto flex max-w-2xl items-center gap-3">
        <button
          onClick={onToggle}
          className="flex h-8 w-8 shrink-0 items-center justify-center rounded-full border border-accent/40 bg-accent/15 text-accent transition hover:border-accent/60 hover:bg-accent/25"
          aria-label={playing ? "Pause" : "Play"}
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
          aria-label="Stop"
        >
          <X className="h-3.5 w-3.5" />
        </button>
      </div>
    </div>
  );
}
