import { useState, useEffect, useCallback, useRef, useMemo, type ReactNode } from "react";
import { useSearchParams, useNavigate } from "react-router-dom";
import { invoke } from "@tauri-apps/api/core";
import {
  Search,
  Download,
  Heart,
  ArrowDownToLine,
  Loader,
  X,
  Cpu,
  BookOpen,
  Layers,
  TrendingUp,
  Clock,
  ThumbsUp,
  AlertTriangle,
  FileText,
  ExternalLink,
  Monitor,
  Info,
  ArrowRight,
  Server,
  Check,
  HardDrive,
  ChevronDown,
  SlidersHorizontal,
  ChevronLeft,
  Users,
} from "lucide-react";
import { motion, AnimatePresence } from "framer-motion";

import { cn, typography, interactive } from "../../design-tokens";
import { useI18n, type TranslationKey, type TranslateParams } from "../../../core/i18n/context";

type TFn = (key: TranslationKey, params?: TranslateParams) => string;
import { Switch } from "../../components/Switch";
import { HfReadmeRenderer } from "./components/HfReadmeRenderer";
import { useImageBundle } from "./components/imageBundle/useImageBundle";
import { BundleArchitecturePicker } from "./components/imageBundle/BundleArchitecturePicker";
import { BundleReview } from "./components/imageBundle/BundleReview";
import { BundleFilePicker } from "./components/imageBundle/BundleFilePicker";
import { HfTokenMenu, isHfGatedError } from "./components/HfTokenMenu";
import { formatBundleBytes, type BundleRole, type ValidatedAsset } from "./components/imageBundle/types";
import { InlineDownloadCards } from "./components/DownloadQueueBar";
import {
  useDownloadQueue,
  type QueuedDownload,
} from "../../../core/downloads/DownloadQueueContext";
import { BottomMenu, MenuLabel, MenuDivider } from "../../components/BottomMenu";
import { GuidedTour, useGuidedTour } from "../../components/GuidedTour";
import { toast } from "../../components/toast";
import {
  addOrUpdateModel,
  readSettings,
  readSettingsCached,
  SETTINGS_UPDATED_EVENT,
} from "../../../core/storage/repo";
import { createDefaultAdvancedModelSettings } from "../../../core/storage/schemas";
import type { ProviderCredential } from "../../../core/storage/schemas";
import { openExternalUrl } from "../../../core/utils/openExternal";
import { getPlatform } from "../../../core/utils/platform";
import OllamaIcon from "../../../assets/ollama_light.png";

interface HfSearchResult {
  modelId: string;
  author: string;
  likes: number;
  downloads: number;
  tags: string[];
  pipelineTag: string | null;
  lastModified: string | null;
  trendingScore: number | null;
}

interface HfAuthorOverview {
  name: string;
  fullname: string | null;
  avatarUrl: string | null;
  details: string | null;
  kind: string | null;
  isPro: boolean;
  numModels: number;
  numDatasets: number;
  numSpaces: number;
  numLikes: number;
  numFollowers: number;
  numFollowing: number;
  createdAt: string | null;
}

interface HfModelFile {
  filename: string;
  size: number;
  quantization: string;
  isMmproj: boolean;
  isMtp: boolean;
  role?: string | null;
}

interface RunabilityScore {
  filename: string;
  score: number;
  label: "excellent" | "good" | "marginal" | "poor" | "unrunnable";
  fitsInRam: boolean;
  fitsInVram: boolean;
  gpuMode: string;
}

interface FileRecommendation {
  filename: string;
  size: number;
  quantization: string;
  quantQuality: number;
  maxContextF16: number;
  maxContextQ80: number;
  maxContextQ40: number;
  /** Max context for 100% GPU offload (Q8_0 KV). 0 if model doesn't fit. */
  optimalGpuCtx: number;
  /** Max context for total RAM+VRAM before swapping (Q8_0 KV). */
  optimalRamCtx: number;
}

interface BestRecommendation {
  filename: string;
  contextLength: number;
  kvType: string;
  score: number;
  viable: boolean;
}

interface ModelArchInfo {
  nextnPredictLayers: number | null;
  architecture: string | null;
  blockCount: number | null;
  embeddingLength: number | null;
  headCount: number | null;
  headCountKv: number | null;
  contextLength: number | null;
  feedForwardLength: number | null;
  fileType: number | null;
  slidingWindow: number | null;
  kvLoraRank: number | null;
  keyLength: number | null;
  valueLength: number | null;
  incompleteParse: boolean;
}

interface RecommendationData {
  availableRam: number;
  availableVram: number;
  supportsGpuOffload: boolean;
  unifiedMemory: boolean;
  totalAvailable: number;
  kvBasePerToken: number | null;
  kvContextCap: number | null;
  modelMaxContext: number;
  arch: ModelArchInfo | null;
  files: FileRecommendation[];
  best: BestRecommendation | null;
}

const KV_BPV: Record<string, number> = {
  f32: 4.0,
  f16: 2.0,
  q8_0: 1.0,
  q5_1: 0.6875,
  q5_0: 0.625,
  q4_1: 0.5625,
  q4_0: 0.5,
  iq4_nl: 0.5,
};

function dynamicSafetyReserve(totalSystemMemory: number): number {
  return Math.min(Math.max(totalSystemMemory * 0.1, 512_000_000), 2_000_000_000);
}

/** Compute max context for a given file size + KV BPV dynamically */
function maxContextForBpv(
  fileSize: number,
  kvBasePerToken: number | null,
  bpv: number,
  totalAvailable: number,
  modelMaxCtx: number,
  sidecarReserveBytes = 0,
): number {
  if (!kvBasePerToken || kvBasePerToken <= 0) return modelMaxCtx;
  const safety = dynamicSafetyReserve(totalAvailable);
  const overhead = computeOverhead(fileSize);
  const remaining = Math.max(
    totalAvailable - fileSize - overhead - sidecarReserveBytes - safety,
    0,
  );
  const bytesPerToken = kvBasePerToken * bpv;
  if (bytesPerToken <= 0) return modelMaxCtx;
  const maxCtx = Math.floor(remaining / bytesPerToken);
  return Math.min(Math.max(maxCtx, 0), modelMaxCtx);
}

/** Compute buffer overhead: max(modelSize × 5%, 200MB) */
function computeOverhead(modelSize: number): number {
  return Math.max(modelSize * 0.05, 200_000_000);
}

function computeGpuOptimalContext(
  fileSize: number,
  kvBasePerToken: number | null,
  bpv: number,
  availableVram: number,
  modelMaxCtx: number,
  kvContextCap: number | null,
  sidecarReserveBytes = 0,
): number {
  if (availableVram <= 0 || !kvBasePerToken) return 0;
  const vramBudget = Math.max(availableVram * 0.9 - sidecarReserveBytes, 0);
  const overhead = computeOverhead(fileSize);
  if (fileSize + overhead >= vramBudget) return 0;
  const vramForKv = vramBudget - fileSize - overhead;
  const rawCtx = Math.floor(vramForKv / (kvBasePerToken * bpv));
  if (kvContextCap && rawCtx >= kvContextCap) return modelMaxCtx;
  return rawCtx >= 512 ? Math.min(rawCtx, modelMaxCtx) : 0;
}

function computeRamMaxContext(
  fileSize: number,
  kvBasePerToken: number | null,
  bpv: number,
  totalAvailable: number,
  modelMaxCtx: number,
  kvContextCap: number | null,
  sidecarReserveBytes = 0,
): number {
  if (!kvBasePerToken) return 0;
  const safety = dynamicSafetyReserve(totalAvailable);
  const overhead = computeOverhead(fileSize);
  const remaining = Math.max(
    totalAvailable - fileSize - overhead - sidecarReserveBytes - safety,
    0,
  );
  const rawCtx = Math.floor(remaining / (kvBasePerToken * bpv));
  if (kvContextCap && rawCtx >= kvContextCap) return modelMaxCtx;
  return rawCtx >= 512 ? Math.min(rawCtx, modelMaxCtx) : 0;
}

function calcScore(
  modelSize: number,
  quantQuality: number,
  kvCacheBytes: number,
  availableRam: number,
  totalAvailable: number,
  availableVram: number,
  modelOffload: ModelOffload = "auto",
  kvPlacement: KvPlacement = "auto",
  sidecarReserveBytes = 0,
): { score: number; label: string; fitsVram: boolean; gpuMode: string; gpuScore: number } {
  const overhead = computeOverhead(modelSize);
  const totalNeeded = modelSize + kvCacheBytes + overhead + sidecarReserveBytes;
  const ramBudget = availableRam * 0.9;
  const vramBudget = availableVram * 0.9;
  const modelFitsRam = availableRam > 0 && modelSize + overhead <= ramBudget;
  const kvFitsVram = availableVram > 0 && kvCacheBytes + overhead <= vramBudget;
  const kvFitsRam = availableRam > 0 && kvCacheBytes + overhead <= ramBudget;
  const kvOnVramRequested = kvPlacement === "vram";
  const kvOnRamRequested = kvPlacement === "ram";
  const effectiveVramNeed =
    modelSize + overhead + sidecarReserveBytes + (kvOnRamRequested ? 0 : kvCacheBytes);

  // Memory fitness (25%)
  let memoryScore: number;
  if (totalAvailable === 0) memoryScore = 50;
  else if (totalNeeded > totalAvailable) memoryScore = 0;
  else {
    const r = totalAvailable / totalNeeded;
    memoryScore = r < 1.2 ? 20 : r < 1.5 ? 50 : r < 2.0 ? 70 : r < 3.0 ? 85 : 100;
  }

  const scoreForPlacement = (
    placement: Exclude<ModelOffload, "auto">,
  ): { score: number; fitsVram: boolean; gpuMode: string; priority: number } => {
    if (placement === "cpu") {
      if (!modelFitsRam) {
        return {
          score: 0,
          fitsVram: false,
          gpuMode: kvOnVramRequested && availableVram > 0 ? "ramModelVramCtx" : "cpu",
          priority: 0,
        };
      }
      const ramFitRatio = Math.min(ramBudget / (modelSize + overhead), 1.0);
      const baseScore = kvOnVramRequested && kvFitsVram && availableVram > 0 ? 82 : 68;
      return {
        score: baseScore + ramFitRatio * (kvOnVramRequested ? 10 : 14),
        fitsVram: false,
        gpuMode: kvOnVramRequested && availableVram > 0 ? "ramModelVramCtx" : "ramModelRamCtx",
        priority: 0,
      };
    }

    if (availableVram <= 0) {
      return {
        score: 0,
        fitsVram: false,
        gpuMode: placement === "gpu" ? "gpuUnavailable" : "cpu",
        priority: placement === "gpu" ? 2 : 1,
      };
    }

    if (placement === "gpu") {
      if (modelSize === 0) {
        return { score: 10, fitsVram: false, gpuMode: "gpuUnavailable", priority: 2 };
      }
      if (modelSize > vramBudget) {
        return { score: 8, fitsVram: false, gpuMode: "gpuUnavailable", priority: 2 };
      }
      if (kvOnRamRequested) {
        const ramCtxFitRatio =
          kvCacheBytes + overhead > 0 ? Math.min(ramBudget / (kvCacheBytes + overhead), 1.0) : 1.0;
        return {
          score: 80 + ramCtxFitRatio * (kvFitsRam ? 12 : 4),
          fitsVram: false,
          gpuMode: kvFitsRam ? "kvSpill" : "kvHeavySpill",
          priority: 2,
        };
      }
      if (effectiveVramNeed <= vramBudget) {
        return { score: 100, fitsVram: true, gpuMode: "full", priority: 2 };
      }
      const remaining = vramBudget - modelSize;
      const spill = kvCacheBytes + overhead;
      const fitRatio = spill > 0 ? Math.min(remaining / spill, 1.0) : 1.0;
      let adjustedFitRatio = fitRatio;
      if (kvOnVramRequested && !kvFitsVram) adjustedFitRatio *= 0.55;
      if (kvOnRamRequested) adjustedFitRatio = Math.max(adjustedFitRatio, 0.7);
      return {
        score: 72 + adjustedFitRatio * 23,
        fitsVram: true,
        gpuMode:
          adjustedFitRatio >= 0.8
            ? "nearFull"
            : adjustedFitRatio >= 0.4
              ? "kvSpill"
              : "kvHeavySpill",
        priority: 2,
      };
    }

    if (kvOnRamRequested && modelSize <= vramBudget) {
      const ramCtxFitRatio =
        kvCacheBytes + overhead > 0 ? Math.min(ramBudget / (kvCacheBytes + overhead), 1.0) : 1.0;
      return {
        score: Math.max(12, 78 + ramCtxFitRatio * (kvFitsRam ? 14 : 6)),
        fitsVram: false,
        gpuMode: kvFitsRam ? "kvSpill" : "kvHeavySpill",
        priority: 1,
      };
    }
    if (effectiveVramNeed <= vramBudget) {
      return { score: 96, fitsVram: true, gpuMode: "full", priority: 1 };
    }
    if (modelSize === 0) {
      return { score: 10, fitsVram: false, gpuMode: "cpu", priority: 1 };
    }
    const offloadRatio = Math.min(vramBudget / modelSize, 1.0);
    const kvPenalty = kvOnVramRequested && !kvFitsVram ? 12 : 0;
    const kvBonus = kvOnRamRequested ? 4 : 0;
    return {
      score: Math.max(12, 28 + offloadRatio * 54 + kvBonus - kvPenalty),
      fitsVram: false,
      gpuMode:
        offloadRatio >= 0.75
          ? "mostLayers"
          : offloadRatio >= 0.5
            ? "halfLayers"
            : offloadRatio >= 0.2
              ? "fewLayers"
              : "cpu",
      priority: 1,
    };
  };

  const placementResult =
    modelOffload === "auto"
      ? (["gpu", "mixed", "cpu"] as const)
          .map((placement) => {
            const result = scoreForPlacement(placement);
            return { ...result, adjustedScore: result.score + result.priority * 4 };
          })
          .sort((a, b) => b.adjustedScore - a.adjustedScore)[0]
      : { ...scoreForPlacement(modelOffload), adjustedScore: 0 };

  const gpuScore = placementResult.score;
  const fitsVram = placementResult.fitsVram;
  const gpuMode = placementResult.gpuMode;

  // KV headroom (15%)
  let kvScore: number;
  const headroom = Math.max(totalAvailable - modelSize - overhead, 0);
  if (kvCacheBytes === 0) kvScore = 50;
  else if (headroom === 0) kvScore = 0;
  else if (headroom >= kvCacheBytes) {
    const r = headroom / kvCacheBytes;
    kvScore = r >= 2.0 ? 100 : 50 + 50 * (r - 1.0);
  } else {
    kvScore = 50 * (headroom / kvCacheBytes);
  }

  let raw = memoryScore * 0.25 + gpuScore * 0.35 + kvScore * 0.15 + quantQuality * 0.25;
  if (memoryScore === 0) raw = Math.min(raw, 10);
  const score = Math.min(Math.round(raw), 100);
  const label =
    score >= 80
      ? "excellent"
      : score >= 60
        ? "good"
        : score >= 40
          ? "marginal"
          : score >= 20
            ? "poor"
            : "unrunnable";
  return { score, label, fitsVram, gpuMode, gpuScore };
}

function getRunabilityModeCopy(t: TFn, gpuMode: string): { short: string; long: string } {
  switch (gpuMode) {
    case "gpuUnavailable":
      return {
        short: t("hfBrowser.runMode.gpuUnavailableShort"),
        long: t("hfBrowser.runMode.gpuUnavailableLong"),
      };
    case "full":
      return {
        short: t("hfBrowser.runMode.fullShort"),
        long: t("hfBrowser.runMode.fullLong"),
      };
    case "nearFull":
      return {
        short: t("hfBrowser.runMode.nearFullShort"),
        long: t("hfBrowser.runMode.nearFullLong"),
      };
    case "kvSpill":
      return {
        short: t("hfBrowser.runMode.kvSpillShort"),
        long: t("hfBrowser.runMode.kvSpillLong"),
      };
    case "kvHeavySpill":
      return {
        short: t("hfBrowser.runMode.kvHeavySpillShort"),
        long: t("hfBrowser.runMode.kvHeavySpillLong"),
      };
    case "ramModelVramCtx":
      return {
        short: t("hfBrowser.runMode.ramModelVramCtxShort"),
        long: t("hfBrowser.runMode.ramModelVramCtxLong"),
      };
    case "ramModelRamCtx":
      return {
        short: t("hfBrowser.runMode.ramModelRamCtxShort"),
        long: t("hfBrowser.runMode.ramModelRamCtxLong"),
      };
    case "mostLayers":
      return {
        short: t("hfBrowser.runMode.mostLayersShort"),
        long: t("hfBrowser.runMode.mostLayersLong"),
      };
    case "halfLayers":
      return {
        short: t("hfBrowser.runMode.halfLayersShort"),
        long: t("hfBrowser.runMode.halfLayersLong"),
      };
    case "fewLayers":
      return {
        short: t("hfBrowser.runMode.fewLayersShort"),
        long: t("hfBrowser.runMode.fewLayersLong"),
      };
    case "cpu":
    default:
      return {
        short: t("hfBrowser.runMode.cpuShort"),
        long: t("hfBrowser.runMode.cpuLong"),
      };
  }
}

function getKvPlacementCopy(t: TFn, value: KvPlacement): { label: string; description: string } {
  switch (value) {
    case "ram":
      return {
        label: t("hfBrowser.kvPlacement.ramLabel"),
        description: t("hfBrowser.kvPlacement.ramDescription"),
      };
    case "vram":
      return {
        label: t("hfBrowser.kvPlacement.vramLabel"),
        description: t("hfBrowser.kvPlacement.vramDescription"),
      };
    case "auto":
    default:
      return {
        label: t("hfBrowser.kvPlacement.autoLabel"),
        description: t("hfBrowser.kvPlacement.autoDescription"),
      };
  }
}

function kvPlacementToOffloadKqv(value: KvPlacement): boolean | null {
  switch (value) {
    case "ram":
      return false;
    case "vram":
      return true;
    case "auto":
    default:
      return null;
  }
}

function getModelOffloadCopy(t: TFn, value: ModelOffload): { label: string; description: string } {
  switch (value) {
    case "cpu":
      return {
        label: t("hfBrowser.modelOffload.cpuLabel"),
        description: t("hfBrowser.modelOffload.cpuDescription"),
      };
    case "gpu":
      return {
        label: t("hfBrowser.modelOffload.gpuLabel"),
        description: t("hfBrowser.modelOffload.gpuDescription"),
      };
    case "mixed":
      return {
        label: t("hfBrowser.modelOffload.mixedLabel"),
        description: t("hfBrowser.modelOffload.mixedDescription"),
      };
    case "auto":
    default:
      return {
        label: t("hfBrowser.modelOffload.autoLabel"),
        description: t("hfBrowser.modelOffload.autoDescription"),
      };
  }
}

function getHeadroomStatus(
  t: TFn,
  totalNeeded: number,
  totalAvailable: number,
): {
  label: string;
  tone: string;
  description: string;
} {
  const remaining = totalAvailable - totalNeeded;
  if (totalAvailable <= 0 || totalNeeded > totalAvailable) {
    return {
      label: t("hfBrowser.headroom.riskyLabel"),
      tone: "text-red-400",
      description: t("hfBrowser.headroom.riskyDescription"),
    };
  }
  const headroomRatio = remaining / Math.max(totalAvailable, 1);
  if (remaining >= 4_000_000_000 || headroomRatio >= 0.25) {
    return {
      label: t("hfBrowser.headroom.comfortableLabel"),
      tone: "text-emerald-400",
      description: t("hfBrowser.headroom.comfortableDescription"),
    };
  }
  if (remaining >= 1_500_000_000 || headroomRatio >= 0.12) {
    return {
      label: t("hfBrowser.headroom.okLabel"),
      tone: "text-blue-400",
      description: t("hfBrowser.headroom.okDescription"),
    };
  }
  return {
    label: t("hfBrowser.headroom.tightLabel"),
    tone: "text-amber-400",
    description: t("hfBrowser.headroom.tightDescription"),
  };
}

function getRunStatus(t: TFn, score: number): { label: string; tone: string; description: string } {
  if (score >= 75) {
    return {
      label: t("hfBrowser.runStatus.yesLabel"),
      tone: "text-emerald-400",
      description: t("hfBrowser.runStatus.yesDescription"),
    };
  }
  if (score >= 55) {
    return {
      label: t("hfBrowser.runStatus.borderlineLabel"),
      tone: "text-amber-400",
      description: t("hfBrowser.runStatus.borderlineDescription"),
    };
  }
  return {
    label: t("hfBrowser.runStatus.noLabel"),
    tone: "text-red-400",
    description: t("hfBrowser.runStatus.noDescription"),
  };
}

function getPerformanceStatus(
  t: TFn,
  gpuMode: string,
): {
  prefill: { label: string; tone: string };
  generation: { label: string; tone: string };
} {
  const fast = t("hfBrowser.perf.fast");
  const medium = t("hfBrowser.perf.medium");
  const slow = t("hfBrowser.perf.slow");
  switch (gpuMode) {
    case "full":
      return {
        prefill: { label: fast, tone: "text-emerald-400" },
        generation: { label: fast, tone: "text-emerald-400" },
      };
    case "nearFull":
      return {
        prefill: { label: fast, tone: "text-emerald-400" },
        generation: { label: fast, tone: "text-emerald-400" },
      };
    case "kvSpill":
    case "ramModelVramCtx":
      return {
        prefill: { label: medium, tone: "text-blue-400" },
        generation: { label: medium, tone: "text-blue-400" },
      };
    case "kvHeavySpill":
    case "mostLayers":
    case "halfLayers":
      return {
        prefill: { label: slow, tone: "text-amber-400" },
        generation: { label: medium, tone: "text-blue-400" },
      };
    case "fewLayers":
    case "ramModelRamCtx":
    case "cpu":
    case "gpuUnavailable":
    default:
      return {
        prefill: { label: slow, tone: "text-amber-400" },
        generation: { label: slow, tone: "text-amber-400" },
      };
  }
}

function modelOffloadToGpuLayers(
  value: ModelOffload,
  totalLayers: number | null | undefined,
  mixedLayers: number | null,
): number | null {
  switch (value) {
    case "cpu":
      return 0;
    case "gpu":
      return totalLayers && totalLayers > 0 ? totalLayers : null;
    case "mixed":
      return mixedLayers != null && mixedLayers > 0 ? mixedLayers : null;
    case "auto":
    default:
      return null;
  }
}

function gpuLayersToModelOffload(
  llamaGpuLayers: number | null | undefined,
  totalLayers: number | null | undefined,
): ModelOffload {
  if (llamaGpuLayers == null) return "auto";
  if (llamaGpuLayers <= 0) return "cpu";
  if (totalLayers != null && totalLayers > 0 && llamaGpuLayers >= totalLayers) return "gpu";
  return "mixed";
}

interface HfModelInfo {
  modelId: string;
  author: string;
  likes: number;
  downloads: number;
  tags: string[];
  architecture: string | null;
  contextLength: number | null;
  parameterCount: number | null;
  files: HfModelFile[];
}

type SortMode = "trending" | "downloads" | "likes" | "lastModified";
type FilesPanelTab = "recommended" | "files";
type CompareSelection = {
  id: number;
  filename: string;
  kvType: string;
};
type TrackedDownloadRole = "model" | "mmproj" | "mtp";
type KvPlacement = "auto" | "ram" | "vram";
type ModelOffload = "auto" | "cpu" | "gpu" | "mixed";

function gpuOffloadLayerCount(blockCount: number | null | undefined): number | null {
  if (blockCount == null || blockCount <= 0) return null;
  return blockCount + 1;
}

type QueueDownloadMetadata = {
  createModelWhenFinished?: boolean;
  mmprojFile?: string | false;
  mtpFile?: string | false;
  mtpBundled?: boolean | null;
  installId?: string | null;
  displayName?: string | null;
  contextLength?: number | null;
  kvType?: string | null;
  llamaOffloadKqv?: boolean | null;
  llamaGpuLayers?: number | null;
  llamaModelOffloadMode?: ModelOffload | null;
  downloadRole?: TrackedDownloadRole | null;
  queueKind?: string | null;
  destinationPath?: string | null;
  variant?: string | null;
};

type ViewState =
  | { kind: "search" }
  | { kind: "model"; modelId: string }
  | { kind: "author"; author: string };

function formatBytes(bytes: number): string {
  if (bytes === 0) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const i = Math.floor(Math.log(bytes) / Math.log(1024));
  const value = bytes / Math.pow(1024, i);
  return `${value.toFixed(i > 1 ? 1 : 0)} ${units[i]}`;
}

function formatNumber(n: number): string {
  if (n >= 1_000_000) return `${(n / 1_000_000).toFixed(1)}M`;
  if (n >= 1_000) return `${(n / 1_000).toFixed(1)}K`;
  return n.toString();
}

function formatTimeAgo(isoDate: string): string {
  const now = Date.now();
  const then = new Date(isoDate).getTime();
  if (isNaN(then)) return isoDate;
  const diffMs = now - then;
  const mins = Math.floor(diffMs / 60_000);
  if (mins < 1) return "just now";
  if (mins < 60) return `${mins}m ago`;
  const hours = Math.floor(mins / 60);
  if (hours < 24) return `${hours}h ago`;
  const days = Math.floor(hours / 24);
  if (days < 30) return `${days}d ago`;
  const months = Math.floor(days / 30);
  if (months < 12) return `${months}mo ago`;
  const years = Math.floor(months / 12);
  return `${years}y ago`;
}

/** Try to extract a human-readable param size from tags or model name.
 *  Checks tags first (e.g. "7b"), then falls back to parsing the model name
 *  (e.g. "Qwen3.5-35B-A3B-GGUF" → "35B"). */
function extractParamSize(tags: string[], modelId: string): string | null {
  for (const tag of tags) {
    const lower = tag.toLowerCase();
    const match = lower.match(/^(\d+(?:\.\d+)?)(b|m|k|t)$/);
    if (match) {
      return `${match[1]}${match[2].toUpperCase()}`;
    }
  }
  const name = modelId.split("/").pop() || modelId;
  const nameMatch = name.match(/[-_](\d+(?:\.\d+)?)[Bb][-_]/);
  if (nameMatch) {
    return `${nameMatch[1]}B`;
  }
  const endMatch = name.match(/[-_](\d+(?:\.\d+)?)[Bb](?:-|$)/);
  if (endMatch) {
    return `${endMatch[1]}B`;
  }
  return null;
}

/** Convert a param size string ("7B", "120M", "1.5K") into a value in billions. */
function paramSizeToBillions(value: string | null): number | null {
  if (!value) return null;
  const match = value.match(/^(\d+(?:\.\d+)?)([KMBT])$/i);
  if (!match) return null;
  const numeric = Number(match[1]);
  const unit = match[2].toUpperCase();
  const multipliers: Record<string, number> = {
    K: 1e-6,
    M: 1e-3,
    B: 1,
    T: 1e3,
  };
  return numeric * multipliers[unit];
}

const COMMON_PIPELINE_TAGS = [
  "text-generation",
  "text-to-text",
  "image-text-to-text",
  "text-to-image",
  "text-to-video",
  "image-to-text",
  "automatic-speech-recognition",
  "text-to-speech",
  "feature-extraction",
  "sentence-similarity",
];

/** Generate a deterministic color from a string for avatar fallback */
function authorColor(name: string): string {
  const colors = [
    "bg-emerald-500/30",
    "bg-blue-500/30",
    "bg-violet-500/30",
    "bg-amber-500/30",
    "bg-rose-500/30",
    "bg-cyan-500/30",
    "bg-pink-500/30",
    "bg-teal-500/30",
  ];
  let hash = 0;
  for (let i = 0; i < name.length; i++) {
    hash = ((hash << 5) - hash + name.charCodeAt(i)) | 0;
  }
  return colors[Math.abs(hash) % colors.length];
}

function formatParams(count: number): string {
  const fmt = (n: number) => (n % 1 === 0 ? n.toFixed(0) : n.toFixed(1));
  if (count >= 1_000_000_000_000) return `${fmt(count / 1_000_000_000_000)}T`;
  if (count >= 1_000_000_000) return `${fmt(count / 1_000_000_000)}B`;
  if (count >= 1_000_000) return `${fmt(count / 1_000_000)}M`;
  if (count >= 1_000) return `${fmt(count / 1_000)}K`;
  return count.toString();
}

function extractModelShortName(modelId: string): string {
  const parts = modelId.split("/");
  return parts[parts.length - 1] || modelId;
}

function extractFileDisplayName(filename: string): string {
  const base = filename.split("/").pop() || filename;
  return base.replace(/\.gguf$/i, "");
}

/** Extract a model ID from a HuggingFace URL, or return null if not a HF URL. */
function parseHfUrl(input: string): string | null {
  const trimmed = input.trim();
  try {
    const url = new URL(trimmed);
    if (url.hostname !== "huggingface.co" && url.hostname !== "www.huggingface.co") return null;
    const segments = url.pathname.split("/").filter(Boolean);
    if (segments.length >= 2) {
      return `${segments[0]}/${segments[1]}`;
    }
  } catch {
    const hfMatch = trimmed.match(/^(?:www\.)?huggingface\.co\/([^/]+\/[^/\s]+)/i);
    if (hfMatch) return hfMatch[1];
  }
  return null;
}

function DetailReportContent({
  recData,
  selectedFile,
  kvType,
  modelOffload,
  kvPlacement,
  contextLength,
  sidecarReserveBytes = 0,
  t,
}: {
  recData: RecommendationData;
  selectedFile: FileRecommendation;
  kvType: string;
  modelOffload: ModelOffload;
  kvPlacement: KvPlacement;
  contextLength: number;
  sidecarReserveBytes?: number;
  t: TFn;
}) {
  const bpv = KV_BPV[kvType] || 2;
  const totalAvail = recData.totalAvailable;
  const maxCtx = Math.max(
    maxContextForBpv(
      selectedFile.size,
      recData.kvBasePerToken,
      bpv,
      totalAvail,
      recData.modelMaxContext,
      sidecarReserveBytes,
    ),
    1024,
  );
  const clampedCtx = Math.min(Math.max(contextLength, 1024), maxCtx);
  const effectiveKvCtx = recData.kvContextCap
    ? Math.min(clampedCtx, recData.kvContextCap)
    : clampedCtx;
  const kvBytes = recData.kvBasePerToken ? recData.kvBasePerToken * bpv * effectiveKvCtx : 0;
  const overhead = computeOverhead(selectedFile.size);
  const totalNeeded = selectedFile.size + kvBytes + overhead + sidecarReserveBytes;
  const gpuResidentBytes =
    selectedFile.size + overhead + sidecarReserveBytes + (kvPlacement === "ram" ? 0 : kvBytes);
  const headroom = Math.max(totalAvail - totalNeeded, 0);
  const vramBudget = Math.max(recData.availableVram * 0.9 - sidecarReserveBytes, 0);
  const { score, gpuMode, gpuScore } = calcScore(
    selectedFile.size,
    selectedFile.quantQuality,
    kvBytes,
    recData.availableRam,
    totalAvail,
    recData.availableVram,
    modelOffload,
    kvPlacement,
    sidecarReserveBytes,
  );

  const modelMax = recData.modelMaxContext;
  const detailFullGpuCtx = computeGpuOptimalContext(
    selectedFile.size,
    recData.kvBasePerToken,
    bpv,
    recData.availableVram,
    modelMax,
    recData.kvContextCap,
    sidecarReserveBytes,
  );
  const detailMaxRamCtx = computeRamMaxContext(
    selectedFile.size,
    recData.kvBasePerToken,
    bpv,
    totalAvail,
    modelMax,
    recData.kvContextCap,
    sidecarReserveBytes,
  );

  const memoryScore = (() => {
    if (totalAvail === 0) return 50;
    if (totalNeeded > totalAvail) return 0;
    const r = totalAvail / totalNeeded;
    return r < 1.2 ? 20 : r < 1.5 ? 50 : r < 2.0 ? 70 : r < 3.0 ? 85 : 100;
  })();
  const kvScore = (() => {
    if (kvBytes === 0) return 50;
    const h = Math.max(totalAvail - selectedFile.size - overhead - sidecarReserveBytes, 0);
    if (h === 0) return 0;
    if (h >= kvBytes) {
      const r = h / kvBytes;
      return r >= 2 ? 100 : Math.round(50 + 50 * (r - 1));
    }
    return Math.round(50 * (h / kvBytes));
  })();
  const modelOffloadCopy = getModelOffloadCopy(t, modelOffload);
  const kvPlacementCopy = getKvPlacementCopy(t, kvPlacement);
  const showGpuPlanning = recData.availableVram > 0 && modelOffload !== "cpu";

  const offloadPct = (() => {
    if (recData.availableVram <= 0 || modelOffload === "cpu") return 0;
    if (modelOffload === "gpu") return selectedFile.size <= vramBudget ? 100 : 0;
    if (selectedFile.size <= 0) return 0;
    if (modelOffload === "mixed") {
      return Math.min(Math.round((vramBudget / selectedFile.size) * 100), 100);
    }
    if (gpuResidentBytes <= vramBudget) return 100;
    return Math.min(Math.round((vramBudget / gpuResidentBytes) * 100), 99);
  })();

  const detailTotalLayers = gpuOffloadLayerCount(recData.arch?.blockCount);
  const detailRecLayers = (() => {
    if (!detailTotalLayers || detailTotalLayers <= 0) return null;
    if (!showGpuPlanning) return 0;
    if (gpuResidentBytes <= vramBudget) return detailTotalLayers;
    if (recData.availableVram <= 0) return null;
    const layers = Math.floor((vramBudget / gpuResidentBytes) * detailTotalLayers);
    return Math.max(Math.min(layers, detailTotalLayers), 0);
  })();

  const fullGpuCtx = (() => {
    if (recData.availableVram <= 0 || !recData.kvBasePerToken) return null;
    if (kvPlacement === "ram") return null;
    if (totalNeeded <= vramBudget) return null;
    if (selectedFile.size + overhead >= vramBudget) return null;
    const vramForKv = vramBudget - selectedFile.size - overhead;
    const bpvVal = KV_BPV[kvType] || 2;
    const maxCtxForGpu = Math.floor(vramForKv / (recData.kvBasePerToken * bpvVal));
    if (maxCtxForGpu < 512) return null;
    return maxCtxForGpu;
  })();

  const row = (label: string, value: string, highlight?: string) => (
    <div className="flex items-center justify-between py-1.5">
      <span className="text-[11px] text-white/50">{label}</span>
      <span className={cn("text-[11px] font-mono font-medium", highlight || "text-white/80")}>
        {value}
      </span>
    </div>
  );

  const bar = (label: string, value: number, weight: number, color: string) => (
    <div className="space-y-1">
      <div className="flex items-center justify-between">
        <span className="text-[11px] text-white/50">
          {label} <span className="text-white/25">({Math.round(weight * 100)}%)</span>
        </span>
        <span className={cn("text-[11px] font-mono font-semibold", color)}>
          {Math.round(value)}
        </span>
      </div>
      <div className="h-1 w-full rounded-full bg-white/10 overflow-hidden">
        <div
          className={cn("h-full rounded-full transition-all", color.replace("text-", "bg-"))}
          style={{ width: `${Math.min(value, 100)}%` }}
        />
      </div>
    </div>
  );

  const scoreColor =
    score >= 80
      ? "text-emerald-400"
      : score >= 60
        ? "text-blue-400"
        : score >= 40
          ? "text-amber-400"
          : score >= 20
            ? "text-orange-400"
            : "text-red-400";

  return (
    <div className="space-y-1">
      <MenuLabel>{t("hfBrowser.detailSystem")}</MenuLabel>
      <div className="rounded-xl border border-white/10 bg-white/3 px-4 py-1 divide-y divide-white/5">
        {row(t("hfBrowser.detailRam"), formatBytes(recData.availableRam))}
        {row(
          t("hfBrowser.detailVram"),
          recData.availableVram > 0 ? formatBytes(recData.availableVram) : "—",
        )}
        {recData.availableVram > 0 &&
          row(t("hfBrowser.detailVramBudget"), formatBytes(vramBudget), "text-white/60")}
        {recData.unifiedMemory &&
          row(t("hfBrowser.detailMemMode"), t("hfBrowser.detailUnified"), "text-amber-400")}
        {row(t("hfBrowser.detailTotalAvailable"), formatBytes(totalAvail))}
      </div>

      {recData.arch && (
        <>
          <MenuLabel>{t("hfBrowser.detailArchitecture")}</MenuLabel>
          <div className="rounded-xl border border-white/10 bg-white/3 px-4 py-1 divide-y divide-white/5">
            {recData.arch.architecture &&
              row(t("hfBrowser.detailArch"), recData.arch.architecture.toUpperCase())}
            {recData.arch.blockCount != null &&
              row(t("hfBrowser.detailLayers"), recData.arch.blockCount.toString())}
            {recData.arch.embeddingLength != null &&
              row(t("hfBrowser.detailEmbedding"), recData.arch.embeddingLength.toLocaleString())}
            {recData.arch.headCount != null &&
              row(t("hfBrowser.detailHeads"), recData.arch.headCount.toString())}
            {recData.arch.headCountKv != null &&
              row(t("hfBrowser.detailKvHeads"), recData.arch.headCountKv.toString())}
            {recData.arch.feedForwardLength != null &&
              row(t("hfBrowser.detailFfn"), recData.arch.feedForwardLength.toLocaleString())}
            {recData.arch.contextLength != null &&
              row(t("hfBrowser.detailTrainCtx"), recData.arch.contextLength.toLocaleString())}
            {recData.arch.slidingWindow != null &&
              row(t("hfBrowser.detailSwa"), recData.arch.slidingWindow.toLocaleString())}
            {recData.arch.kvLoraRank != null &&
              row(t("hfBrowser.detailMlaRank"), recData.arch.kvLoraRank.toString())}
            {recData.arch.incompleteParse &&
              row(
                t("hfBrowser.detailParseStatus"),
                t("hfBrowser.detailIncomplete"),
                "text-amber-400",
              )}
          </div>
        </>
      )}

      <MenuDivider />

      <MenuLabel>{t("hfBrowser.detailConfig")}</MenuLabel>
      <div className="rounded-xl border border-white/10 bg-white/3 px-4 py-1 divide-y divide-white/5">
        {row(t("hfBrowser.quantization"), selectedFile.quantization)}
        {row(t("hfBrowser.detailModelSize"), formatBytes(selectedFile.size))}
        {row(t("hfBrowser.contextLength"), clampedCtx.toLocaleString() + " tokens")}
        {row(t("hfBrowser.detailModelOffload"), modelOffloadCopy.label)}
        {row(t("hfBrowser.detailKvLocation"), kvPlacementCopy.label)}
        {row(t("hfBrowser.kvCacheType"), kvType.toUpperCase())}
        {recData.kvContextCap &&
          row(
            t("hfBrowser.detailEffectiveKvCtx"),
            effectiveKvCtx.toLocaleString() + " tokens",
            "text-white/60",
          )}
        {showGpuPlanning &&
          detailFullGpuCtx > 0 &&
          row(
            t("hfBrowser.detailOptimalGpuCtx"),
            detailFullGpuCtx.toLocaleString() + " tokens",
            "text-emerald-400",
          )}
        {detailMaxRamCtx > 0 &&
          row(
            t("hfBrowser.detailOptimalRamCtx"),
            detailMaxRamCtx.toLocaleString() + " tokens",
            "text-amber-400",
          )}
      </div>

      <MenuLabel>{t("hfBrowser.detailMemory")}</MenuLabel>
      <div className="rounded-xl border border-white/10 bg-white/3 px-4 py-1 divide-y divide-white/5">
        {row(t("hfBrowser.detailWeights"), formatBytes(selectedFile.size))}
        {row(t("hfBrowser.detailKvCache"), kvBytes > 0 ? formatBytes(kvBytes) : "—")}
        {row(t("hfBrowser.detailComputeBuffer"), formatBytes(overhead))}
        {row(
          t("hfBrowser.detailTotalNeeded"),
          formatBytes(totalNeeded),
          totalNeeded > totalAvail ? "text-red-400" : "text-emerald-400",
        )}
        {row(
          t("hfBrowser.detailHeadroom"),
          headroom > 0 ? formatBytes(headroom) : "0 B",
          headroom > 0 ? "text-emerald-400/70" : "text-red-400/70",
        )}
        {recData.availableVram > 0 &&
          row(
            t("hfBrowser.detailMode"),
            getRunabilityModeCopy(t, gpuMode).long,
            ["full", "nearFull"].includes(gpuMode)
              ? "text-emerald-400"
              : ["kvSpill", "mostLayers", "ramModelVramCtx"].includes(gpuMode)
                ? "text-blue-400"
                : ["kvHeavySpill", "halfLayers", "ramModelRamCtx"].includes(gpuMode)
                  ? "text-amber-400"
                  : "text-red-400",
          )}
        {showGpuPlanning &&
          row(
            t("hfBrowser.detailOffload"),
            `${offloadPct}%`,
            offloadPct >= 100
              ? "text-emerald-400"
              : offloadPct >= 50
                ? "text-blue-400"
                : offloadPct > 0
                  ? "text-amber-400"
                  : "text-red-400",
          )}
        {detailRecLayers != null &&
          detailTotalLayers != null &&
          detailRecLayers < detailTotalLayers &&
          detailRecLayers > 0 &&
          row(
            t("hfBrowser.detailLayers-ngl"),
            `${detailRecLayers} / ${detailTotalLayers}`,
            detailRecLayers >= detailTotalLayers * 0.8
              ? "text-blue-400"
              : detailRecLayers >= detailTotalLayers * 0.5
                ? "text-amber-400"
                : "text-red-400",
          )}
      </div>

      {kvBytes > 0 &&
        recData.availableVram > 0 &&
        (() => {
          let kvVramPct: number;
          if (!showGpuPlanning && kvPlacement !== "vram") {
            kvVramPct = 0;
          } else if (kvPlacement === "ram") {
            kvVramPct = 0;
          } else if (kvPlacement === "vram") {
            kvVramPct = kvBytes > 0 ? Math.min(Math.round((vramBudget / kvBytes) * 100), 100) : 100;
          } else if (totalNeeded <= vramBudget) {
            kvVramPct = 100;
          } else if (selectedFile.size >= vramBudget) {
            const layerRatio = Math.min(vramBudget / selectedFile.size, 1.0);
            kvVramPct = Math.round(layerRatio * 100);
          } else {
            const vramForKv = vramBudget - selectedFile.size - overhead;
            kvVramPct = vramForKv > 0 ? Math.min(Math.round((vramForKv / kvBytes) * 100), 100) : 0;
          }
          const kvRamPct = 100 - kvVramPct;
          const kvOnVram = kvBytes * (kvVramPct / 100);
          const kvOnRam = kvBytes - kvOnVram;

          return (
            <>
              <MenuLabel>{t("hfBrowser.detailKvDistribution")}</MenuLabel>
              <div className="rounded-xl border border-white/10 bg-white/3 px-4 py-1 divide-y divide-white/5">
                {row(
                  t("hfBrowser.detailKvOnGpu"),
                  `${formatBytes(kvOnVram)} (${kvVramPct}%)`,
                  kvVramPct >= 80
                    ? "text-emerald-400"
                    : kvVramPct >= 40
                      ? "text-blue-400"
                      : "text-amber-400",
                )}
                {row(
                  t("hfBrowser.detailKvOnRam"),
                  `${formatBytes(kvOnRam)} (${kvRamPct}%)`,
                  kvRamPct === 0
                    ? "text-emerald-400/60"
                    : kvRamPct <= 20
                      ? "text-blue-400"
                      : "text-amber-400",
                )}
              </div>
              {kvRamPct > 0 && (
                <div className="flex items-start gap-2 rounded-xl border border-amber-400/15 bg-amber-400/5 px-3 py-2 mt-1">
                  <Info size={12} className="text-amber-400 shrink-0 mb-0.5" />
                  <p className="text-[12px] leading-snug text-amber-300/70">
                    {t("hfBrowser.kvDistributionTip", { pct: kvRamPct.toString() })}
                  </p>
                </div>
              )}
            </>
          );
        })()}

      {showGpuPlanning && fullGpuCtx && fullGpuCtx < clampedCtx && (
        <div className="flex items-start gap-2 rounded-xl border border-blue-400/20 bg-blue-400/5 px-3 py-2 mt-1">
          <Info size={12} className="text-blue-400 shrink-0 mt-0.5" />
          <p className="text-[12px] leading-snug text-blue-300/80">
            {t("hfBrowser.detailCtxTip", { ctx: fullGpuCtx.toLocaleString() })}
          </p>
        </div>
      )}

      <MenuDivider />

      <MenuLabel>{t("hfBrowser.detailScoreBreakdown")}</MenuLabel>
      <div className="rounded-xl border border-white/10 bg-white/3 px-4 py-3 space-y-3">
        {bar(
          t("hfBrowser.detailMemFitness"),
          memoryScore,
          0.25,
          memoryScore >= 70
            ? "text-emerald-400"
            : memoryScore >= 40
              ? "text-amber-400"
              : "text-red-400",
        )}
        {bar(
          t("hfBrowser.detailGpuAccel"),
          gpuScore,
          0.35,
          gpuScore >= 75 ? "text-emerald-400" : gpuScore >= 35 ? "text-amber-400" : "text-red-400",
        )}
        {bar(
          t("hfBrowser.detailKvHeadroom"),
          kvScore,
          0.15,
          kvScore >= 70 ? "text-emerald-400" : kvScore >= 40 ? "text-amber-400" : "text-red-400",
        )}
        {bar(
          t("hfBrowser.detailQuantQuality"),
          selectedFile.quantQuality,
          0.25,
          selectedFile.quantQuality >= 75
            ? "text-emerald-400"
            : selectedFile.quantQuality >= 50
              ? "text-amber-400"
              : "text-red-400",
        )}
      </div>

      <div className="flex items-center justify-between rounded-xl border border-white/10 bg-white/3 px-4 py-3 mt-2">
        <span className="text-[12px] font-semibold text-white/70">
          {t("hfBrowser.detailFinalScore")}
        </span>
        <span className={cn("text-2xl font-bold", scoreColor)}>{score}</span>
      </div>
    </div>
  );
}

export function HuggingFaceBrowserPage() {
  const { t } = useI18n();
  const hfNavigate = useNavigate();
  const openInstalledImageModel = useCallback(
    (modelId: string) => hfNavigate(`/settings/models/${modelId}`),
    [hfNavigate],
  );
  const [searchParams, setSearchParams] = useSearchParams();
  const returnTo = searchParams.get("returnTo");
  const { queue, dismissItem, hasItems: hasDownloads } = useDownloadQueue();

  const [hasPersistedLocalModel, setHasPersistedLocalModel] = useState(() => {
    const cached = readSettingsCached();
    return cached ? cached.models.some((m) => m.providerId === "llamacpp") : false;
  });
  const hasCompletedDownload = queue.some(
    (item) => item.status === "complete" && item.createModelWhenFinished,
  );
  const hasLocalModel = hasPersistedLocalModel || hasCompletedDownload;

  const [avatars, setAvatars] = useState<Record<string, string>>({});

  const [defaultedToSearch, setDefaultedToSearch] = useState(false);

  useEffect(() => {
    const syncLocalModelState = async () => {
      const settings = await readSettings();
      setHasPersistedLocalModel(settings.models.some((model) => model.providerId === "llamacpp"));
      const ollama = settings.providerCredentials.filter((p) => p.providerId === "ollama");
      setOllamaProviders(ollama);
      setSelectedOllamaProviderId((prev) =>
        prev && ollama.some((p) => p.id === prev) ? prev : null,
      );
      setProvidersLoaded(true);
    };

    void syncLocalModelState();

    const handler = () => {
      void syncLocalModelState();
    };
    window.addEventListener(SETTINGS_UPDATED_EVENT, handler);
    return () => window.removeEventListener(SETTINGS_UPDATED_EVENT, handler);
  }, []);

  // Helper to preserve returnTo across param changes
  const preserveParams = useCallback(
    (next: Record<string, string>) => {
      if (returnTo) next.returnTo = returnTo;
      return next;
    },
    [returnTo],
  );

  useEffect(() => {
    if (defaultedToSearch) return;
    if (searchParams.get("model")) {
      setSearchParams(preserveParams({}), { replace: true });
    }
    setDefaultedToSearch(true);
  }, [defaultedToSearch, searchParams, setSearchParams, preserveParams]);

  const view: ViewState = useMemo(() => {
    if (!defaultedToSearch) return { kind: "search" };
    const modelParam = searchParams.get("model");
    if (modelParam) return { kind: "model", modelId: modelParam };
    const authorParam = searchParams.get("author");
    if (authorParam) return { kind: "author", author: authorParam };
    return { kind: "search" };
  }, [defaultedToSearch, searchParams]);

  const setView = useCallback(
    (v: ViewState) => {
      if (v.kind === "search") {
        setSearchParams(preserveParams({}), { replace: true });
      } else if (v.kind === "model") {
        setSearchParams(preserveParams({ model: v.modelId }), { replace: false });
      } else if (v.kind === "author") {
        setSearchParams(preserveParams({ author: v.author }), { replace: false });
      }
    },
    [setSearchParams, preserveParams],
  );

  const isMobilePlatform = useMemo(() => getPlatform().type === "mobile", []);

  const HF_DESTINATION_STORAGE_KEY = "hfBrowser:destinationProviderId";
  type HfBrowserMode = "llm" | "image" | "imageBundle";
  const HF_MODE_STORAGE_KEY = "hfBrowser:mode";
  const [browserMode, setBrowserMode] = useState<HfBrowserMode>(() => {
    if (typeof window === "undefined") return "llm";
    const saved = window.sessionStorage.getItem(HF_MODE_STORAGE_KEY);
    return saved === "image" || saved === "imageBundle" ? saved : "llm";
  });
  const [providersLoaded, setProvidersLoaded] = useState(() => readSettingsCached() != null);
  const [ollamaProviders, setOllamaProviders] = useState<ProviderCredential[]>(() => {
    const cached = readSettingsCached();
    return cached ? cached.providerCredentials.filter((p) => p.providerId === "ollama") : [];
  });
  const [selectedOllamaProviderId, setSelectedOllamaProviderId] = useState<string | null>(() => {
    if (typeof window === "undefined") return null;
    return window.sessionStorage.getItem(HF_DESTINATION_STORAGE_KEY);
  });
  const [showOllamaProviderMenu, setShowOllamaProviderMenu] = useState(false);

  useEffect(() => {
    if (typeof window === "undefined") return;
    if (selectedOllamaProviderId) {
      window.sessionStorage.setItem(HF_DESTINATION_STORAGE_KEY, selectedOllamaProviderId);
    } else {
      window.sessionStorage.removeItem(HF_DESTINATION_STORAGE_KEY);
    }
  }, [selectedOllamaProviderId]);

  useEffect(() => {
    window.sessionStorage.setItem(HF_MODE_STORAGE_KEY, browserMode);
    if (browserMode !== "llm") setSelectedOllamaProviderId(null);
  }, [browserMode]);

  // Filter state
  const [showFilterMenu, setShowFilterMenu] = useState(false);
  const [filterPipelineTags, setFilterPipelineTags] = useState<Set<string>>(new Set());
  const [filterParamMin, setFilterParamMin] = useState<string>("");
  const [filterParamMax, setFilterParamMax] = useState<string>("");
  const [filterAuthor, setFilterAuthor] = useState<string>("");
  const [debouncedFilterAuthor, setDebouncedFilterAuthor] = useState<string>("");
  const [unfilteredSearch, setUnfilteredSearch] = useState(false);
  const [bundleFormat, setBundleFormat] = useState<"gguf" | "safetensors">("gguf");

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedFilterAuthor(filterAuthor.trim()), 350);
    return () => clearTimeout(timer);
  }, [filterAuthor]);

  const filterParamMinNum = filterParamMin ? Number(filterParamMin) : null;
  const filterParamMaxNum = filterParamMax ? Number(filterParamMax) : null;
  const activeFilterCount =
    filterPipelineTags.size +
    (filterParamMinNum != null && !isNaN(filterParamMinNum) ? 1 : 0) +
    (filterParamMaxNum != null && !isNaN(filterParamMaxNum) ? 1 : 0) +
    (filterAuthor.trim() ? 1 : 0) +
    (unfilteredSearch ? 1 : 0);

  type HfBrowserViewMode = "list" | "grid" | "gallery";
  const HF_VIEW_STORAGE_KEY = "hfBrowser:viewMode";
  const [browserViewMode, setBrowserViewMode] = useState<HfBrowserViewMode>(() => {
    if (typeof window === "undefined") return "grid";
    const stored = window.localStorage.getItem(HF_VIEW_STORAGE_KEY);
    return stored === "list" || stored === "grid" || stored === "gallery" ? stored : "grid";
  });

  useEffect(() => {
    if (typeof window === "undefined") return;
    window.localStorage.setItem(HF_VIEW_STORAGE_KEY, browserViewMode);
    (window as any).__hfBrowserViewMode = browserViewMode;
    window.dispatchEvent(new CustomEvent("hfBrowser:viewModeChanged"));
  }, [browserViewMode]);

  useEffect(() => {
    const handler = () => {
      setBrowserViewMode((prev) => {
        const order: HfBrowserViewMode[] = ["list", "grid", "gallery"];
        return order[(order.indexOf(prev) + 1) % order.length];
      });
    };
    window.addEventListener("hfBrowser:cycleViewMode", handler);
    return () => window.removeEventListener("hfBrowser:cycleViewMode", handler);
  }, []);

  const selectedOllamaProvider = useMemo(
    () => ollamaProviders.find((p) => p.id === selectedOllamaProviderId) ?? null,
    [ollamaProviders, selectedOllamaProviderId],
  );
  const isOllamaMode = browserMode === "llm" && !!selectedOllamaProvider;
  const isImageMode = browserMode === "image";
  const isBundleMode = browserMode === "imageBundle";
  const unfilteredActive = unfilteredSearch && !isOllamaMode && !isBundleMode;
  const goToModelsPage = useCallback(() => hfNavigate("/settings/models"), [hfNavigate]);
  const bundle = useImageBundle({
    enabled: isBundleMode,
    onOpenModel: openInstalledImageModel,
    onDownloadStarted: goToModelsPage,
  });

  const handleRailRole = useCallback(
    (role: BundleRole) => {
      if (bundle.draft.bundleId) return;
      bundle.clearRole(role);
      setView({ kind: "search" });
    },
    [bundle.draft.bundleId, bundle.clearRole, setView],
  );

  const handleBundleSelect = useCallback(
    (asset: ValidatedAsset) => {
      bundle.selectAsset(asset);
      setView({ kind: "search" });
    },
    [bundle.selectAsset, setView],
  );

  useEffect(() => {
    if (!isMobilePlatform || !providersLoaded || ollamaProviders.length === 0) return;
    if (
      selectedOllamaProviderId &&
      ollamaProviders.some((p) => p.id === selectedOllamaProviderId)
    ) {
      return;
    }
    if (ollamaProviders.length === 1) {
      setSelectedOllamaProviderId(ollamaProviders[0].id);
    } else {
      setShowOllamaProviderMenu(true);
    }
  }, [isMobilePlatform, providersLoaded, ollamaProviders, selectedOllamaProviderId]);

  const sproutConfig = useMemo(() => {
    const config = (selectedOllamaProvider?.config ?? {}) as Record<string, any>;
    if (config.sproutEnabled !== true) return null;
    const url = typeof config.sproutUrl === "string" ? config.sproutUrl.trim() : "";
    if (!url) return null;
    const apiKey = typeof config.sproutApiKey === "string" ? config.sproutApiKey : "";
    return { url, apiKey };
  }, [selectedOllamaProvider]);
  const sproutActive = isOllamaMode && !!sproutConfig;

  const [query, setQuery] = useState("");
  const [debouncedQuery, setDebouncedQuery] = useState("");
  const [searchFocused, setSearchFocused] = useState(false);
  const collapseSearchNeighbors = isMobilePlatform && (searchFocused || query.trim().length > 0);
  const [sortMode, setSortMode] = useState<SortMode>("trending");
  const [results, setResults] = useState<HfSearchResult[]>([]);
  const [searching, setSearching] = useState(false);
  const [searchError, setSearchError] = useState<string | null>(null);
  const [hasSearched, setHasSearched] = useState(false);
  const [loadingMore, setLoadingMore] = useState(false);
  const [hasMore, setHasMore] = useState(true);

  const displayedResults = useMemo(() => {
    if (isBundleMode) return results;
    if (filterPipelineTags.size === 0 && filterParamMinNum == null && filterParamMaxNum == null) {
      return results;
    }
    return results.filter((r) => {
      if (filterPipelineTags.size > 0) {
        if (!r.pipelineTag || !filterPipelineTags.has(r.pipelineTag)) return false;
      }
      if (filterParamMinNum != null || filterParamMaxNum != null) {
        const sizeStr = extractParamSize(r.tags, r.modelId);
        const sizeB = paramSizeToBillions(sizeStr);
        if (sizeB == null) return false;
        if (filterParamMinNum != null && !isNaN(filterParamMinNum) && sizeB < filterParamMinNum) {
          return false;
        }
        if (filterParamMaxNum != null && !isNaN(filterParamMaxNum) && sizeB > filterParamMaxNum) {
          return false;
        }
      }
      return true;
    });
  }, [results, filterPipelineTags, filterParamMinNum, filterParamMaxNum, isBundleMode]);

  // Pipeline tags present in current results (to surface relevant options)
  const availablePipelineTags = useMemo(() => {
    const tags = new Set<string>();
    for (const r of results) {
      if (r.pipelineTag) tags.add(r.pipelineTag);
    }
    return tags;
  }, [results]);

  const PAGE_SIZE = 30;

  const [modelInfo, setModelInfo] = useState<HfModelInfo | null>(null);
  const [loadingFiles, setLoadingFiles] = useState(false);
  const [filesError, setFilesError] = useState<string | null>(null);
  const [gatedMenuOpen, setGatedMenuOpen] = useState(false);

  const [readme, setReadme] = useState<string | null>(null);
  const [readmeLoading, setReadmeLoading] = useState(false);
  const [runabilityScores, setRunabilityScores] = useState<Record<string, RunabilityScore>>({});

  const { shouldShow: showHfTour, dismiss: dismissHfTour } = useGuidedTour("hfBrowser");

  // Recommendation panel state
  const [recData, setRecData] = useState<RecommendationData | null>(null);
  const [recLoading, setRecLoading] = useState(false);
  const [recFile, setRecFile] = useState(""); // selected file in dropdown
  const [recContext, setRecContext] = useState(4096);
  const [recKvType, setRecKvType] = useState("f16");
  const [recModelOffload, setRecModelOffload] = useState<ModelOffload>("auto");
  const [recKvPlacement, setRecKvPlacement] = useState<KvPlacement>("auto");
  const [recImageSupport, setRecImageSupport] = useState(false);
  const [recMmprojFile, setRecMmprojFile] = useState("");
  const [recMtpSupport, setRecMtpSupport] = useState(false);
  const [recMtpFile, setRecMtpFile] = useState("");
  const [detailSheetOpen, setDetailSheetOpen] = useState(false);
  const [compareOpen, setCompareOpen] = useState(false);
  const [compareSelections, setCompareSelections] = useState<CompareSelection[]>([]);
  const [filesPanelTab, setFilesPanelTab] = useState<FilesPanelTab>("recommended");
  const [filesDrawerOpen, setFilesDrawerOpen] = useState(false);

  const searchInputRef = useRef<HTMLInputElement>(null);
  const filesPanelRef = useRef<HTMLDivElement>(null);
  const rafRef = useRef<number>(0);
  const compareNextIdRef = useRef(1);
  const compareScrollRefs = useRef<Record<number, HTMLDivElement | null>>({});
  const compareSyncRafRef = useRef<number | null>(null);
  const compareSyncSourceIdRef = useRef<number | null>(null);
  const compareSyncRatioRef = useRef(0);
  const processedRecommendedInstallsRef = useRef<Set<string>>(new Set());
  const prevQueueRef = useRef<QueuedDownload[]>([]);

  useEffect(() => {
    return () => {
      if (compareSyncRafRef.current != null) {
        cancelAnimationFrame(compareSyncRafRef.current);
      }
    };
  }, []);

  useEffect(() => {
    if (view.kind !== "model" || !filesPanelRef.current) return;

    let scrollable: HTMLElement | null = filesPanelRef.current.parentElement;
    while (scrollable) {
      const style = getComputedStyle(scrollable);
      if (
        (style.overflowY === "auto" || style.overflowY === "scroll") &&
        scrollable.scrollHeight > scrollable.clientHeight
      ) {
        break;
      }
      scrollable = scrollable.parentElement;
    }
    if (!scrollable) return;

    const updatePanel = () => {
      const panel = filesPanelRef.current;
      if (!panel) return;
      const scrollTop = scrollable!.scrollTop;
      panel.style.transform = `translateY(${scrollTop}px)`;
      const panelRect = panel.getBoundingClientRect();
      const availableHeight = window.innerHeight - panelRect.top - 24;
      panel.style.maxHeight = `${Math.max(availableHeight, 200)}px`;
    };

    const handleScroll = () => {
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
      rafRef.current = requestAnimationFrame(updatePanel);
    };

    scrollable.addEventListener("scroll", handleScroll, { passive: true });
    window.addEventListener("resize", handleScroll, { passive: true });
    filesPanelRef.current.style.transform = "translateY(0px)";
    updatePanel();
    return () => {
      scrollable!.removeEventListener("scroll", handleScroll);
      window.removeEventListener("resize", handleScroll);
      if (rafRef.current) cancelAnimationFrame(rafRef.current);
    };
  }, [view.kind, modelInfo]);

  useEffect(() => {
    setFilesDrawerOpen(false);
  }, [view.kind, modelInfo]);

  useEffect(() => {
    const modelId = parseHfUrl(query);
    if (modelId) {
      setDebouncedQuery(modelId);
      return;
    }
    const timer = setTimeout(() => setDebouncedQuery(query), 350);
    return () => clearTimeout(timer);
  }, [query]);

  const sortField = useCallback(
    (mode: SortMode) =>
      mode === "trending"
        ? "trendingScore"
        : mode === "downloads"
          ? "downloads"
          : mode === "likes"
            ? "likes"
            : "lastModified",
    [],
  );

  const isDirectLookup = parseHfUrl(query) !== null;

  const doSearch = useCallback(async () => {
    if (isBundleMode && (!bundle.profile || !bundle.currentRole)) {
      setResults([]);
      setSearching(false);
      setHasSearched(false);
      setHasMore(false);
      return;
    }
    setSearching(true);
    setSearchError(null);
    setHasMore(true);
    try {
      const data = await invoke<HfSearchResult[]>("hf_search_models", {
        query: debouncedQuery,
        limit: isDirectLookup ? 5 : PAGE_SIZE,
        sort: sortField(sortMode),
        offset: 0,
        author: debouncedFilterAuthor || null,
        mode: browserMode,
        profileId: isBundleMode ? bundle.profile?.id : null,
        bundleRole: isBundleMode ? bundle.currentRole : null,
        bundleFormat: isBundleMode ? bundleFormat : null,
        unfiltered: unfilteredActive,
      });
      if (isDirectLookup && !isBundleMode) {
        const exact = data.filter((d) => d.modelId.toLowerCase() === debouncedQuery.toLowerCase());
        setResults(exact.length > 0 ? exact : data.slice(0, 1));
        setHasMore(false);
      } else {
        setResults(data);
        if (isBundleMode || data.length < PAGE_SIZE) setHasMore(false);
      }
    } catch (err: any) {
      setSearchError(err?.message || String(err));
      setResults([]);
      setHasMore(false);
    } finally {
      setSearching(false);
      setHasSearched(true);
    }
  }, [
    debouncedQuery,
    sortMode,
    sortField,
    isDirectLookup,
    debouncedFilterAuthor,
    browserMode,
    isBundleMode,
    bundle.profile,
    bundle.currentRole,
    bundleFormat,
    unfilteredActive,
  ]);

  const loadMore = useCallback(async () => {
    if (loadingMore || !hasMore || isBundleMode) return;
    setLoadingMore(true);
    try {
      const data = await invoke<HfSearchResult[]>("hf_search_models", {
        query: debouncedQuery,
        limit: PAGE_SIZE,
        sort: sortField(sortMode),
        offset: results.length,
        author: debouncedFilterAuthor || null,
        mode: browserMode,
        unfiltered: unfilteredActive,
      });
      if (data.length < PAGE_SIZE) setHasMore(false);
      if (data.length > 0) {
        setResults((prev) => {
          const seen = new Set(prev.map((p) => p.modelId));
          const fresh = data.filter((d) => !seen.has(d.modelId));
          return fresh.length > 0 ? [...prev, ...fresh] : prev;
        });
      }
    } catch {
    } finally {
      setLoadingMore(false);
    }
  }, [
    loadingMore,
    hasMore,
    isBundleMode,
    debouncedQuery,
    sortMode,
    sortField,
    results.length,
    debouncedFilterAuthor,
    browserMode,
    unfilteredActive,
  ]);

  useEffect(() => {
    if (view.kind === "search") {
      doSearch();
    }
  }, [debouncedQuery, sortMode, view.kind, doSearch]);

  useEffect(() => {
    if (!isBundleMode || !bundle.profile || !bundle.currentRole) return;
    const remembered = bundle.draft.repositories[bundle.currentRole];
    const recommended = bundle.profile.recommendedRepositories[bundle.currentRole];
    setQuery(
      bundle.currentRole === "diffusion_model"
        ? (remembered ?? "")
        : (remembered ?? recommended ?? ""),
    );
  }, [isBundleMode, bundle.profile, bundle.currentRole]); // eslint-disable-line react-hooks/exhaustive-deps

  useEffect(() => {
    if (results.length === 0) return;
    const uniqueAuthors = [...new Set(results.map((r) => r.author))];
    const missing = uniqueAuthors.filter((a) => !(a in avatars));
    if (missing.length === 0) return;

    let cancelled = false;
    invoke<Record<string, string>>("hf_get_avatars", { authors: missing })
      .then((fetched) => {
        if (cancelled) return;
        setAvatars((prev) => ({ ...prev, ...fetched }));
      })
      .catch(() => {});

    return () => {
      cancelled = true;
    };
  }, [results]); // eslint-disable-line react-hooks/exhaustive-deps

  const openModel = useCallback(
    async (modelId: string) => {
      setView({ kind: "model", modelId });
      setModelInfo(null);
      setLoadingFiles(true);
      setFilesError(null);
      setReadme(null);
      setReadmeLoading(true);
      setRunabilityScores({});
      setRecData(null);
      setRecLoading(isImageMode || isBundleMode ? false : !isOllamaMode || sproutActive);
      setFilesPanelTab(
        isImageMode || isBundleMode || (isOllamaMode && !sproutActive) ? "files" : "recommended",
      );
      if (isBundleMode) void bundle.loadRoleFiles(modelId);
      setCompareOpen(false);
      setCompareSelections([]);
      setRecImageSupport(false);
      setRecMmprojFile("");

      const filesPromise = invoke<HfModelInfo>("hf_get_model_files", {
        modelId,
        mode: browserMode,
      })
        .then((info) => {
          setModelInfo(info);
          const runnableFiles = info.files.filter((f) => f.size > 0 && !f.isMmproj && !f.isMtp);
          if (!isImageMode && !isBundleMode && runnableFiles.length > 0 && (!isOllamaMode || sproutActive)) {
            invoke<RunabilityScore[]>("hf_compute_runability", {
              modelId: info.modelId,
              files: runnableFiles.map((f) => ({
                filename: f.filename,
                size: f.size,
                quantization: f.quantization,
              })),
              sproutUrl: sproutConfig?.url,
              sproutApiKey: sproutConfig?.apiKey || undefined,
            })
              .then((scores) => {
                const map: Record<string, RunabilityScore> = {};
                for (const s of scores) map[s.filename] = s;
                setRunabilityScores(map);
              })
              .catch(() => {});
          }
        })
        .catch((err: any) => {
          const message = err?.message || String(err);
          setFilesError(message);
          if (!isOllamaMode && isHfGatedError(message)) setGatedMenuOpen(true);
        })
        .finally(() => {
          setLoadingFiles(false);
        });

      const readmePromise = invoke<string>("hf_fetch_readme", { modelId })
        .then((md) => {
          setReadme(md);
        })
        .catch(() => {
          setReadme(null);
        })
        .finally(() => {
          setReadmeLoading(false);
        });

      await Promise.allSettled([filesPromise, readmePromise]);
    },
    [setView, isOllamaMode, isImageMode, isBundleMode, bundle.loadRoleFiles, sproutActive, sproutConfig, browserMode],
  );

  useEffect(() => {
    if (bundle.roleFilesError && isHfGatedError(bundle.roleFilesError)) setGatedMenuOpen(true);
  }, [bundle.roleFilesError]);

  useEffect(() => {
    if (view.kind !== "model" || !modelInfo) return;
    if (isImageMode || isBundleMode) {
      setRecLoading(false);
      setRecData(null);
      return;
    }
    if (isOllamaMode && !sproutActive) {
      setRecLoading(false);
      setRecData(null);
      return;
    }
    const files = modelInfo.files.filter((f) => f.size > 0 && !f.isMmproj && !f.isMtp);
    if (files.length === 0) {
      setRecLoading(false);
      return;
    }

    let cancelled = false;
    setRecLoading(true);

    invoke<RecommendationData>("hf_get_recommendation_data", {
      modelId: modelInfo.modelId,
      files: files.map((f) => ({
        filename: f.filename,
        size: f.size,
        quantization: f.quantization,
      })),
      sproutUrl: sproutConfig?.url,
      sproutApiKey: sproutConfig?.apiKey || undefined,
    })
      .then((data) => {
        if (cancelled) return;
        setRecData(data);
        setRecFile(data.best?.filename || files[0]?.filename || "");
        setRecContext(data.best?.contextLength || 4096);
        setRecKvType(data.best?.kvType || "q8_0");
        if (!data.supportsGpuOffload) {
          setRecModelOffload("auto");
          setRecKvPlacement("auto");
        }
      })
      .catch(() => {
        if (cancelled) return;
        setRecData(null);
      })
      .finally(() => {
        if (!cancelled) setRecLoading(false);
      });

    return () => {
      cancelled = true;
    };
  }, [view.kind, modelInfo, isOllamaMode, isImageMode, isBundleMode, sproutActive, sproutConfig]);

  const filesWithSize = modelInfo?.files.filter((f) => f.size > 0) ?? [];
  const runnableFilesWithSize = useMemo(
    () => filesWithSize.filter((f) => !f.isMmproj && !f.isMtp),
    [filesWithSize],
  );
  const mmprojFilesWithSize = useMemo(
    () => filesWithSize.filter((f) => f.isMmproj),
    [filesWithSize],
  );
  const mtpFilesWithSize = useMemo(
    () => filesWithSize.filter((f) => f.isMtp && !f.isMmproj),
    [filesWithSize],
  );
  const selectedRecommendedFile = useMemo(
    () => recData?.files.find((file) => file.filename === recFile) ?? recData?.files[0] ?? null,
    [recData, recFile],
  );
  const recMtpBundled = (recData?.arch?.nextnPredictLayers ?? 0) > 0;
  const recMtpAvailable = recMtpBundled || mtpFilesWithSize.length > 0;
  const selectedRecMmproj = useMemo(
    () =>
      recImageSupport && recMmprojFile
        ? (mmprojFilesWithSize.find((file) => file.filename === recMmprojFile) ?? null)
        : null,
    [mmprojFilesWithSize, recImageSupport, recMmprojFile],
  );
  const selectedRecMtp = useMemo(
    () =>
      recMtpSupport && !recMtpBundled
        ? (mtpFilesWithSize.find((file) => file.filename === recMtpFile) ??
          mtpFilesWithSize[0] ??
          null)
        : null,
    [mtpFilesWithSize, recMtpBundled, recMtpFile, recMtpSupport],
  );
  const activeSidecarReserveBytes = (selectedRecMmproj?.size ?? 0) + (selectedRecMtp?.size ?? 0);
  const gpuOptionsEnabled = Boolean(recData?.supportsGpuOffload);
  const recommendedMixedGpuLayers = useMemo(() => {
    if (!recData || !selectedRecommendedFile) return null;
    const totalLayers = gpuOffloadLayerCount(recData.arch?.blockCount);
    if (!totalLayers || totalLayers <= 0 || recData.availableVram <= 0) return null;

    const bpv = KV_BPV[recKvType] || 2;
    const maxAllowedContext = Math.max(
      maxContextForBpv(
        selectedRecommendedFile.size,
        recData.kvBasePerToken,
        bpv,
        recData.totalAvailable,
        recData.modelMaxContext,
        activeSidecarReserveBytes,
      ),
      1024,
    );
    const clampedCtx = Math.min(Math.max(recContext, 1024), maxAllowedContext);
    const effectiveKvCtx = recData.kvContextCap
      ? Math.min(clampedCtx, recData.kvContextCap)
      : clampedCtx;
    const kvBytes = recData.kvBasePerToken ? recData.kvBasePerToken * bpv * effectiveKvCtx : 0;
    const overhead = computeOverhead(selectedRecommendedFile.size);
    const totalNeeded =
      selectedRecommendedFile.size + kvBytes + overhead + activeSidecarReserveBytes;
    const vramBudget = Math.max(recData.availableVram * 0.9 - activeSidecarReserveBytes, 0);

    if (selectedRecommendedFile.size <= vramBudget) return totalLayers;
    if (totalNeeded <= 0) return null;

    const layerBudget =
      recKvPlacement === "ram"
        ? Math.max(vramBudget - overhead, 0)
        : Math.max(vramBudget - Math.min(kvBytes, vramBudget * 0.25), 0);
    const layers = Math.floor((layerBudget / totalNeeded) * totalLayers);
    return Math.max(Math.min(layers, totalLayers), 1);
  }, [
    activeSidecarReserveBytes,
    recContext,
    recData,
    recFile,
    recKvPlacement,
    recKvType,
    selectedRecommendedFile,
  ]);

  useEffect(() => {
    if (isOllamaMode) {
      setRecImageSupport(false);
      setRecMmprojFile("");
    }
  }, [isOllamaMode]);

  useEffect(() => {
    if (!mmprojFilesWithSize.length || isOllamaMode) {
      setRecImageSupport(false);
      setRecMmprojFile("");
      return;
    }

    setRecMmprojFile((current) => {
      if (current && mmprojFilesWithSize.some((file) => file.filename === current)) {
        return current;
      }
      return mmprojFilesWithSize[0]?.filename ?? "";
    });
  }, [mmprojFilesWithSize, isOllamaMode]);

  useEffect(() => {
    if (!mtpFilesWithSize.length) {
      setRecMtpFile("");
      if (!recMtpBundled) {
        setRecMtpSupport(false);
      }
      return;
    }

    setRecMtpFile((current) => {
      if (current && mtpFilesWithSize.some((file) => file.filename === current)) {
        return current;
      }
      return mtpFilesWithSize[0]?.filename ?? "";
    });
  }, [mtpFilesWithSize, recMtpBundled]);

  const sortedFilesWithSize = useMemo(() => {
    const sortedRunnable = [...runnableFilesWithSize].sort((a, b) => {
      const aScore = runabilityScores[a.filename]?.score;
      const bScore = runabilityScores[b.filename]?.score;

      if (aScore != null && bScore != null) {
        if (aScore !== bScore) return bScore - aScore;
        return a.filename.localeCompare(b.filename);
      }
      if (aScore != null) return -1;
      if (bScore != null) return 1;
      return a.filename.localeCompare(b.filename);
    });

    const sortedMmproj = [...mmprojFilesWithSize].sort((a, b) =>
      a.filename.localeCompare(b.filename),
    );

    const sortedMtp = [...mtpFilesWithSize].sort((a, b) => a.filename.localeCompare(b.filename));

    return [...sortedRunnable, ...sortedMtp, ...sortedMmproj];
  }, [mmprojFilesWithSize, mtpFilesWithSize, runabilityScores, runnableFilesWithSize]);

  const openCompareModal = useCallback(() => {
    if (!recData || recData.files.length === 0) return;
    const primary =
      recData.files.find((f) => f.filename === recFile)?.filename || recData.files[0].filename;
    const secondary = recData.files.find((f) => f.filename !== primary)?.filename;

    compareNextIdRef.current = 3;
    const initial: CompareSelection[] = [{ id: 1, filename: primary, kvType: recKvType }];
    if (secondary) {
      initial.push({ id: 2, filename: secondary, kvType: recKvType });
    }

    setCompareSelections(initial);
    setCompareOpen(true);
  }, [recData, recFile, recKvType]);

  const addCompareSelection = useCallback(() => {
    if (!recData || compareSelections.length >= 3 || recData.files.length === 0) return;
    const selected = new Set(compareSelections.map((s) => s.filename));
    const next = recData.files.find((f) => !selected.has(f.filename)) || recData.files[0];
    const id = compareNextIdRef.current++;
    setCompareSelections((prev) => [...prev, { id, filename: next.filename, kvType: recKvType }]);
  }, [compareSelections, recData, recKvType]);

  const updateCompareSelection = useCallback((id: number, patch: Partial<CompareSelection>) => {
    setCompareSelections((prev) =>
      prev.map((item) => (item.id === id ? { ...item, ...patch } : item)),
    );
  }, []);

  const removeCompareSelection = useCallback((id: number) => {
    setCompareSelections((prev) => prev.filter((item) => item.id !== id));
    delete compareScrollRefs.current[id];
  }, []);

  const handleCompareReportScroll = useCallback(
    (sourceId: number, event: React.UIEvent<HTMLDivElement>) => {
      // Ignore programmatic scroll events to prevent feedback loops.
      if (!event.nativeEvent.isTrusted) return;

      const sourceEl = event.currentTarget;
      const sourceMax = Math.max(sourceEl.scrollHeight - sourceEl.clientHeight, 0);
      compareSyncSourceIdRef.current = sourceId;
      compareSyncRatioRef.current = sourceMax > 0 ? sourceEl.scrollTop / sourceMax : 0;

      if (compareSyncRafRef.current != null) return;
      compareSyncRafRef.current = requestAnimationFrame(() => {
        const activeSourceId = compareSyncSourceIdRef.current;
        const ratio = compareSyncRatioRef.current;
        for (const [idStr, el] of Object.entries(compareScrollRefs.current)) {
          const id = Number(idStr);
          if (id === activeSourceId || !el) continue;
          const targetMax = Math.max(el.scrollHeight - el.clientHeight, 0);
          el.scrollTop = ratio * targetMax;
        }
        compareSyncRafRef.current = null;
      });
    },
    [],
  );

  const queueTrackedDownload = useCallback(
    async (
      modelId: string,
      filename: string,
      metadata: QueueDownloadMetadata | null = null,
      ollamaContext?: { providerId: string; quantization: string | null } | null,
    ) => {
      try {
        if (ollamaContext) {
          const tag =
            ollamaContext.quantization && ollamaContext.quantization.trim().length > 0
              ? `:${ollamaContext.quantization}`
              : "";
          const modelRef = `hf.co/${modelId}${tag}`;
          const augmentedMetadata: QueueDownloadMetadata = {
            ...(metadata ?? {}),
            displayName: metadata?.displayName ?? filename,
          };
          return await invoke<string>("ollama_pull_model", {
            credentialId: ollamaContext.providerId,
            modelRef,
            metadata: augmentedMetadata,
          });
        }
        return await invoke<string>("hf_queue_download", {
          modelId,
          filename,
          metadata,
        });
      } catch (err: any) {
        const message = typeof err === "string" ? err : err?.message || "";
        if (!ollamaContext && isHfGatedError(message)) {
          setGatedMenuOpen(true);
          return null;
        }
        toast.error(
          t("hfBrowser.downloadFailedToast"),
          message || t("hfBrowser.downloadFailedUnknown"),
        );
        return null;
      }
    },
    [t],
  );

  const autoCreateModelFromQueuedDownload = useCallback(
    async (
      modelItem: QueuedDownload,
      mmprojItem: QueuedDownload | null,
      mtpItem: QueuedDownload | null,
    ) => {
      if (!modelItem.resultPath) {
        return;
      }

      if (typeof modelItem.mmprojFile === "string" && !mmprojItem?.resultPath) {
        return;
      }
      if (typeof modelItem.mtpFile === "string" && !mtpItem?.resultPath) {
        return;
      }

      const installId = modelItem.installId || modelItem.id;
      const displayName =
        modelItem.displayName ||
        extractFileDisplayName(modelItem.filename) ||
        extractModelShortName(modelItem.modelId);
      const contextLength =
        modelItem.contextLength != null && modelItem.contextLength > 0
          ? Math.floor(modelItem.contextLength)
          : 8192;
      const kvType = modelItem.kvType || "q8_0";
      const mmprojPath = mmprojItem?.resultPath ?? null;
      const hasImageSupport = !!mmprojPath;
      const mtpPath = mtpItem?.resultPath ?? null;
      const hasMtpSupport = !!mtpPath || modelItem.mtpBundled === true;
      const modelOffloadValue =
        (modelItem.llamaModelOffloadMode as ModelOffload | null | undefined) ??
        gpuLayersToModelOffload(modelItem.llamaGpuLayers ?? null, null);
      const modelOffloadCopy = getModelOffloadCopy(t, modelOffloadValue);
      const persistedGpuLayers =
        modelOffloadValue === "cpu"
          ? 0
          : modelOffloadValue === "gpu"
            ? (modelItem.llamaGpuLayers ?? null)
            : null;
      const placementValue: KvPlacement =
        modelItem.llamaOffloadKqv === true
          ? "vram"
          : modelItem.llamaOffloadKqv === false
            ? "ram"
            : "auto";
      const placementCopy = getKvPlacementCopy(t, placementValue);

      try {
        const defaultAdvanced = createDefaultAdvancedModelSettings();
        await addOrUpdateModel({
          name: modelItem.resultPath,
          providerId: "llamacpp",
          providerLabel: "llama.cpp (Local)",
          displayName,
          inputScopes: hasImageSupport ? ["text", "image"] : ["text"],
          outputScopes: ["text"],
          advancedModelSettings: {
            ...defaultAdvanced,
            contextLength,
            llamaKvType: kvType as NonNullable<typeof defaultAdvanced.llamaKvType>,
            llamaGpuLayers: persistedGpuLayers,
            llamaOffloadKqv: modelItem.llamaOffloadKqv == null ? null : modelItem.llamaOffloadKqv,
            llamaMmprojPath: mmprojPath,
            llamaMtpEnabled: hasMtpSupport ? true : null,
            llamaMtpModelPath: mtpPath,
          },
        });

        const featureNotes = [
          hasImageSupport ? t("hfBrowser.featureImageSupport") : null,
          hasMtpSupport ? t("hfBrowser.featureMtp") : null,
        ]
          .filter(Boolean)
          .join(t("hfBrowser.featureJoin"));
        toast.success(
          t("hfBrowser.modelInstalled"),
          featureNotes
            ? t("hfBrowser.modelInstalledWithFeatures", {
                name: displayName,
                features: featureNotes,
                ctx: contextLength.toLocaleString(),
                kv: kvType.toUpperCase(),
                offload: modelOffloadCopy.label,
                placement: placementCopy.label,
              })
            : t("hfBrowser.modelInstalledNoFeatures", {
                name: displayName,
                ctx: contextLength.toLocaleString(),
                kv: kvType.toUpperCase(),
                offload: modelOffloadCopy.label,
                placement: placementCopy.label,
              }),
        );
        setHasPersistedLocalModel(true);

        await dismissItem(modelItem.id);
        if (mmprojItem) {
          await dismissItem(mmprojItem.id);
        }
        if (mtpItem) {
          await dismissItem(mtpItem.id);
        }
      } catch (err: any) {
        processedRecommendedInstallsRef.current.delete(installId);
        toast.error(
          t("hfBrowser.modelSetupFailed"),
          t("hfBrowser.modelSetupFailedBody", {
            name: displayName,
            error: err?.message || String(err),
          }),
        );
      }
    },
    [dismissItem, t],
  );

  const queueRecommendedDownload = useCallback(async () => {
    if (!modelInfo || !recData) return;
    const selectedFile = recData.files.find((f) => f.filename === recFile) ?? recData.files[0];
    if (!selectedFile) return;
    const selectedMmproj = selectedRecMmproj;
    if (recImageSupport && !selectedMmproj) {
      toast.error(
        t("hfBrowser.imageSupportUnavailable"),
        t("hfBrowser.imageSupportUnavailableBody"),
      );
      return;
    }

    const bpv = KV_BPV[recKvType] || 2;
    const maxAllowedContext = Math.max(
      maxContextForBpv(
        selectedFile.size,
        recData.kvBasePerToken,
        bpv,
        recData.totalAvailable,
        recData.modelMaxContext,
        activeSidecarReserveBytes,
      ),
      1024,
    );
    const requestedContext = Math.min(Math.max(recContext, 1024), maxAllowedContext);

    const installId = crypto.randomUUID();
    const displayName =
      extractFileDisplayName(selectedFile.filename) || extractModelShortName(modelInfo.modelId);
    const requestedModelOffload = gpuOptionsEnabled ? recModelOffload : "auto";
    const requestedGpuLayers = modelOffloadToGpuLayers(
      requestedModelOffload,
      gpuOffloadLayerCount(recData.arch?.blockCount),
      recommendedMixedGpuLayers,
    );

    const selectedMtp = selectedRecMtp;

    if (selectedMmproj) {
      const mmprojQueueId = await queueTrackedDownload(modelInfo.modelId, selectedMmproj.filename, {
        createModelWhenFinished: false,
        mmprojFile: false,
        installId,
        displayName,
        downloadRole: "mmproj",
      });
      if (!mmprojQueueId) {
        return;
      }
    }

    if (selectedMtp) {
      const mtpQueueId = await queueTrackedDownload(modelInfo.modelId, selectedMtp.filename, {
        createModelWhenFinished: false,
        mmprojFile: false,
        mtpFile: false,
        installId,
        displayName,
        downloadRole: "mtp",
      });
      if (!mtpQueueId) {
        return;
      }
    }

    const modelQueueId = await queueTrackedDownload(modelInfo.modelId, selectedFile.filename, {
      createModelWhenFinished: true,
      mmprojFile: selectedMmproj?.filename ?? false,
      mtpFile: selectedMtp?.filename ?? false,
      mtpBundled: recMtpSupport && recMtpBundled,
      installId,
      displayName,
      contextLength: requestedContext,
      kvType: recKvType,
      llamaOffloadKqv: gpuOptionsEnabled ? kvPlacementToOffloadKqv(recKvPlacement) : null,
      llamaGpuLayers: requestedGpuLayers,
      llamaModelOffloadMode: requestedModelOffload,
      downloadRole: "model",
    });
    if (!modelQueueId && selectedMmproj) {
      toast.error(
        t("hfBrowser.downloadPartiallyQueued"),
        t("hfBrowser.downloadPartiallyQueuedBody"),
      );
    }
  }, [
    mmprojFilesWithSize,
    mtpFilesWithSize,
    modelInfo,
    queueTrackedDownload,
    recContext,
    recData,
    recFile,
    recModelOffload,
    recImageSupport,
    recKvType,
    recKvPlacement,
    recMmprojFile,
    recMtpSupport,
    recMtpFile,
    recMtpBundled,
    selectedRecMmproj,
    selectedRecMtp,
    activeSidecarReserveBytes,
    gpuOptionsEnabled,
    recommendedMixedGpuLayers,
    t,
  ]);

  const queueFilesDownload = useCallback(
    async (file: HfModelFile) => {
      if (!modelInfo) return;

      const fileRecommendation =
        recData?.files.find((item) => item.filename === file.filename) ?? null;
      const requestedModelOffload = gpuOptionsEnabled ? recModelOffload : "auto";
      const requestedGpuLayers = fileRecommendation
        ? modelOffloadToGpuLayers(
            requestedModelOffload,
            gpuOffloadLayerCount(recData?.arch?.blockCount),
            recommendedMixedGpuLayers,
          )
        : null;

      const shouldCreateModel = Boolean(returnTo) && !file.isMmproj && !file.isMtp;
      const metadata: QueueDownloadMetadata | null = isImageMode
        ? { queueKind: "hf_image_manual" }
        : shouldCreateModel
        ? {
            createModelWhenFinished: true,
            installId: crypto.randomUUID(),
            displayName:
              extractFileDisplayName(file.filename) || extractModelShortName(modelInfo.modelId),
            contextLength: recContext,
            kvType: recKvType,
            llamaOffloadKqv: gpuOptionsEnabled ? kvPlacementToOffloadKqv(recKvPlacement) : null,
            llamaGpuLayers: requestedGpuLayers,
            llamaModelOffloadMode: requestedModelOffload,
            downloadRole: "model" as const,
          }
        : null;

      const ollamaContext =
        isOllamaMode && selectedOllamaProviderId
          ? { providerId: selectedOllamaProviderId, quantization: file.quantization }
          : null;
      await queueTrackedDownload(modelInfo.modelId, file.filename, metadata, ollamaContext);
    },
    [
      gpuOptionsEnabled,
      isImageMode,
      isOllamaMode,
      modelInfo,
      queueTrackedDownload,
      recContext,
      recData?.arch?.blockCount,
      recData?.files,
      recKvPlacement,
      recKvType,
      recModelOffload,
      recommendedMixedGpuLayers,
      returnTo,
      selectedOllamaProviderId,
    ],
  );

  useEffect(() => {
    for (const modelItem of queue) {
      if (
        !modelItem.createModelWhenFinished ||
        modelItem.downloadRole === "mmproj" ||
        modelItem.downloadRole === "mtp"
      ) {
        continue;
      }
      if (modelItem.status !== "complete") {
        continue;
      }

      const installId = modelItem.installId || modelItem.id;
      if (processedRecommendedInstallsRef.current.has(installId)) {
        continue;
      }

      let mmprojItem: QueuedDownload | null = null;
      if (typeof modelItem.mmprojFile === "string") {
        mmprojItem =
          queue.find(
            (item) =>
              item.installId === modelItem.installId &&
              item.downloadRole === "mmproj" &&
              item.filename === modelItem.mmprojFile,
          ) ?? null;
        if (!mmprojItem || mmprojItem.status !== "complete" || !mmprojItem.resultPath) {
          continue;
        }
      }

      let mtpItem: QueuedDownload | null = null;
      if (typeof modelItem.mtpFile === "string") {
        mtpItem =
          queue.find(
            (item) =>
              item.installId === modelItem.installId &&
              item.downloadRole === "mtp" &&
              item.filename === modelItem.mtpFile,
          ) ?? null;
        if (!mtpItem || mtpItem.status !== "complete" || !mtpItem.resultPath) {
          continue;
        }
      }

      processedRecommendedInstallsRef.current.add(installId);
      void autoCreateModelFromQueuedDownload(modelItem, mmprojItem, mtpItem);
    }
  }, [queue, autoCreateModelFromQueuedDownload]);

  useEffect(() => {
    const prev = prevQueueRef.current;

    for (const item of queue) {
      const prevItem = prev.find((p) => p.id === item.id);
      const becameComplete =
        (!prevItem || prevItem.status !== "complete") && item.status === "complete";
      const becameError = (!prevItem || prevItem.status !== "error") && item.status === "error";
      const becameCancelled =
        (!prevItem || prevItem.status !== "cancelled") && item.status === "cancelled";

      if (becameComplete && !item.installId) {
        toast.success(
          t("hfBrowser.downloadCompleteToast"),
          t("hfBrowser.downloadCompleteBody", { name: item.filename }),
        );
        void dismissItem(item.id);
      }

      if (becameError) {
        if (item.installId) {
          processedRecommendedInstallsRef.current.delete(item.installId);
        }
        toast.error(
          t("hfBrowser.downloadFailedToast"),
          t("hfBrowser.downloadFailedFileBody", {
            name: item.filename,
            error: item.error || t("hfBrowser.downloadFailedUnknown"),
          }),
        );
        if (!isOllamaMode && isHfGatedError(item.error || "")) setGatedMenuOpen(true);
      }

      if (becameCancelled && item.installId) {
        processedRecommendedInstallsRef.current.delete(item.installId);
      }
    }

    prevQueueRef.current = queue;
  }, [queue, dismissItem, t, isOllamaMode]);

  if (isMobilePlatform && providersLoaded && ollamaProviders.length === 0) {
    return (
      <div className="flex h-full flex-col items-center justify-center gap-3 px-8 text-center text-fg">
        <div className="flex h-12 w-12 items-center justify-center rounded-2xl border border-fg/10 bg-fg/5">
          <img src={OllamaIcon} alt="Ollama" className="h-6 w-6 opacity-70" />
        </div>
        <h2 className="text-[15px] font-semibold text-fg">{t("hfBrowser.mobileOllamaTitle")}</h2>
        <p className="max-w-xs text-[12.5px] leading-relaxed text-fg/55">
          {t("hfBrowser.destinationNoOllama")}
        </p>
        <button
          onClick={() => hfNavigate("/settings/providers")}
          className="mt-2 rounded-full border border-accent/40 bg-accent/15 px-5 py-2 text-[13px] font-medium text-accent transition hover:bg-accent/25 active:scale-[0.98]"
        >
          {t("hfBrowser.mobileOllamaCta")}
        </button>
      </div>
    );
  }

  return (
    <div className="flex h-full flex-col text-fg">
      <div className="flex-1">
        {!isMobilePlatform && (
          <div className="sticky top-0 z-20 flex gap-1 border-b border-fg/7 bg-surface/95 px-4 py-2 backdrop-blur">
            <button
              onClick={() => {
                setBrowserMode("llm");
                setSelectedOllamaProviderId(null);
                setView({ kind: "search" });
              }}
              className={cn("flex items-center gap-1.5 rounded-lg px-3 py-2 text-[11.5px] font-medium transition", browserMode === "llm" && !isOllamaMode ? "bg-accent/12 text-accent" : "text-fg/45 hover:bg-fg/5 hover:text-fg")}
            >
              <Cpu size={13} /> {t("hfBrowser.destinationLlamaCpp")}
            </button>
            <button
              onClick={() => {
                setBrowserMode("llm");
                setShowOllamaProviderMenu(true);
              }}
              className={cn("flex items-center gap-1.5 rounded-lg px-3 py-2 text-[11.5px] font-medium transition", isOllamaMode ? "bg-emerald-400/10 text-emerald-300" : "text-fg/45 hover:bg-fg/5 hover:text-fg")}
            >
              <img src={OllamaIcon} alt="Ollama" className="h-3.5 w-3.5" /> {t("hfBrowser.destinationOllama")}
            </button>
            <button
              onClick={() => {
                setBrowserMode("image");
                setView({ kind: "search" });
              }}
              className={cn("flex items-center gap-1.5 rounded-lg px-3 py-2 text-[11.5px] font-medium transition", browserMode === "image" ? "bg-accent/12 text-accent" : "text-fg/45 hover:bg-fg/5 hover:text-fg")}
            >
              <Layers size={13} /> {t("hfBrowser.destinationLocalImage")}
            </button>
            <button
              onClick={() => {
                setBrowserMode("imageBundle");
                setView({ kind: "search" });
              }}
              className={cn("flex items-center gap-1.5 rounded-lg px-3 py-2 text-[11.5px] font-medium transition", browserMode === "imageBundle" ? "bg-accent/12 text-accent" : "text-fg/45 hover:bg-fg/5 hover:text-fg")}
            >
              <Download size={13} /> {t("hfBrowser.destinationSelectDownload")}
            </button>
          </div>
        )}
        {isBundleMode && !bundle.profile ? (
          <BundleArchitecturePicker bundle={bundle} />
        ) : isBundleMode && bundle.ready && view.kind === "search" ? (
          <BundleReview
            bundle={bundle}
            onOpenModel={openInstalledImageModel}
            onRedoRole={handleRailRole}
          />
        ) : (
        <AnimatePresence mode="wait">
          {view.kind === "search" && (
            <motion.div
              key="search"
              initial={{ opacity: 0, x: -10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: -10 }}
              transition={{ duration: 0.15 }}
              className="flex flex-col"
            >
              {/* Search + filters */}
              <div className="sticky top-0 z-10 border-b border-fg/5 bg-surface/95 backdrop-blur px-4 py-3 space-y-3">
                {isBundleMode && bundle.profile && bundle.currentRole && (
                  <div className="space-y-2">
                    <div className="flex items-center justify-between gap-3">
                      <div className="min-w-0">
                        <div className="truncate text-[10px] font-semibold uppercase tracking-[0.16em] text-accent">
                          {t("hfBrowser.bundle.componentStep", {
                            current: bundle.requiredRoles.indexOf(bundle.currentRole) + 1,
                            total: bundle.requiredRoles.length,
                          })}
                          <span className="text-fg/35"> · {bundle.profile.displayName}</span>
                        </div>
                        <h2 className="mt-0.5 truncate text-[15px] font-semibold text-fg">
                          {t("hfBrowser.bundle.selectRole", {
                            role: bundle.roleLabel(bundle.currentRole),
                          })}
                        </h2>
                      </div>
                      <div className="flex shrink-0 items-center gap-2">
                        <button
                          onClick={bundle.resetProfile}
                          className="text-[11px] text-fg/45 transition hover:text-fg"
                        >
                          {t("hfBrowser.bundle.changeArchitecture")}
                        </button>
                      </div>
                    </div>
                    <div className="flex flex-wrap items-center gap-1.5">
                      {bundle.requiredRoles.map((role) => {
                        const asset = bundle.draft.selections[role];
                        const isCurrent = role === bundle.currentRole;
                        return (
                          <button
                            key={role}
                            disabled={!asset || !!bundle.draft.bundleId}
                            onClick={() => handleRailRole(role)}
                            title={
                              asset
                                ? `${asset.relativePath} (${formatBundleBytes(asset.size)})`
                                : undefined
                            }
                            className={cn(
                              "flex items-center gap-1.5 rounded-full border px-2.5 py-1 text-[10.5px] font-medium transition disabled:cursor-default",
                              asset
                                ? "border-emerald-400/25 bg-emerald-400/8 text-emerald-300 hover:border-emerald-400/45"
                                : isCurrent
                                  ? "border-accent/35 bg-accent/10 text-accent"
                                  : "border-fg/10 text-fg/35",
                            )}
                          >
                            {asset ? (
                              <Check size={10} />
                            ) : (
                              <span className="h-1 w-1 rounded-full bg-current" />
                            )}
                            {bundle.roleLabel(role)}
                          </button>
                        );
                      })}
                    </div>
                    <div className="flex flex-wrap items-center gap-1.5">
                      <span className="text-[10.5px] text-fg/35">
                        {t("hfBrowser.bundle.formatLabel")}
                      </span>
                      {(["gguf", "safetensors"] as const).map((format) => (
                        <button
                          key={format}
                          type="button"
                          onClick={() => setBundleFormat(format)}
                          title={t("hfBrowser.bundle.formatHint")}
                          className={cn(
                            "rounded-full border px-2.5 py-1 text-[10.5px] font-medium transition",
                            bundleFormat === format
                              ? "border-accent/35 bg-accent/10 text-accent"
                              : "border-fg/10 text-fg/35 hover:border-fg/25 hover:text-fg/60",
                          )}
                        >
                          {format === "gguf"
                            ? t("hfBrowser.bundle.formatGguf")
                            : t("hfBrowser.bundle.formatSafetensors")}
                        </button>
                      ))}
                    </div>
                  </div>
                )}
                {/* Search bar with inline destination */}
                <div className="flex items-stretch gap-2">
                  <div className="relative flex-1">
                    <Search
                      size={15}
                      strokeWidth={2.25}
                      className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-fg/40"
                    />
                    <input
                      ref={searchInputRef}
                      type="text"
                      value={query}
                      onChange={(e) => setQuery(e.target.value)}
                      onFocus={() => setSearchFocused(true)}
                      onBlur={() => setSearchFocused(false)}
                      placeholder={
                        isBundleMode
                          ? t("hfBrowser.bundle.searchPlaceholder")
                          : isImageMode
                            ? t("hfBrowser.bundle.repositoryPlaceholder")
                            : t("hfBrowser.searchPlaceholder")
                      }
                      className={cn(
                        "h-10 w-full rounded-xl border border-fg/10 bg-fg/4 pl-10 pr-9 text-[13px] text-fg placeholder-fg/35",
                        "transition focus:border-accent/40 focus:bg-fg/6 focus:outline-none focus:ring-1 focus:ring-accent/20",
                      )}
                    />
                    {query && (
                      <button
                        onClick={() => setQuery("")}
                        className="absolute right-2.5 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-md text-fg/40 transition hover:bg-fg/8 hover:text-fg/80"
                        aria-label={t("hfBrowser.clearSearch")}
                      >
                        <X size={13} />
                      </button>
                    )}
                  </div>

                  {!isBundleMode && (
                  <motion.div
                    className="flex shrink-0 items-stretch gap-2 overflow-hidden"
                    animate={{
                      width: collapseSearchNeighbors ? 0 : "auto",
                      opacity: collapseSearchNeighbors ? 0 : 1,
                      marginLeft: collapseSearchNeighbors ? -8 : 0,
                    }}
                    initial={false}
                    transition={{ type: "spring", stiffness: 380, damping: 36, mass: 0.7 }}
                  >
                    {/* Filter button */}
                    <button
                      onClick={() => setShowFilterMenu(true)}
                      className={cn(
                        "relative flex h-10 shrink-0 items-center gap-1.5 rounded-xl border px-3 text-[13px] font-medium transition",
                        activeFilterCount > 0
                          ? "border-accent/40 bg-accent/12 text-accent hover:border-accent/55"
                          : "border-fg/10 bg-fg/4 text-fg/70 hover:border-fg/25 hover:text-fg",
                      )}
                      aria-haspopup="menu"
                      aria-expanded={showFilterMenu}
                      aria-label={t("hfBrowser.filters")}
                      title={t("hfBrowser.filters")}
                    >
                      <SlidersHorizontal size={14} />
                      {activeFilterCount > 0 && (
                        <span className="rounded-full bg-accent/25 px-1.5 text-[10px] font-semibold leading-[1.4] tabular-nums">
                          {activeFilterCount}
                        </span>
                      )}
                    </button>

                    {/* Destination picker */}
                    <button
                      onClick={() => setShowOllamaProviderMenu(true)}
                      className={cn(
                        "group flex h-10 shrink-0 items-center gap-2 rounded-xl border px-3 text-[13px] font-medium transition",
                        isOllamaMode
                          ? "border-emerald-400/35 bg-emerald-500/10 text-emerald-200 hover:border-emerald-400/55 hover:bg-emerald-500/15"
                          : "border-fg/10 bg-fg/4 text-fg/70 hover:border-fg/25 hover:text-fg",
                      )}
                      aria-haspopup="menu"
                      aria-expanded={showOllamaProviderMenu}
                      aria-label={t("hfBrowser.destination")}
                    >
                      {isOllamaMode ? (
                        <img src={OllamaIcon} alt="Ollama" className="h-4 w-4 shrink-0" />
                      ) : (
                        <HardDrive size={14} className="text-fg/55" />
                      )}
                      <span className="max-w-[9rem] truncate">
                        {isOllamaMode
                          ? (selectedOllamaProvider?.label ?? t("hfBrowser.destinationOllama"))
                          : t("hfBrowser.destinationLocal")}
                      </span>
                      <ChevronDown
                        size={13}
                        className="shrink-0 opacity-50 transition group-hover:opacity-90"
                      />
                    </button>
                  </motion.div>
                  )}
                </div>

                {isBundleMode &&
                  bundle.currentRole &&
                  bundle.currentRole !== "diffusion_model" &&
                  query.trim().length > 0 &&
                  query.trim() ===
                    (bundle.profile?.recommendedRepositories[bundle.currentRole] ?? "") && (
                    <p className="text-[10.5px] text-fg/40">
                      {t("hfBrowser.bundle.recommendedPrefilled")}
                    </p>
                  )}

                {/* Sort segmented bar with sliding indicator */}
                <div className="flex gap-1 rounded-xl border border-fg/8 bg-fg/3 p-1">
                  {(
                    [
                      { key: "trending", icon: TrendingUp, label: t("hfBrowser.sortTrending") },
                      {
                        key: "downloads",
                        icon: ArrowDownToLine,
                        label: t("hfBrowser.sortDownloads"),
                      },
                      { key: "likes", icon: ThumbsUp, label: t("hfBrowser.sortLikes") },
                      { key: "lastModified", icon: Clock, label: t("hfBrowser.sortRecent") },
                    ] as const
                  ).map(({ key, icon: Icon, label }) => {
                    const active = sortMode === key;
                    return (
                      <button
                        key={key}
                        onClick={() => setSortMode(key)}
                        className={cn(
                          "relative flex items-center justify-center gap-1.5 whitespace-nowrap rounded-lg px-3 py-1.5 text-[12px] font-medium transition-colors",
                          active
                            ? "flex-1 text-accent"
                            : "flex-none text-fg/55 hover:text-fg/85 sm:flex-1",
                        )}
                        aria-label={label}
                        title={label}
                      >
                        {active && (
                          <motion.span
                            layoutId="hfSortIndicator"
                            className="absolute inset-0 rounded-lg bg-accent/15 ring-1 ring-inset ring-accent/30"
                            transition={{ type: "spring", stiffness: 420, damping: 34, mass: 0.6 }}
                          />
                        )}
                        <span className="relative z-10 flex items-center gap-1.5">
                          <Icon size={12} />
                          <span className={cn(!active && "hidden sm:inline")}>{label}</span>
                        </span>
                      </button>
                    );
                  })}
                </div>
              </div>

              {/* Content area */}
              <div className="px-4 py-3 space-y-2">
                {/* Inline download cards */}
                {hasDownloads && (
                  <InlineDownloadCards
                    showDivider={results.length > 0 || searching}
                    dividerLabel={t("hfBrowser.sortTrending")}
                  />
                )}

                {/* Loading state  */}
                {searching && !hasSearched && (
                  <div className="grid grid-cols-1 gap-1.5 md:grid-cols-2 xl:grid-cols-3">
                    {Array.from({ length: 12 }).map((_, i) => (
                      <div
                        key={i}
                        className="flex items-center gap-2.5 rounded-lg border border-fg/8 bg-fg/[0.02] px-2.5 py-2 animate-pulse"
                      >
                        <div className="h-6 w-6 shrink-0 rounded-full bg-fg/8" />
                        <div className="min-w-0 flex-1 space-y-1.5">
                          <div className="h-3 w-3/4 rounded bg-fg/8" />
                          <div className="h-2 w-1/2 rounded bg-fg/5" />
                        </div>
                        <div className="hidden sm:flex shrink-0 gap-2">
                          <div className="h-2.5 w-8 rounded bg-fg/5" />
                          <div className="h-2.5 w-6 rounded bg-fg/5" />
                        </div>
                      </div>
                    ))}
                  </div>
                )}

                {/* Searching spinner */}
                {searching && hasSearched && (
                  <div className="flex items-center justify-center gap-2 py-12 text-fg/50">
                    <Loader size={18} className="animate-spin" />
                    <span className="text-sm">{t("hfBrowser.searching")}</span>
                  </div>
                )}

                {searchError && (
                  <div className="flex flex-col items-center gap-2 py-16 text-center">
                    <AlertTriangle size={24} className="text-danger/70" />
                    <p className="text-sm text-fg/60">{searchError}</p>
                  </div>
                )}

                {!searching && !searchError && hasSearched && results.length === 0 && (
                  <div className="flex flex-col items-center gap-2 py-16 text-center">
                    <Search size={32} className="text-fg/20" />
                    <p className="text-sm font-medium text-fg/60">{t("hfBrowser.noResults")}</p>
                    <p className="text-xs text-fg/40">
                      {isBundleMode
                        ? t("hfBrowser.bundle.noCompatibleRepositories")
                        : t("hfBrowser.noResultsHint")}
                    </p>
                  </div>
                )}

                {!searching && results.length > 0 && displayedResults.length === 0 && (
                  <div className="flex flex-col items-center gap-2 py-12 text-center">
                    <SlidersHorizontal size={20} className="text-fg/30" />
                    <p className="text-sm text-fg/60">{t("hfBrowser.noResultsMatchFilters")}</p>
                    <button
                      onClick={() => {
                        setFilterPipelineTags(new Set());
                        setFilterParamMin("");
                        setFilterParamMax("");
                        setFilterAuthor("");
                      }}
                      className="mt-1 rounded-full border border-fg/15 bg-fg/5 px-3 py-1 text-[11px] font-medium text-fg/70 transition hover:border-fg/30"
                    >
                      {t("hfBrowser.clearFilters")}
                    </button>
                  </div>
                )}

                {!searching && displayedResults.length > 0 && (
                  <div
                    className={cn(
                      browserViewMode === "list" && "flex flex-col",
                      browserViewMode === "grid" &&
                        "grid grid-cols-1 gap-1.5 md:grid-cols-2 xl:grid-cols-3",
                      browserViewMode === "gallery" && "grid grid-cols-1 gap-2.5 md:grid-cols-2",
                    )}
                  >
                    {displayedResults.map((model) => {
                      const paramSize = extractParamSize(model.tags, model.modelId);
                      const avatarUrl = avatars[model.author];

                      if (browserViewMode === "list") {
                        return (
                          <button
                            key={model.modelId}
                            onClick={() => openModel(model.modelId)}
                            className="group flex items-center gap-2.5 border-b border-fg/[0.05] px-1.5 py-2 text-left transition hover:bg-fg/[0.03]"
                          >
                            {avatarUrl ? (
                              <img
                                src={avatarUrl}
                                alt={model.author}
                                className="h-5 w-5 shrink-0 rounded-full object-cover"
                                loading="lazy"
                              />
                            ) : (
                              <div
                                className={cn(
                                  "flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[11px] font-bold text-fg/75",
                                  authorColor(model.author),
                                )}
                              >
                                {model.author.charAt(0).toUpperCase()}
                              </div>
                            )}
                            <span className="min-w-0 flex-1 truncate text-[12.5px] font-medium text-fg">
                              {model.modelId}
                            </span>
                            <div className="hidden shrink-0 items-center gap-1.5 text-[10.5px] text-fg/45 md:flex">
                              {model.pipelineTag && (
                                <span className="truncate max-w-[8rem]">{model.pipelineTag}</span>
                              )}
                              {paramSize && (
                                <>
                                  <span className="text-fg/20">·</span>
                                  <span>{paramSize}</span>
                                </>
                              )}
                            </div>
                            <div className="flex shrink-0 items-center gap-2 text-[10.5px] tabular-nums text-fg/50">
                              <span
                                className="flex items-center gap-0.5"
                                title={t("hfBrowser.downloadsTitle")}
                              >
                                <ArrowDownToLine size={10} className="text-fg/35" />
                                {formatNumber(model.downloads)}
                              </span>
                              <span
                                className="flex items-center gap-0.5"
                                title={t("hfBrowser.likesTitle")}
                              >
                                <Heart size={10} className="text-fg/35" />
                                {formatNumber(model.likes)}
                              </span>
                            </div>
                          </button>
                        );
                      }

                      if (browserViewMode === "gallery") {
                        return (
                          <button
                            key={model.modelId}
                            onClick={() => openModel(model.modelId)}
                            className={cn(
                              "group flex items-stretch gap-3 rounded-xl border border-fg/10 bg-fg/[0.025] p-3 text-left transition",
                              "hover:border-fg/25 hover:bg-fg/[0.05] active:scale-[0.99]",
                            )}
                          >
                            {avatarUrl ? (
                              <img
                                src={avatarUrl}
                                alt={model.author}
                                className="h-12 w-12 shrink-0 self-center rounded-lg object-cover"
                                loading="lazy"
                              />
                            ) : (
                              <div
                                className={cn(
                                  "flex h-12 w-12 shrink-0 self-center items-center justify-center rounded-lg text-[18px] font-bold text-fg/75",
                                  authorColor(model.author),
                                )}
                              >
                                {model.author.charAt(0).toUpperCase()}
                              </div>
                            )}

                            <div className="flex min-w-0 flex-1 flex-col justify-center gap-1">
                              <div className="flex items-baseline gap-2 min-w-0">
                                <span className="truncate text-[13px] font-semibold text-fg">
                                  {model.modelId.split("/").pop()}
                                </span>
                                <span className="truncate text-[10.5px] text-fg/40">
                                  {model.author}
                                </span>
                              </div>

                              <div className="flex items-center gap-2 text-[10.5px] text-fg/55">
                                {model.pipelineTag && (
                                  <span className="truncate rounded bg-fg/8 px-1.5 py-0.5 font-medium">
                                    {model.pipelineTag}
                                  </span>
                                )}
                                {paramSize && (
                                  <span className="shrink-0 rounded bg-fg/8 px-1.5 py-0.5 font-medium">
                                    {paramSize}
                                  </span>
                                )}
                                {model.lastModified && (
                                  <span className="shrink-0 text-fg/40">
                                    {formatTimeAgo(model.lastModified)}
                                  </span>
                                )}
                              </div>

                              <div className="flex items-center gap-3 text-[10.5px] tabular-nums text-fg/50">
                                <span
                                  className="flex items-center gap-1"
                                  title={t("hfBrowser.downloadsTitle")}
                                >
                                  <ArrowDownToLine size={10} className="text-fg/35" />
                                  {formatNumber(model.downloads)}
                                </span>
                                <span
                                  className="flex items-center gap-1"
                                  title={t("hfBrowser.likesTitle")}
                                >
                                  <Heart size={10} className="text-fg/35" />
                                  {formatNumber(model.likes)}
                                </span>
                              </div>
                            </div>
                          </button>
                        );
                      }

                      // grid (default compact card)
                      return (
                        <button
                          key={model.modelId}
                          onClick={() => openModel(model.modelId)}
                          className={cn(
                            "group flex items-center gap-2.5 rounded-lg border border-fg/8 bg-fg/[0.02] px-2.5 py-2 text-left transition",
                            "hover:border-fg/20 hover:bg-fg/[0.05] active:scale-[0.99]",
                          )}
                        >
                          {avatarUrl ? (
                            <img
                              src={avatarUrl}
                              alt={model.author}
                              className="h-6 w-6 shrink-0 rounded-full object-cover"
                              loading="lazy"
                            />
                          ) : (
                            <div
                              className={cn(
                                "flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[12px] font-bold text-fg/75",
                                authorColor(model.author),
                              )}
                            >
                              {model.author.charAt(0).toUpperCase()}
                            </div>
                          )}

                          <div className="min-w-0 flex-1">
                            <div className="truncate text-[12.5px] font-medium text-fg">
                              {model.modelId}
                            </div>
                            <div className="mt-0.5 flex items-center gap-1 truncate text-[10.5px] text-fg/45">
                              {model.pipelineTag && (
                                <span className="truncate">{model.pipelineTag}</span>
                              )}
                              {paramSize && (
                                <>
                                  <span className="text-fg/20">·</span>
                                  <span className="shrink-0">{paramSize}</span>
                                </>
                              )}
                              {model.lastModified && (
                                <>
                                  <span className="text-fg/20">·</span>
                                  <span className="shrink-0 truncate">
                                    {formatTimeAgo(model.lastModified)}
                                  </span>
                                </>
                              )}
                            </div>
                          </div>

                          <div className="flex shrink-0 items-center gap-2.5 text-[10.5px] tabular-nums text-fg/50">
                            <span
                              className="flex items-center gap-1"
                              title={t("hfBrowser.downloadsTitle")}
                            >
                              <ArrowDownToLine size={10} className="text-fg/35" />
                              {formatNumber(model.downloads)}
                            </span>
                            <span
                              className="flex items-center gap-1"
                              title={t("hfBrowser.likesTitle")}
                            >
                              <Heart size={10} className="text-fg/35" />
                              {formatNumber(model.likes)}
                            </span>
                          </div>
                        </button>
                      );
                    })}
                  </div>
                )}

                {/* Load more button (hidden for direct URL lookups) */}
                {!searching && hasSearched && results.length > 0 && hasMore && !isDirectLookup && (
                  <div className="flex justify-center pt-2 pb-4">
                    <button
                      onClick={loadMore}
                      disabled={loadingMore}
                      className={cn(
                        "flex items-center gap-2 rounded-xl border border-fg/10 bg-fg/5 px-6 py-2.5 text-sm font-medium text-fg/60 transition",
                        "hover:border-fg/20 hover:bg-fg/10 hover:text-fg/80 active:scale-[0.98]",
                        loadingMore && "pointer-events-none opacity-60",
                      )}
                    >
                      {loadingMore ? (
                        <>
                          <Loader size={14} className="animate-spin" />
                          {t("hfBrowser.loading")}
                        </>
                      ) : (
                        t("hfBrowser.loadMore")
                      )}
                    </button>
                  </div>
                )}

                {/* End of results indicator (not for direct lookups) */}
                {!searching && hasSearched && results.length > 0 && !hasMore && !isDirectLookup && (
                  <p className="py-4 text-center text-[11px] text-fg/25">
                    {t("hfBrowser.noMoreResults")}
                  </p>
                )}
              </div>
            </motion.div>
          )}

          {view.kind === "author" && (
            <motion.div
              key="author"
              initial={{ opacity: 0, x: 10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 10 }}
              transition={{ duration: 0.15 }}
              className="flex flex-col px-4 py-4"
            >
              <AuthorView
                author={view.author}
                onBack={() => setView({ kind: "search" })}
                onOpenModel={openModel}
              />
            </motion.div>
          )}

          {view.kind === "model" && (
            <motion.div
              key="model"
              initial={{ opacity: 0, x: 10 }}
              animate={{ opacity: 1, x: 0 }}
              exit={{ opacity: 0, x: 10 }}
              transition={{ duration: 0.15 }}
              className="flex flex-col"
            >
              {/* Loading state */}
              {loadingFiles && !modelInfo && (
                <div className="flex items-center justify-center gap-2 py-20 text-fg/50">
                  <Loader size={18} className="animate-spin" />
                  <span className="text-sm">{t("hfBrowser.loadingModelInfo")}</span>
                </div>
              )}

              {/* Error state */}
              {filesError && !modelInfo && (
                <div className="flex flex-col items-center gap-3 px-4 py-20 text-center">
                  <AlertTriangle size={24} className="text-danger/70" />
                  <p className="text-sm text-fg/60">{filesError}</p>
                  <button
                    onClick={() => setView({ kind: "search" })}
                    className="text-xs text-accent hover:underline"
                  >
                    {t("hfBrowser.backToSearch")}
                  </button>
                </div>
              )}

              {/* Main content */}
              {modelInfo && (
                <div className="flex items-stretch">
                  <div className="flex-1 min-w-0">
                    {/* Model header card */}
                    <div className="border-b border-fg/5 px-4 py-4">
                      <div className="flex items-center gap-2">
                        <h1
                          className={cn(typography.h1.size, typography.h1.weight, "text-fg flex-1")}
                        >
                          {extractModelShortName(modelInfo.modelId)}
                        </h1>
                        <a
                          href={`https://huggingface.co/${modelInfo.modelId}`}
                          target="_blank"
                          rel="noreferrer"
                          className="flex shrink-0 items-center gap-1 text-[11px] text-accent/70 hover:text-accent transition"
                        >
                          <ExternalLink size={12} />
                          HuggingFace
                        </a>
                      </div>
                      <button
                        type="button"
                        disabled={isBundleMode}
                        onClick={() => setView({ kind: "author", author: modelInfo.author })}
                        className="mt-0.5 block text-left text-xs text-fg/45 transition enabled:hover:text-accent"
                      >
                        {modelInfo.author}
                      </button>

                      {/* Stats row */}
                      <div className="mt-3 flex flex-wrap gap-2">
                        {modelInfo.architecture && (
                          <div className="flex items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-xs text-fg/70">
                            <Cpu size={12} className="text-accent/70" />
                            {modelInfo.architecture}
                          </div>
                        )}
                        {modelInfo.contextLength != null && (
                          <div className="flex items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-xs text-fg/70">
                            <BookOpen size={12} className="text-info/70" />
                            {formatNumber(modelInfo.contextLength)} ctx
                          </div>
                        )}
                        {modelInfo.parameterCount != null && (
                          <div className="flex items-center gap-1.5 rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-1.5 text-xs text-fg/70">
                            <Layers size={12} className="text-secondary/70" />
                            {formatParams(modelInfo.parameterCount)} params
                          </div>
                        )}
                      </div>

                      <div className="mt-2 flex items-center gap-4 text-xs text-fg/45">
                        <span className="flex items-center gap-1">
                          <Heart size={11} className="text-pink-400/70" />
                          {formatNumber(modelInfo.likes)} {t("hfBrowser.likes")}
                        </span>
                        <span className="flex items-center gap-1">
                          <ArrowDownToLine size={11} className="text-blue-400/70" />
                          {formatNumber(modelInfo.downloads)} {t("hfBrowser.downloads")}
                        </span>
                      </div>
                    </div>

                    {/* Download cards on model detail page */}
                    {hasDownloads && (
                      <div className="border-b border-fg/5 px-4 py-3">
                        <InlineDownloadCards />
                      </div>
                    )}

                    {/* README content */}
                    <div className="px-4 py-4 pb-24">
                      {readmeLoading && (
                        <div className="flex items-center justify-center gap-2 py-12 text-fg/40">
                          <Loader size={16} className="animate-spin" />
                          <span className="text-xs">{t("hfBrowser.loadingReadme")}</span>
                        </div>
                      )}

                      {!readmeLoading && readme && <HfReadmeRenderer content={readme} />}

                      {!readmeLoading && !readme && (
                        <div className="flex flex-col items-center gap-2 py-12 text-center text-fg/30">
                          <FileText size={32} />
                          <p className="text-sm">{t("hfBrowser.noReadme")}</p>
                        </div>
                      )}
                    </div>
                  </div>

                  {(isBundleMode || filesWithSize.length > 0) && (
                    <>
                      {isMobilePlatform && (
                        <>
                          <button
                            onClick={() => setFilesDrawerOpen(true)}
                            className={cn(
                              "fixed bottom-6 right-4 z-90 flex items-center gap-2 rounded-full border border-accent/30 bg-surface-el px-4 py-2.5 text-[12px] font-semibold text-accent shadow-lg transition active:scale-95",
                              filesDrawerOpen && "pointer-events-none opacity-0",
                            )}
                          >
                            <FileText size={13} />
                            {t("hfBrowser.files")} ({filesWithSize.length})
                          </button>
                          <div
                            className={cn(
                              "fixed inset-0 z-100 bg-black/45 backdrop-blur-sm transition-opacity duration-200",
                              filesDrawerOpen ? "opacity-100" : "pointer-events-none opacity-0",
                            )}
                            onClick={() => setFilesDrawerOpen(false)}
                            aria-hidden="true"
                          />
                        </>
                      )}
                      <div
                        className={cn(
                          isMobilePlatform
                            ? "fixed bottom-0 right-0 top-[var(--titlebar-h,0px)] z-110 flex w-[min(90vw,24rem)] flex-col border-l border-fg/10 bg-surface-el pt-[env(safe-area-inset-top)] transition-transform duration-300 ease-out"
                            : "w-84 shrink-0 border-l border-fg/10 bg-surface/50 relative",
                          isMobilePlatform && !filesDrawerOpen && "translate-x-full",
                        )}
                      >
                        {isMobilePlatform && (
                          <div className="flex shrink-0 items-center justify-between gap-3 border-b border-fg/10 px-4 py-3">
                            <span className="truncate text-[13px] font-semibold text-fg">
                              {extractModelShortName(modelInfo.modelId)}
                            </span>
                            <button
                              onClick={() => setFilesDrawerOpen(false)}
                              className="flex h-7 w-7 shrink-0 items-center justify-center rounded-full border border-fg/10 bg-fg/5 text-fg/70 transition hover:bg-fg/10 hover:text-fg"
                              aria-label={t("components.bottomMenu.closeLabel")}
                            >
                              <X size={13} />
                            </button>
                          </div>
                        )}
                        <div
                          ref={isMobilePlatform ? undefined : filesPanelRef}
                          className={cn(
                            "flex flex-col overflow-hidden will-change-transform rounded-b-xl",
                            isMobilePlatform && "min-h-0 flex-1",
                          )}
                        >
                          {isBundleMode ? (
                            <BundleFilePicker
                              bundle={bundle}
                              modelId={modelInfo.modelId}
                              onSelect={handleBundleSelect}
                              onAddToken={() => setGatedMenuOpen(true)}
                            />
                          ) : (
                            <>
                          <div className="border-b border-fg/10 px-3 py-2 space-y-2">
                            {isOllamaMode && !sproutActive ? (
                              <>
                                <div className="rounded-md border border-amber-400/20 bg-amber-500/5 px-3 py-2 text-[12px] leading-relaxed text-amber-300/95">
                                  <div className="flex items-start gap-2">
                                    <AlertTriangle size={13} className="mt-0.5 shrink-0" />
                                    <div className="space-y-0.5">
                                      <div className="font-medium">
                                        {t("hfBrowser.ollamaModeNoticeTitle")}
                                      </div>
                                      <div className="text-amber-300/80">
                                        {t("hfBrowser.ollamaModeNoticeBody")}
                                      </div>
                                      <button
                                        type="button"
                                        onClick={() =>
                                          void openExternalUrl(
                                            "https://github.com/LettuceAI/Sprout",
                                          )
                                        }
                                        className="mt-1 inline-flex items-center gap-1 font-medium text-amber-200 underline underline-offset-2 hover:text-amber-100"
                                      >
                                        <ExternalLink size={11} />
                                        {t("hfBrowser.getSprout")}
                                      </button>
                                    </div>
                                  </div>
                                </div>
                                <div className="rounded-lg border border-fg/10 bg-fg/5 px-3 py-2 text-center text-[12px] font-medium text-fg/60">
                                  {t("hfBrowser.files")} ({filesWithSize.length})
                                </div>
                              </>
                            ) : (
                              <div
                                className="grid grid-cols-2 gap-1 rounded-lg border border-fg/10 bg-fg/5 p-1"
                                data-tour-id="hf-rec-tabs"
                              >
                                <button
                                  type="button"
                                  onClick={() => setFilesPanelTab("recommended")}
                                  className={cn(
                                    "rounded-md px-2 py-1.5 text-[11px] font-medium transition-colors",
                                    filesPanelTab === "recommended"
                                      ? "bg-emerald-400/15 text-emerald-400"
                                      : "text-fg/50 hover:text-fg/70",
                                  )}
                                >
                                  {t("hfBrowser.recommendedSettings")}
                                </button>
                                <button
                                  type="button"
                                  onClick={() => setFilesPanelTab("files")}
                                  className={cn(
                                    "rounded-md px-2 py-1.5 text-[11px] font-medium transition-colors",
                                    filesPanelTab === "files"
                                      ? "bg-emerald-400/15 text-emerald-400"
                                      : "text-fg/50 hover:text-fg/70",
                                  )}
                                >
                                  {t("hfBrowser.files")} ({filesWithSize.length})
                                </button>
                              </div>
                            )}
                          </div>

                          {/* Recommended settings tab */}
                          {filesPanelTab === "recommended" && (
                            <div className="flex-1 overflow-y-auto px-3 py-3 pb-6">
                              <div
                                className="rounded-xl border border-fg/10 bg-fg/3 px-3 py-2.5"
                                data-tour-id="hf-rec-panel"
                              >
                                {recLoading || !recData ? (
                                  <div className="space-y-2.5 animate-pulse">
                                    <div className="h-8 w-24 rounded bg-fg/10" />
                                    <div className="h-12 rounded-lg bg-fg/10" />
                                    <div className="h-3 w-28 rounded bg-fg/10" />
                                    <div className="h-9 rounded-md bg-fg/10" />
                                    <div className="h-3 w-28 rounded bg-fg/10" />
                                    <div className="h-2 rounded bg-fg/10" />
                                    <div className="h-9 rounded-md bg-fg/10" />
                                    <div className="h-8 rounded-lg bg-fg/10" />
                                  </div>
                                ) : (
                                  (() => {
                                    const selFile =
                                      recData.files.find((f) => f.filename === recFile) ||
                                      recData.files[0];
                                    if (!selFile) return null;

                                    const totalAvail = recData.totalAvailable;
                                    const bpvSel = KV_BPV[recKvType] || 2;
                                    const maxCtx = Math.max(
                                      maxContextForBpv(
                                        selFile.size,
                                        recData.kvBasePerToken,
                                        bpvSel,
                                        totalAvail,
                                        recData.modelMaxContext,
                                        activeSidecarReserveBytes,
                                      ),
                                      1024,
                                    );
                                    const clampedCtx = Math.min(Math.max(recContext, 1024), maxCtx);

                                    const effectiveKvCtx = recData.kvContextCap
                                      ? Math.min(clampedCtx, recData.kvContextCap)
                                      : clampedCtx;
                                    const kvBytes = recData.kvBasePerToken
                                      ? recData.kvBasePerToken * bpvSel * effectiveKvCtx
                                      : 0;
                                    const { score, gpuMode } = calcScore(
                                      selFile.size,
                                      selFile.quantQuality,
                                      kvBytes,
                                      recData.availableRam,
                                      totalAvail,
                                      recData.availableVram,
                                      recModelOffload,
                                      recKvPlacement,
                                      activeSidecarReserveBytes,
                                    );
                                    const modeCopy = getRunabilityModeCopy(t, gpuMode);
                                    const runStatus = getRunStatus(t, score);
                                    const totalNeeded =
                                      selFile.size +
                                      kvBytes +
                                      computeOverhead(selFile.size) +
                                      activeSidecarReserveBytes;
                                    const remainingHeadroom = Math.max(totalAvail - totalNeeded, 0);
                                    const headroomStatus = getHeadroomStatus(
                                      t,
                                      totalNeeded,
                                      totalAvail,
                                    );
                                    const perfStatus = getPerformanceStatus(t, gpuMode);
                                    const statusRow = (
                                      labelText: string,
                                      value: ReactNode,
                                      tone?: string,
                                    ) => (
                                      <div className="flex items-center justify-between gap-3 py-1.5">
                                        <span className="text-[12px] text-fg/45">{labelText}</span>
                                        <span
                                          className={cn(
                                            "min-w-0 text-right text-[12px] font-medium",
                                            tone || "text-fg/75",
                                          )}
                                        >
                                          {value}
                                        </span>
                                      </div>
                                    );

                                    const upgradeSuggestion = (() => {
                                      if (selFile.quantQuality >= 90) return null;
                                      const bpvVal = KV_BPV[recKvType] || 2;
                                      let best: { file: FileRecommendation; score: number } | null =
                                        null;
                                      for (const f of recData.files) {
                                        if (f.quantQuality <= selFile.quantQuality) continue;
                                        if (f.filename === selFile.filename) continue;
                                        const fMaxCtx = Math.max(
                                          maxContextForBpv(
                                            f.size,
                                            recData.kvBasePerToken,
                                            bpvVal,
                                            totalAvail,
                                            recData.modelMaxContext,
                                            activeSidecarReserveBytes,
                                          ),
                                          1024,
                                        );
                                        if (fMaxCtx < 1024) continue;
                                        const fCtx = Math.min(clampedCtx, fMaxCtx);
                                        const fEffKvCtx = recData.kvContextCap
                                          ? Math.min(fCtx, recData.kvContextCap)
                                          : fCtx;
                                        const fKv = recData.kvBasePerToken
                                          ? recData.kvBasePerToken * bpvVal * fEffKvCtx
                                          : 0;
                                        const { score: fScore } = calcScore(
                                          f.size,
                                          f.quantQuality,
                                          fKv,
                                          recData.availableRam,
                                          totalAvail,
                                          recData.availableVram,
                                          recModelOffload,
                                          recKvPlacement,
                                          activeSidecarReserveBytes,
                                        );
                                        if (fScore < 70) continue;
                                        if (
                                          !best ||
                                          f.quantQuality > best.file.quantQuality ||
                                          (f.quantQuality === best.file.quantQuality &&
                                            fScore > best.score)
                                        ) {
                                          best = { file: f, score: fScore };
                                        }
                                      }
                                      return best;
                                    })();

                                    return (
                                      <div className="mt-2 space-y-2.5">
                                        <div className="rounded-lg border border-fg/10 bg-fg/[0.035] px-3 py-2.5">
                                          <div className="flex items-start justify-between gap-3 border-b border-fg/8 pb-2">
                                            <div className="min-w-0">
                                              <div className="text-[14px] font-semibold text-fg">
                                                {modeCopy.short}
                                              </div>
                                              <p className="mt-0.5 text-[12px] leading-snug text-fg/50">
                                                {modeCopy.long}
                                              </p>
                                            </div>
                                            <div className="text-right shrink-0">
                                              <div
                                                className={cn(
                                                  "text-[13px] font-semibold",
                                                  runStatus.tone,
                                                )}
                                              >
                                                {runStatus.label}
                                              </div>
                                              <div className="mt-0.5 flex items-center justify-end gap-1 text-[11px] text-fg/35">
                                                <Monitor size={9} />
                                                {t("hfBrowser.willRun")}
                                              </div>
                                            </div>
                                          </div>
                                          <div className="mt-1.5 divide-y divide-fg/6">
                                            {statusRow(
                                              t("hfBrowser.statusHeadroom"),
                                              t("hfBrowser.headroomLeft", {
                                                label: headroomStatus.label,
                                                size: formatBytes(remainingHeadroom),
                                              }),
                                              headroomStatus.tone,
                                            )}
                                            {statusRow(
                                              t("hfBrowser.statusPrefill"),
                                              perfStatus.prefill.label,
                                              perfStatus.prefill.tone,
                                            )}
                                            {statusRow(
                                              t("hfBrowser.statusGeneration"),
                                              perfStatus.generation.label,
                                              perfStatus.generation.tone,
                                            )}
                                            {statusRow(
                                              t("hfBrowser.statusConfidence"),
                                              t("hfBrowser.confidenceValue", { score }),
                                              runStatus.tone,
                                            )}
                                          </div>
                                        </div>
                                        <p className="text-[12px] leading-snug text-fg/45">
                                          {headroomStatus.description}
                                        </p>

                                        {/* Upgrade suggestion */}
                                        {upgradeSuggestion && (
                                          <button
                                            onClick={() => {
                                              setRecFile(upgradeSuggestion.file.filename);
                                              const mx = Math.max(
                                                maxContextForBpv(
                                                  upgradeSuggestion.file.size,
                                                  recData.kvBasePerToken,
                                                  bpvSel,
                                                  totalAvail,
                                                  recData.modelMaxContext,
                                                  activeSidecarReserveBytes,
                                                ),
                                                1024,
                                              );
                                              setRecContext(Math.min(recContext, mx));
                                            }}
                                            className="flex w-full items-start gap-2 rounded-lg border border-emerald-400/20 bg-emerald-400/5 px-2.5 py-2 text-left transition hover:bg-emerald-400/10 active:scale-[0.98]"
                                          >
                                            <TrendingUp
                                              size={11}
                                              className="text-emerald-400 shrink-0 mt-0.5"
                                            />
                                            <div className="flex-1 min-w-0">
                                              <p className="text-[12px] leading-snug text-emerald-400/90">
                                                {t("hfBrowser.upgradeSuggestion", {
                                                  quant: upgradeSuggestion.file.quantization,
                                                  size: formatBytes(upgradeSuggestion.file.size),
                                                  score: upgradeSuggestion.score.toString(),
                                                })}
                                              </p>
                                            </div>
                                          </button>
                                        )}

                                        {/* Quantization */}
                                        <div data-tour-id="hf-rec-quant">
                                          <label className="text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                                            {t("hfBrowser.quantization")}
                                          </label>
                                          <select
                                            value={recFile}
                                            onChange={(e) => {
                                              setRecFile(e.target.value);
                                              const f = recData.files.find(
                                                (x) => x.filename === e.target.value,
                                              );
                                              if (f) {
                                                const mx = Math.max(
                                                  maxContextForBpv(
                                                    f.size,
                                                    recData.kvBasePerToken,
                                                    bpvSel,
                                                    totalAvail,
                                                    recData.modelMaxContext,
                                                    activeSidecarReserveBytes,
                                                  ),
                                                  1024,
                                                );
                                                const optimalGpuCtx = computeGpuOptimalContext(
                                                  f.size,
                                                  recData.kvBasePerToken,
                                                  bpvSel,
                                                  recData.availableVram,
                                                  recData.modelMaxContext,
                                                  recData.kvContextCap,
                                                  activeSidecarReserveBytes,
                                                );
                                                const optimalRamCtx = computeRamMaxContext(
                                                  f.size,
                                                  recData.kvBasePerToken,
                                                  bpvSel,
                                                  totalAvail,
                                                  recData.modelMaxContext,
                                                  recData.kvContextCap,
                                                  activeSidecarReserveBytes,
                                                );
                                                const optimal =
                                                  optimalGpuCtx > 0
                                                    ? optimalGpuCtx
                                                    : optimalRamCtx > 0
                                                      ? optimalRamCtx
                                                      : 8192;
                                                setRecContext(Math.min(optimal, mx));
                                              }
                                            }}
                                            className="mt-1 w-full rounded-md border border-fg/10 bg-fg/5 px-2 py-1.5 text-[11px] text-fg focus:border-fg/25 focus:outline-none"
                                          >
                                            {recData.files.map((f) => (
                                              <option key={f.filename} value={f.filename}>
                                                {f.quantization} — {formatBytes(f.size)}
                                              </option>
                                            ))}
                                          </select>
                                        </div>

                                        {mmprojFilesWithSize.length > 0 && isOllamaMode && (
                                          <div className="rounded-lg border border-amber-400/20 bg-amber-500/5 px-2.5 py-2 text-[11px] leading-relaxed text-amber-300/80">
                                            {t("hfBrowser.ollamaVisionUnsupported")}
                                          </div>
                                        )}
                                        {mmprojFilesWithSize.length > 0 && !isOllamaMode && (
                                          <>
                                            <div className="flex items-center justify-between gap-3 rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-2">
                                              <div className="min-w-0">
                                                <span className="block text-[11px] font-medium text-fg/85">
                                                  {t("hfBrowser.imageSupport")}
                                                </span>
                                                <span className="block whitespace-nowrap text-[11px] text-fg/40">
                                                  {t("hfBrowser.imageSupportHint")}
                                                </span>
                                              </div>
                                              <div className="flex items-center gap-2">
                                                <span className="text-[10px] font-medium uppercase tracking-[0.12em] text-fg/35">
                                                  {recImageSupport
                                                    ? t("common.labels.on")
                                                    : t("common.labels.off")}
                                                </span>
                                                <Switch
                                                  id="hf-image-support-toggle"
                                                  checked={recImageSupport}
                                                  onChange={setRecImageSupport}
                                                />
                                              </div>
                                            </div>

                                            {recImageSupport && (
                                              <div>
                                                <label className="text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                                                  {t("hfBrowser.mmproj")}
                                                </label>
                                                <select
                                                  value={recMmprojFile}
                                                  onChange={(e) => setRecMmprojFile(e.target.value)}
                                                  className="mt-1 w-full rounded-md border border-fg/10 bg-fg/5 px-2 py-1.5 text-[11px] text-fg focus:border-fg/25 focus:outline-none"
                                                >
                                                  {mmprojFilesWithSize.map((file) => (
                                                    <option
                                                      key={file.filename}
                                                      value={file.filename}
                                                    >
                                                      {file.quantization} — {formatBytes(file.size)}
                                                    </option>
                                                  ))}
                                                </select>
                                              </div>
                                            )}
                                          </>
                                        )}

                                        {recMtpAvailable && (
                                          <>
                                            <div className="flex items-center justify-between gap-3 rounded-lg border border-fg/10 bg-fg/5 px-2.5 py-2">
                                              <div className="min-w-0">
                                                <span className="block text-[11px] font-medium text-fg/85">
                                                  {t("hfBrowser.mtpTitle")}
                                                </span>
                                                <span className="block whitespace-nowrap text-[11px] text-fg/40">
                                                  {recMtpBundled
                                                    ? t("hfBrowser.mtpHintBundled")
                                                    : t("hfBrowser.mtpHintSidecar")}
                                                </span>
                                              </div>
                                              <div className="flex items-center gap-2">
                                                <span className="text-[10px] font-medium uppercase tracking-[0.12em] text-fg/35">
                                                  {recMtpSupport
                                                    ? t("common.labels.on")
                                                    : t("common.labels.off")}
                                                </span>
                                                <Switch
                                                  id="hf-mtp-support-toggle"
                                                  checked={recMtpSupport}
                                                  onChange={setRecMtpSupport}
                                                />
                                              </div>
                                            </div>

                                            {recMtpSupport && !recMtpBundled && (
                                              <div>
                                                <label className="text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                                                  {t("hfBrowser.mtpFile")}
                                                </label>
                                                <select
                                                  value={recMtpFile}
                                                  onChange={(e) => setRecMtpFile(e.target.value)}
                                                  className="mt-1 w-full rounded-md border border-fg/10 bg-fg/5 px-2 py-1.5 text-[11px] text-fg focus:border-fg/25 focus:outline-none"
                                                >
                                                  {mtpFilesWithSize.map((file) => (
                                                    <option
                                                      key={file.filename}
                                                      value={file.filename}
                                                    >
                                                      {file.filename} — {formatBytes(file.size)}
                                                    </option>
                                                  ))}
                                                </select>
                                              </div>
                                            )}
                                          </>
                                        )}

                                        {/* Context length */}
                                        {(() => {
                                          const modelMax = recData.modelMaxContext;
                                          const showGpuGuidance =
                                            gpuOptionsEnabled && recModelOffload !== "cpu";
                                          const warnOnGpuCtxOverflow =
                                            showGpuGuidance && recKvPlacement !== "ram";
                                          // Max context for 100% GPU offload (model+KV+compute all in VRAM)
                                          const fullGpuCtx = computeGpuOptimalContext(
                                            selFile.size,
                                            recData.kvBasePerToken,
                                            KV_BPV[recKvType] || 2,
                                            recData.availableVram,
                                            modelMax,
                                            recData.kvContextCap,
                                            activeSidecarReserveBytes,
                                          );
                                          // Max context before RAM runs out (dynamic for current KV type)
                                          const ramCtx = computeRamMaxContext(
                                            selFile.size,
                                            recData.kvBasePerToken,
                                            KV_BPV[recKvType] || 2,
                                            totalAvail,
                                            modelMax,
                                            recData.kvContextCap,
                                            activeSidecarReserveBytes,
                                          );

                                          return (
                                            <div data-tour-id="hf-rec-context">
                                              <div className="flex items-center justify-between">
                                                <label className="text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                                                  {t("hfBrowser.contextLength")}
                                                </label>
                                                <span className="text-[12px] font-mono text-fg/60">
                                                  {clampedCtx.toLocaleString()}
                                                </span>
                                              </div>
                                              <div className="relative mt-1">
                                                <input
                                                  type="range"
                                                  min={1024}
                                                  max={maxCtx}
                                                  step={256}
                                                  value={clampedCtx}
                                                  onChange={(e) =>
                                                    setRecContext(Number(e.target.value))
                                                  }
                                                  className="w-full accent-accent h-1.5"
                                                />
                                                {/* Optimal context tick marks below slider */}
                                                {maxCtx > 1024 &&
                                                  (() => {
                                                    const pct = (v: number) =>
                                                      ((v - 1024) / (maxCtx - 1024)) * 100;
                                                    return (
                                                      <>
                                                        {showGpuGuidance &&
                                                          fullGpuCtx > 1024 &&
                                                          fullGpuCtx < maxCtx && (
                                                            <div
                                                              className="absolute bottom-0 w-0.5 bg-emerald-400 rounded-full pointer-events-none"
                                                              style={{
                                                                left: `${pct(fullGpuCtx)}%`,
                                                                height: 6,
                                                                transform: "translateY(100%)",
                                                              }}
                                                            />
                                                          )}
                                                        {ramCtx > 1024 &&
                                                          ramCtx < maxCtx &&
                                                          ramCtx !== fullGpuCtx && (
                                                            <div
                                                              className="absolute bottom-0 w-0.5 bg-amber-400 rounded-full pointer-events-none"
                                                              style={{
                                                                left: `${pct(ramCtx)}%`,
                                                                height: 6,
                                                                transform: "translateY(100%)",
                                                              }}
                                                            />
                                                          )}
                                                      </>
                                                    );
                                                  })()}
                                              </div>
                                              <div className="flex justify-between text-[9px] text-fg/30">
                                                <span>1,024</span>
                                                <span>{maxCtx.toLocaleString()}</span>
                                              </div>
                                              {/* Clickable context presets */}
                                              {((showGpuGuidance && fullGpuCtx > 0) ||
                                                ramCtx > 0) && (
                                                <div className="flex flex-wrap gap-x-3 gap-y-1 mt-1.5">
                                                  {showGpuGuidance &&
                                                    fullGpuCtx > 0 &&
                                                    fullGpuCtx < maxCtx && (
                                                      <button
                                                        type="button"
                                                        onClick={() =>
                                                          setRecContext(
                                                            Math.min(fullGpuCtx, maxCtx),
                                                          )
                                                        }
                                                        className="flex items-center gap-1.5 text-[12px] font-medium text-emerald-400/80 hover:text-emerald-300 transition-colors"
                                                      >
                                                        <span className="inline-block w-2 h-2 rounded-full bg-emerald-400 shadow-[0_0_4px_rgba(52,211,153,0.4)]" />
                                                        {t("hfBrowser.optimalGpuCtxShort", {
                                                          ctx: fullGpuCtx.toLocaleString(),
                                                        })}
                                                      </button>
                                                    )}
                                                  {ramCtx > 0 && ramCtx !== fullGpuCtx && (
                                                    <button
                                                      type="button"
                                                      onClick={() =>
                                                        setRecContext(Math.min(ramCtx, maxCtx))
                                                      }
                                                      className="flex items-center gap-1.5 text-[12px] font-medium text-amber-400/80 hover:text-amber-300 transition-colors"
                                                    >
                                                      <span className="inline-block w-2 h-2 rounded-full bg-amber-400 shadow-[0_0_4px_rgba(251,191,36,0.4)]" />
                                                      {t("hfBrowser.optimalRamCtxShort", {
                                                        ctx: ramCtx.toLocaleString(),
                                                      })}
                                                    </button>
                                                  )}
                                                </div>
                                              )}
                                              {/* Warning: exceeding GPU-optimal context */}
                                              {warnOnGpuCtxOverflow &&
                                                fullGpuCtx > 0 &&
                                                fullGpuCtx < maxCtx &&
                                                clampedCtx > fullGpuCtx && (
                                                  <div className="flex items-start gap-2 mt-1.5 rounded-lg border border-amber-400/20 bg-amber-400/5 px-2.5 py-2">
                                                    <AlertTriangle
                                                      size={13}
                                                      className="text-amber-400 shrink-0 mt-0.5"
                                                    />
                                                    <p className="text-[11px] leading-snug text-amber-400/80">
                                                      {t("hfBrowser.ctxExceedsGpu", {
                                                        ctx: fullGpuCtx.toLocaleString(),
                                                      })}
                                                    </p>
                                                  </div>
                                                )}
                                              {/* State B: Model exceeds VRAM entirely */}
                                              {showGpuGuidance &&
                                                fullGpuCtx === 0 &&
                                                ramCtx > 0 && (
                                                  <div className="flex items-start gap-2 mt-1.5 rounded-lg border border-blue-400/20 bg-blue-400/5 px-2.5 py-2">
                                                    <Info
                                                      size={13}
                                                      className="text-blue-400 shrink-0 mt-0.5"
                                                    />
                                                    <p className="text-[11px] leading-snug text-blue-300/80">
                                                      {t("hfBrowser.modelExceedsVram")}
                                                    </p>
                                                  </div>
                                                )}
                                            </div>
                                          );
                                        })()}

                                        {/* KV Cache type */}
                                        <div>
                                          <label className="text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                                            {t("hfBrowser.kvCacheType")}
                                          </label>
                                          <select
                                            value={recKvType}
                                            onChange={(e) => {
                                              setRecKvType(e.target.value);
                                              const bpv = KV_BPV[e.target.value] || 2;
                                              const mx = Math.max(
                                                maxContextForBpv(
                                                  selFile.size,
                                                  recData.kvBasePerToken,
                                                  bpv,
                                                  totalAvail,
                                                  recData.modelMaxContext,
                                                  activeSidecarReserveBytes,
                                                ),
                                                1024,
                                              );
                                              setRecContext((prev) => Math.min(prev, mx));
                                            }}
                                            className="mt-1 w-full rounded-md border border-fg/10 bg-fg/5 px-2 py-1.5 text-[11px] text-fg focus:border-fg/25 focus:outline-none"
                                          >
                                            <option value="f32">{t("hfBrowser.kvF32")}</option>
                                            <option value="f16">{t("hfBrowser.kvF16")}</option>
                                            <option value="q8_0">{t("hfBrowser.kvQ80")}</option>
                                            <option value="q5_1">{t("hfBrowser.kvQ51")}</option>
                                            <option value="q5_0">{t("hfBrowser.kvQ50")}</option>
                                            <option value="q4_1">{t("hfBrowser.kvQ41")}</option>
                                            <option value="q4_0">{t("hfBrowser.kvQ40")}</option>
                                            <option value="iq4_nl">{t("hfBrowser.kvIq4nl")}</option>
                                          </select>
                                        </div>

                                        <div data-tour-id="hf-rec-offload">
                                          <label className="text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                                            {t("hfBrowser.detailModelOffload")}
                                          </label>
                                          <div className="mt-1 grid grid-cols-4 gap-1 rounded-md border border-fg/10 bg-fg/5 p-1">
                                            {(["auto", "cpu", "gpu", "mixed"] as const).map(
                                              (value) => {
                                                const active = recModelOffload === value;
                                                const copy = getModelOffloadCopy(t, value);
                                                return (
                                                  <button
                                                    key={value}
                                                    type="button"
                                                    disabled={!gpuOptionsEnabled}
                                                    onClick={() => setRecModelOffload(value)}
                                                    className={cn(
                                                      "rounded-sm px-2 py-1.5 text-[11px] font-medium transition",
                                                      !gpuOptionsEnabled
                                                        ? "cursor-not-allowed text-fg/25"
                                                        : active
                                                          ? "bg-accent/15 text-accent"
                                                          : "text-fg/50 hover:bg-fg/8 hover:text-fg/80",
                                                    )}
                                                    title={copy.description}
                                                  >
                                                    {copy.label}
                                                  </button>
                                                );
                                              },
                                            )}
                                          </div>
                                          <p className="mt-1 text-[11px] leading-snug text-fg/40">
                                            {gpuOptionsEnabled
                                              ? getModelOffloadCopy(t, recModelOffload).description
                                              : t("hfBrowser.gpuOffloadUnavailable")}
                                          </p>
                                        </div>

                                        <div data-tour-id="hf-rec-kv-location">
                                          <label className="text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                                            {t("hfBrowser.kvLocationLabel")}
                                          </label>
                                          <div className="mt-1 grid grid-cols-3 gap-1 rounded-md border border-fg/10 bg-fg/5 p-1">
                                            {(["auto", "ram", "vram"] as const).map((value) => {
                                              const active = recKvPlacement === value;
                                              const copy = getKvPlacementCopy(t, value);
                                              return (
                                                <button
                                                  key={value}
                                                  type="button"
                                                  disabled={!gpuOptionsEnabled}
                                                  onClick={() => setRecKvPlacement(value)}
                                                  className={cn(
                                                    "rounded-sm px-2 py-1.5 text-[11px] font-medium transition",
                                                    !gpuOptionsEnabled
                                                      ? "cursor-not-allowed text-fg/25"
                                                      : active
                                                        ? "bg-accent/15 text-accent"
                                                        : "text-fg/50 hover:bg-fg/8 hover:text-fg/80",
                                                  )}
                                                  title={copy.description}
                                                >
                                                  {copy.label}
                                                </button>
                                              );
                                            })}
                                          </div>
                                          <p className="mt-1 text-[11px] leading-snug text-fg/40">
                                            {gpuOptionsEnabled
                                              ? getKvPlacementCopy(t, recKvPlacement).description
                                              : t("hfBrowser.kvPlacementUnavailable")}
                                          </p>
                                        </div>

                                        {/* Warning */}
                                        {score < 60 && (
                                          <div className="flex items-start gap-2 rounded-lg border border-red-400/20 bg-red-400/10 px-2.5 py-2">
                                            <AlertTriangle
                                              size={12}
                                              className="text-red-400 shrink-0 mt-0.5"
                                            />
                                            <p className="text-[12px] leading-snug text-red-400/90">
                                              {t("hfBrowser.notRecommended")}
                                            </p>
                                          </div>
                                        )}

                                        {/* Download recommended */}
                                        {score >= 60 && (
                                          <button
                                            onClick={() => void queueRecommendedDownload()}
                                            className={cn(
                                              "flex w-full items-center justify-center gap-1.5 rounded-lg border border-emerald-400/30 bg-emerald-400/15 py-1.5 text-[11px] font-semibold text-emerald-500",
                                              interactive.transition.default,
                                              "hover:bg-emerald-400/25 active:scale-[0.97]",
                                            )}
                                          >
                                            <Download size={12} />
                                            {t("hfBrowser.download")} {selFile.quantization}
                                          </button>
                                        )}

                                        <button
                                          onClick={openCompareModal}
                                          className="flex w-full items-center justify-center gap-1 py-1 text-[12px] text-fg/55 hover:text-fg/75 transition-colors"
                                        >
                                          {t("hfBrowser.compare")}
                                        </button>

                                        {/* More details button */}
                                        <button
                                          onClick={() => setDetailSheetOpen(true)}
                                          className="flex w-full items-center justify-center gap-1 py-1 text-[12px] text-fg/40 hover:text-fg/60 transition-colors"
                                        >
                                          <Info size={10} />
                                          {t("hfBrowser.moreDetails")}
                                        </button>
                                      </div>
                                    );
                                  })()
                                )}
                              </div>
                            </div>
                          )}

                          {filesPanelTab === "files" && (
                            <div className="flex-1 overflow-y-auto px-3 py-3 pb-6 space-y-2">
                              {sortedFilesWithSize.map((file) => {
                                const rs = runabilityScores[file.filename];
                                return (
                                  <div
                                    key={file.filename}
                                    className="rounded-xl border border-fg/10 bg-fg/3 px-3 py-2.5"
                                  >
                                    <p
                                      className="truncate text-[12px] font-medium text-fg"
                                      title={file.filename}
                                    >
                                      {file.filename}
                                    </p>
                                    <div className="mt-1.5 flex flex-wrap items-center gap-1.5">
                                      <span className="rounded-md border border-accent/20 bg-accent/10 px-1.5 py-0.5 text-[9px] font-semibold text-accent/80">
                                        {file.quantization}
                                      </span>
                                      {file.isMmproj && (
                                        <span className="rounded-md border border-blue-400/20 bg-blue-400/10 px-1.5 py-0.5 text-[9px] font-semibold text-blue-300">
                                          MMPROJ
                                        </span>
                                      )}
                                      {file.isMtp && (
                                        <span className="rounded-md border border-accent/20 bg-accent/10 px-1.5 py-0.5 text-[9px] font-semibold text-accent/80">
                                          MTP
                                        </span>
                                      )}
                                      {rs && (
                                        <div className="flex flex-col gap-0.5">
                                          <span
                                            className={cn(
                                              "rounded-md border px-1.5 py-0.5 text-[9px] font-semibold w-fit",
                                              rs.label === "excellent"
                                                ? "border-emerald-400/30 bg-emerald-400/15 text-emerald-500"
                                                : rs.label === "good"
                                                  ? "border-blue-400/30 bg-blue-400/15 text-blue-500"
                                                  : rs.label === "marginal"
                                                    ? "border-amber-400/30 bg-amber-400/15 text-amber-500"
                                                    : rs.label === "poor"
                                                      ? "border-orange-400/30 bg-orange-400/15 text-orange-500"
                                                      : "border-red-400/30 bg-red-400/15 text-red-500",
                                            )}
                                            title={t("hfBrowser.runabilityTooltip", {
                                              score: rs.score,
                                              label: rs.label,
                                              mode: getRunabilityModeCopy(t, rs.gpuMode).long,
                                              ram: rs.fitsInRam ? t("hfBrowser.fitsInRam") : "",
                                              vram: rs.fitsInVram ? t("hfBrowser.fitsInVram") : "",
                                            })}
                                          >
                                            {rs.score}
                                          </span>
                                          <span className="max-w-55 text-[10px] leading-tight text-fg/40">
                                            {getRunabilityModeCopy(t, rs.gpuMode).short}
                                          </span>
                                        </div>
                                      )}
                                      <span className="text-[12px] text-fg/45">
                                        {formatBytes(file.size)}
                                      </span>
                                    </div>
                                    <button
                                      onClick={() => void queueFilesDownload(file)}
                                      className={cn(
                                        "mt-2 flex w-full items-center justify-center gap-1.5 rounded-lg border border-accent/30 bg-accent/15 py-1.5 text-[11px] font-semibold text-accent",
                                        interactive.transition.default,
                                        "hover:bg-accent/25 active:scale-[0.97]",
                                      )}
                                    >
                                      {isOllamaMode ? (
                                      <img
                                        src={OllamaIcon}
                                        alt="Ollama"
                                        className="h-3.5 w-3.5 shrink-0"
                                      />
                                    ) : (
                                      <Download size={12} />
                                    )}
                                      {isOllamaMode
                                        ? `${t("hfBrowser.pullToOllama")} ${selectedOllamaProvider?.label ?? ""}`.trim()
                                        : returnTo && !file.isMmproj && !file.isMtp
                                          ? t("hfBrowser.downloadAndUse")
                                          : t("hfBrowser.download")}
                                    </button>
                                  </div>
                                );
                              })}
                            </div>
                          )}
                            </>
                          )}
                        </div>
                      </div>
                    </>
                  )}
                </div>
              )}
            </motion.div>
          )}
        </AnimatePresence>
        )}
      </div>

      {compareOpen && recData && (
        <div
          className="fixed inset-0 z-120 bg-black/70 backdrop-blur-[1px] p-4 flex items-center justify-center"
          onClick={() => setCompareOpen(false)}
        >
          <div
            className="w-full max-w-310 max-h-[92vh] rounded-2xl border border-fg/15 bg-surface/95 shadow-2xl overflow-hidden flex flex-col"
            onClick={(e) => e.stopPropagation()}
          >
            <div className="flex items-center justify-between border-b border-fg/10 px-4 py-3">
              <div>
                <h3 className="text-sm font-semibold text-fg">{t("hfBrowser.compareTitle")}</h3>
                <p className="text-[11px] text-fg/50">{t("hfBrowser.compareSubtitle")}</p>
              </div>
              <button
                type="button"
                onClick={() => setCompareOpen(false)}
                className="rounded-md border border-fg/15 bg-fg/5 p-1.5 text-fg/60 hover:text-fg hover:border-fg/25"
                aria-label={t("hfBrowser.compareClose")}
              >
                <X size={14} />
              </button>
            </div>

            <div className="border-b border-fg/10 px-4 py-3">
              <div className="overflow-x-auto pb-1">
                <div
                  className={cn(
                    "grid gap-2 min-w-full",
                    compareSelections.length === 1
                      ? "grid-cols-1"
                      : compareSelections.length === 2
                        ? "grid-cols-2"
                        : "grid-cols-3",
                  )}
                >
                  {compareSelections.map((selection, index) => (
                    <div
                      key={selection.id}
                      className="rounded-xl border border-fg/10 bg-fg/3 p-2.5 space-y-2"
                    >
                      <div className="flex items-center justify-between">
                        <p className="text-[11px] font-semibold text-fg/70">
                          {t("hfBrowser.compareConfig", { index: index + 1 })}
                        </p>
                        {compareSelections.length > 1 && (
                          <button
                            type="button"
                            onClick={() => removeCompareSelection(selection.id)}
                            className="text-[10px] text-fg/40 hover:text-red-300 transition-colors"
                          >
                            {t("hfBrowser.compareRemove")}
                          </button>
                        )}
                      </div>

                      <div>
                        <label className="text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                          {t("hfBrowser.quantization")}
                        </label>
                        <select
                          value={selection.filename}
                          onChange={(e) =>
                            updateCompareSelection(selection.id, { filename: e.target.value })
                          }
                          className="mt-1 w-full rounded-md border border-fg/10 bg-fg/5 px-2 py-1.5 text-[11px] text-fg focus:border-fg/25 focus:outline-none"
                        >
                          {recData.files.map((f) => (
                            <option key={f.filename} value={f.filename}>
                              {f.quantization} — {formatBytes(f.size)}
                            </option>
                          ))}
                        </select>
                      </div>

                      <div>
                        <label className="text-[9px] font-semibold uppercase tracking-wider text-fg/40">
                          {t("hfBrowser.kvCacheType")}
                        </label>
                        <select
                          value={selection.kvType}
                          onChange={(e) =>
                            updateCompareSelection(selection.id, { kvType: e.target.value })
                          }
                          className="mt-1 w-full rounded-md border border-fg/10 bg-fg/5 px-2 py-1.5 text-[11px] text-fg focus:border-fg/25 focus:outline-none"
                        >
                          <option value="f32">F32 (maximum quality)</option>
                          <option value="f16">F16 (high quality)</option>
                          <option value="q8_0">Q8_0 (balanced)</option>
                          <option value="q5_1">Q5_1 (good savings)</option>
                          <option value="q5_0">Q5_0 (good savings)</option>
                          <option value="q4_1">Q4_1 (memory saver)</option>
                          <option value="q4_0">Q4_0 (memory saver)</option>
                          <option value="iq4_nl">IQ4_NL (aggressive)</option>
                        </select>
                      </div>
                    </div>
                  ))}
                </div>
              </div>

              {compareSelections.length < 3 && (
                <button
                  type="button"
                  onClick={addCompareSelection}
                  className="mt-2 text-[11px] font-medium text-accent/80 hover:text-accent transition-colors"
                >
                  {t("hfBrowser.compareAdd")}
                </button>
              )}
            </div>

            <div className="flex-1 overflow-x-auto px-4 py-3">
              <div
                className={cn(
                  "grid gap-3 min-w-160",
                  compareSelections.length === 1
                    ? "grid-cols-1"
                    : compareSelections.length === 2
                      ? "grid-cols-2"
                      : "grid-cols-3",
                )}
              >
                {compareSelections.map((selection) => {
                  const selectedFile =
                    recData.files.find((f) => f.filename === selection.filename) ||
                    recData.files[0];
                  if (!selectedFile) return null;

                  return (
                    <div
                      key={selection.id}
                      className="rounded-xl border border-fg/10 bg-fg/3 overflow-hidden flex flex-col min-h-0"
                    >
                      <div className="border-b border-fg/10 px-3 py-2">
                        <p
                          className="truncate text-[12px] font-semibold text-fg"
                          title={selectedFile.filename}
                        >
                          {selectedFile.quantization} · {formatBytes(selectedFile.size)}
                        </p>
                        <p className="text-[10px] text-fg/45">
                          {t("hfBrowser.compareKvLabel", { kv: selection.kvType.toUpperCase() })}
                        </p>
                      </div>

                      <div
                        ref={(el) => {
                          compareScrollRefs.current[selection.id] = el;
                        }}
                        onScroll={(e) => handleCompareReportScroll(selection.id, e)}
                        className="overflow-y-auto max-h-[56vh] px-3 py-2"
                      >
                        <DetailReportContent
                          recData={recData}
                          selectedFile={selectedFile}
                          kvType={selection.kvType}
                          modelOffload={recModelOffload}
                          kvPlacement={recKvPlacement}
                          contextLength={recContext}
                          sidecarReserveBytes={activeSidecarReserveBytes}
                          t={t}
                        />
                      </div>
                    </div>
                  );
                })}
              </div>
            </div>
          </div>
        </div>
      )}

      <BottomMenu
        isOpen={detailSheetOpen}
        onClose={() => setDetailSheetOpen(false)}
        title={t("hfBrowser.detailedReport")}
        includeExitIcon
      >
        {recData &&
          (() => {
            const selFile = recData.files.find((f) => f.filename === recFile) || recData.files[0];
            if (!selFile) return null;
            return (
              <DetailReportContent
                recData={recData}
                selectedFile={selFile}
                kvType={recKvType}
                modelOffload={recModelOffload}
                kvPlacement={recKvPlacement}
                contextLength={recContext}
                sidecarReserveBytes={activeSidecarReserveBytes}
                t={t}
              />
            );
          })()}
      </BottomMenu>

      <BottomMenu
        isOpen={showFilterMenu}
        onClose={() => setShowFilterMenu(false)}
        title={t("hfBrowser.filterTitle")}
      >
        <div>
          <p className="mb-4 text-[12.5px] leading-relaxed text-fg/55">
            {t("hfBrowser.filterSubtitle")}
          </p>

          {!isBundleMode && !isOllamaMode && (
            <>
              <div className="mb-2 flex items-center gap-2">
                <span className="text-[10px] font-semibold uppercase tracking-[0.14em] text-fg/40">
                  {t("hfBrowser.filterUnfiltered")}
                </span>
                <div className="h-px flex-1 bg-fg/8" />
                <Switch
                  checked={unfilteredSearch}
                  onChange={setUnfilteredSearch}
                  aria-label={t("hfBrowser.filterUnfiltered")}
                />
              </div>
              <p className="mb-5 text-[11px] leading-relaxed text-fg/45">
                {t("hfBrowser.filterUnfilteredHint")}
              </p>
            </>
          )}

          {/* Author */}
          <div className="mb-2 flex items-center gap-2">
            <span className="text-[10px] font-semibold uppercase tracking-[0.14em] text-fg/40">
              Author
            </span>
            <div className="h-px flex-1 bg-fg/8" />
            {filterAuthor.trim() && (
              <button
                onClick={() => setFilterAuthor("")}
                className="text-[10.5px] text-fg/45 hover:text-fg/75"
              >
                {t("hfBrowser.filterClear")}
              </button>
            )}
          </div>
          <input
            type="text"
            value={filterAuthor}
            onChange={(e) => setFilterAuthor(e.target.value)}
            placeholder="e.g. bartowski"
            className="mb-5 h-9 w-full rounded-lg border border-fg/10 bg-fg/4 px-2.5 text-[13px] text-fg outline-none transition placeholder:text-fg/35 focus:border-accent/40 focus:ring-1 focus:ring-accent/20"
          />

          {/* Pipeline tags */}
          <div className="mb-2 flex items-center gap-2">
            <span className="text-[10px] font-semibold uppercase tracking-[0.14em] text-fg/40">
              {t("hfBrowser.filterPipeline")}
            </span>
            <div className="h-px flex-1 bg-fg/8" />
            {filterPipelineTags.size > 0 && (
              <button
                onClick={() => setFilterPipelineTags(new Set())}
                className="text-[10.5px] text-fg/45 hover:text-fg/75"
              >
                {t("hfBrowser.filterClear")}
              </button>
            )}
          </div>
          <div className="mb-5 flex flex-wrap gap-1.5">
            {[...new Set([...availablePipelineTags, ...COMMON_PIPELINE_TAGS])].map((tag) => {
              const active = filterPipelineTags.has(tag);
              const inResults = availablePipelineTags.has(tag);
              return (
                <button
                  key={tag}
                  onClick={() => {
                    setFilterPipelineTags((prev) => {
                      const next = new Set(prev);
                      if (next.has(tag)) next.delete(tag);
                      else next.add(tag);
                      return next;
                    });
                  }}
                  className={cn(
                    "rounded-full border px-2.5 py-1 text-[11px] font-medium transition",
                    active
                      ? "border-accent/40 bg-accent/15 text-accent"
                      : inResults
                        ? "border-fg/12 bg-fg/4 text-fg/70 hover:border-fg/25"
                        : "border-fg/8 bg-transparent text-fg/40 hover:border-fg/20 hover:text-fg/65",
                  )}
                >
                  {tag}
                </button>
              );
            })}
          </div>

          {/* Param size */}
          <div className="mb-2 flex items-center gap-2">
            <span className="text-[10px] font-semibold uppercase tracking-[0.14em] text-fg/40">
              {t("hfBrowser.filterParamSize")}
            </span>
            <div className="h-px flex-1 bg-fg/8" />
            {(filterParamMin || filterParamMax) && (
              <button
                onClick={() => {
                  setFilterParamMin("");
                  setFilterParamMax("");
                }}
                className="text-[10.5px] text-fg/45 hover:text-fg/75"
              >
                {t("hfBrowser.filterClear")}
              </button>
            )}
          </div>
          <div className="mb-2 grid grid-cols-2 gap-2">
            <label className="flex flex-col gap-1">
              <span className="text-[10.5px] text-fg/55">{t("hfBrowser.filterMin")}</span>
              <input
                type="number"
                inputMode="decimal"
                min={0}
                step={0.1}
                value={filterParamMin}
                onChange={(e) => setFilterParamMin(e.target.value)}
                placeholder={t("hfBrowser.filterMinPlaceholder")}
                className="h-9 rounded-lg border border-fg/10 bg-fg/4 px-2.5 text-[13px] text-fg outline-none transition placeholder:text-fg/35 focus:border-accent/40 focus:ring-1 focus:ring-accent/20"
              />
            </label>
            <label className="flex flex-col gap-1">
              <span className="text-[10.5px] text-fg/55">{t("hfBrowser.filterMax")}</span>
              <input
                type="number"
                inputMode="decimal"
                min={0}
                step={0.1}
                value={filterParamMax}
                onChange={(e) => setFilterParamMax(e.target.value)}
                placeholder={t("hfBrowser.filterMaxPlaceholder")}
                className="h-9 rounded-lg border border-fg/10 bg-fg/4 px-2.5 text-[13px] text-fg outline-none transition placeholder:text-fg/35 focus:border-accent/40 focus:ring-1 focus:ring-accent/20"
              />
            </label>
          </div>
          <p className="mb-5 text-[10.5px] text-fg/40">{t("hfBrowser.filterParamNote")}</p>

          {/* Quick presets */}
          <div className="mb-2 flex items-center gap-2">
            <span className="text-[10px] font-semibold uppercase tracking-[0.14em] text-fg/40">
              {t("hfBrowser.filterQuickPresets")}
            </span>
            <div className="h-px flex-1 bg-fg/8" />
          </div>
          <div className="flex flex-wrap gap-1.5">
            {[
              { label: "≤ 3B", min: "", max: "3" },
              { label: "3–8B", min: "3", max: "8" },
              { label: "8–14B", min: "8", max: "14" },
              { label: "14–34B", min: "14", max: "34" },
              { label: "≥ 34B", min: "34", max: "" },
            ].map((p) => {
              const active = filterParamMin === p.min && filterParamMax === p.max;
              return (
                <button
                  key={p.label}
                  onClick={() => {
                    setFilterParamMin(p.min);
                    setFilterParamMax(p.max);
                  }}
                  className={cn(
                    "rounded-full border px-2.5 py-1 text-[11px] font-medium transition",
                    active
                      ? "border-accent/40 bg-accent/15 text-accent"
                      : "border-fg/12 bg-fg/4 text-fg/70 hover:border-fg/25",
                  )}
                >
                  {p.label}
                </button>
              );
            })}
          </div>

          {activeFilterCount > 0 && (
            <button
              onClick={() => {
                setFilterPipelineTags(new Set());
                setFilterParamMin("");
                setFilterParamMax("");
                setFilterAuthor("");
              }}
              className="mt-5 w-full rounded-lg border border-fg/10 bg-fg/4 py-2 text-[12px] font-medium text-fg/75 transition hover:border-fg/20 hover:text-fg"
            >
              {t("hfBrowser.filterResetAll")}
            </button>
          )}
        </div>
      </BottomMenu>

      <BottomMenu
        isOpen={showOllamaProviderMenu}
        onClose={() => {
          if (isMobilePlatform && !selectedOllamaProviderId) return;
          setShowOllamaProviderMenu(false);
        }}
        title={t("hfBrowser.destinationPickerTitle")}
      >
        <p className="mb-5 text-[12.5px] leading-relaxed text-fg/55">
          {t("hfBrowser.destinationPickerSubtitle")}
        </p>

        <div className="space-y-5">
          {!isMobilePlatform && (
            <div>
              <p className="mb-2 px-1 text-[10px] font-semibold uppercase tracking-[0.14em] text-fg/35">
                {t("hfBrowser.destinationLocalSection")}
              </p>
              <div className="overflow-hidden rounded-xl border border-fg/10 bg-fg/3">
                <button
                  onClick={() => {
                    setSelectedOllamaProviderId(null);
                    setShowOllamaProviderMenu(false);
                  }}
                  className={cn(
                    "flex w-full items-center gap-3 px-4 py-3 text-left transition",
                    !isOllamaMode ? "bg-accent/8" : "hover:bg-fg/5 active:bg-fg/8",
                  )}
                >
                  <HardDrive
                    size={16}
                    className={cn("shrink-0", !isOllamaMode ? "text-accent" : "text-fg/45")}
                  />
                  <div className="min-w-0 flex-1">
                    <div className="truncate text-[13px] font-medium text-fg">
                      {t("hfBrowser.destinationLocal")}
                    </div>
                    <div className="truncate text-[11.5px] text-fg/45">
                      {t("hfBrowser.destinationLocalHint")}
                    </div>
                  </div>
                  {!isOllamaMode && (
                    <Check size={15} strokeWidth={2.5} className="shrink-0 text-accent" />
                  )}
                </button>
              </div>
            </div>
          )}

          <div>
            <div className="mb-2 flex items-center justify-between px-1">
              <p className="text-[10px] font-semibold uppercase tracking-[0.14em] text-fg/35">
                {t("hfBrowser.destinationOllamaSection")}
              </p>
              {ollamaProviders.length > 0 && (
                <span className="text-[10px] font-medium tabular-nums text-fg/40">
                  {ollamaProviders.length}
                </span>
              )}
            </div>
            {ollamaProviders.length === 0 ? (
              <div className="rounded-xl border border-dashed border-fg/10 bg-fg/2 px-4 py-5 text-center">
                <img src={OllamaIcon} alt="Ollama" className="mx-auto mb-2 h-5 w-5 opacity-40" />
                <p className="text-[12px] leading-snug text-fg/55">
                  {t("hfBrowser.destinationNoOllama")}
                </p>
              </div>
            ) : (
              <div className="divide-y divide-fg/8 overflow-hidden rounded-xl border border-fg/10 bg-fg/3">
                {ollamaProviders.map((provider) => {
                  const isSelected = provider.id === selectedOllamaProviderId;
                  return (
                    <button
                      key={provider.id}
                      onClick={() => {
                        setSelectedOllamaProviderId(provider.id);
                        setShowOllamaProviderMenu(false);
                      }}
                      className={cn(
                        "flex w-full items-center gap-3 px-4 py-3 text-left transition",
                        isSelected ? "bg-accent/8" : "hover:bg-fg/5 active:bg-fg/8",
                      )}
                    >
                      <img
                        src={OllamaIcon}
                        alt="Ollama"
                        className={cn("h-4 w-4 shrink-0", !isSelected && "opacity-60")}
                      />
                      <div className="min-w-0 flex-1">
                        <div className="truncate text-[13px] font-medium text-fg">
                          {provider.label}
                        </div>
                        {provider.baseUrl && (
                          <div className="truncate font-mono text-[11px] text-fg/45">
                            {provider.baseUrl}
                          </div>
                        )}
                      </div>
                      {isSelected && (
                        <Check size={15} strokeWidth={2.5} className="shrink-0 text-accent" />
                      )}
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>
      </BottomMenu>

      {returnTo && (
        <div className="fixed bottom-6 left-1/2 -translate-x-1/2 z-50">
          <button
            onClick={() => hfNavigate(returnTo)}
            disabled={!hasLocalModel}
            className={cn(
              "flex items-center gap-2 rounded-full px-6 py-3 text-sm font-bold transition active:scale-[0.98]",
              hasLocalModel
                ? "border border-emerald-500/40 bg-emerald-500 text-black shadow-[0_4px_20px_rgba(16,185,129,0.35)] hover:bg-emerald-400 hover:shadow-[0_4px_24px_rgba(16,185,129,0.5)]"
                : "border border-white/10 bg-white/10 text-white/40 cursor-not-allowed",
            )}
          >
            {hasLocalModel ? t("hfBrowser.continueSetup") : t("hfBrowser.downloadToContinue")}
            <ArrowRight size={16} />
          </button>
        </div>
      )}

      <HfTokenMenu
        isOpen={gatedMenuOpen}
        onClose={() => setGatedMenuOpen(false)}
        gated
        modelId={view.kind === "model" ? view.modelId : null}
        onSaved={() => {
          if (view.kind === "model") void openModel(view.modelId);
        }}
      />

      {showHfTour && recData != null && !recLoading && (
        <GuidedTour tour="hfBrowser" onDismiss={dismissHfTour} />
      )}
    </div>
  );
}

function AuthorView({
  author,
  onBack,
  onOpenModel,
}: {
  author: string;
  onBack: () => void;
  onOpenModel: (modelId: string) => void;
}) {
  const [overview, setOverview] = useState<HfAuthorOverview | null>(null);
  const [models, setModels] = useState<HfSearchResult[] | null>(null);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState<string | null>(null);
  const [search, setSearch] = useState("");
  const [debouncedSearch, setDebouncedSearch] = useState("");

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search.trim()), 300);
    return () => clearTimeout(timer);
  }, [search]);

  useEffect(() => {
    let cancelled = false;
    setOverview(null);
    invoke<HfAuthorOverview>("hf_get_author_overview", { author })
      .then((ov) => {
        if (!cancelled) setOverview(ov);
      })
      .catch(() => {});
    return () => {
      cancelled = true;
    };
  }, [author]);

  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError(null);
    invoke<HfSearchResult[]>("hf_get_author_models", {
      author,
      search: debouncedSearch || null,
      limit: 60,
    })
      .then((md) => {
        if (!cancelled) setModels(md);
      })
      .catch((err) => {
        if (!cancelled) {
          setError(err instanceof Error ? err.message : String(err));
          setModels([]);
        }
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [author, debouncedSearch]);

  const name = overview?.name ?? author;
  const avatarUrl = overview?.avatarUrl ?? null;

  return (
    <div className="flex flex-col gap-3">
      <button
        type="button"
        onClick={onBack}
        className="flex w-fit items-center gap-1 text-[12px] font-medium text-fg/55 transition hover:text-fg"
      >
        <ChevronLeft size={14} />
        Back
      </button>

      <div className="rounded-xl border border-fg/10 bg-fg/[0.025] p-4">
        <div className="flex items-start gap-3">
          {avatarUrl ? (
            <img
              src={avatarUrl}
              alt={name}
              className="h-12 w-12 shrink-0 rounded-xl object-cover"
              loading="lazy"
            />
          ) : (
            <div
              className={cn(
                "flex h-12 w-12 shrink-0 items-center justify-center rounded-xl text-[20px] font-bold text-fg/75",
                authorColor(name),
              )}
            >
              {name.charAt(0).toUpperCase()}
            </div>
          )}

          <div className="min-w-0 flex-1">
            <div className="flex flex-wrap items-center gap-x-2 gap-y-1">
              <span className="truncate text-[15px] font-semibold text-fg">
                {overview?.fullname || name}
              </span>
              <span className="text-[11px] text-fg/40">@{name}</span>
              {overview?.isPro && (
                <span className="rounded border border-amber-400/30 bg-amber-400/10 px-1 py-0.5 text-[9px] font-bold uppercase tracking-wide text-amber-300">
                  Pro
                </span>
              )}
              {overview?.kind && (
                <span className="rounded border border-fg/10 bg-fg/5 px-1.5 py-0.5 text-[9px] font-medium uppercase tracking-wide text-fg/45">
                  {overview.kind}
                </span>
              )}
            </div>

            {overview?.details && (
              <p className="mt-1 line-clamp-2 text-xs text-fg/55">{overview.details}</p>
            )}

            {overview && (
              <div className="mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-[11px] tabular-nums text-fg/50">
                <span className="flex items-center gap-1">
                  <Layers size={11} className="text-fg/35" />
                  {formatNumber(overview.numModels)} models
                </span>
                <span className="flex items-center gap-1">
                  <Heart size={11} className="text-fg/35" />
                  {formatNumber(overview.numLikes)} likes
                </span>
                <span className="flex items-center gap-1">
                  <Users size={11} className="text-fg/35" />
                  {formatNumber(overview.numFollowers)} followers
                </span>
                {overview.createdAt && (
                  <span className="text-fg/35">
                    since {new Date(overview.createdAt).getFullYear()}
                  </span>
                )}
              </div>
            )}
          </div>

          <a
            href={`https://huggingface.co/${name}`}
            target="_blank"
            rel="noreferrer"
            className="flex shrink-0 items-center gap-1 text-[11px] text-accent/70 transition hover:text-accent"
          >
            <ExternalLink size={12} />
            HuggingFace
          </a>
        </div>
      </div>

      <div className="relative">
        <Search
          size={14}
          className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-fg/40"
        />
        <input
          type="text"
          value={search}
          onChange={(e) => setSearch(e.target.value)}
          placeholder="Search this author's models…"
          className="h-9 w-full rounded-lg border border-fg/10 bg-fg/4 pl-9 pr-8 text-[13px] text-fg placeholder-fg/35 outline-none transition focus:border-accent/40 focus:bg-fg/6 focus:ring-1 focus:ring-accent/20"
        />
        {search && (
          <button
            onClick={() => setSearch("")}
            className="absolute right-2 top-1/2 flex h-6 w-6 -translate-y-1/2 items-center justify-center rounded-md text-fg/40 transition hover:bg-fg/8 hover:text-fg/80"
            aria-label="Clear"
          >
            <X size={12} />
          </button>
        )}
      </div>

      <div className="flex items-center justify-between px-0.5">
        <span className="text-[10px] font-semibold uppercase tracking-[0.2em] text-fg/35">
          GGUF models
        </span>
        {models && <span className="text-[10px] tabular-nums text-fg/30">{models.length}</span>}
      </div>

      {loading ? (
        <div className="flex items-center justify-center gap-2 py-16 text-fg/50">
          <Loader size={16} className="animate-spin" />
          <span className="text-sm">Loading…</span>
        </div>
      ) : error ? (
        <div className="rounded-xl border border-danger/20 bg-danger/5 px-4 py-3">
          <p className="text-xs leading-relaxed text-danger/80">{error}</p>
        </div>
      ) : models && models.length > 0 ? (
        <div className="grid grid-cols-1 gap-1.5 md:grid-cols-2 xl:grid-cols-3">
          {models.map((model) => {
            const paramSize = extractParamSize(model.tags, model.modelId);
            return (
              <button
                key={model.modelId}
                onClick={() => onOpenModel(model.modelId)}
                className={cn(
                  "group flex items-center gap-2.5 rounded-lg border border-fg/8 bg-fg/[0.02] px-2.5 py-2 text-left transition",
                  "hover:border-fg/20 hover:bg-fg/[0.05] active:scale-[0.99]",
                )}
              >
                {avatarUrl ? (
                  <img
                    src={avatarUrl}
                    alt={model.author}
                    className="h-6 w-6 shrink-0 rounded-full object-cover"
                    loading="lazy"
                  />
                ) : (
                  <div
                    className={cn(
                      "flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[12px] font-bold text-fg/75",
                      authorColor(model.author),
                    )}
                  >
                    {model.author.charAt(0).toUpperCase()}
                  </div>
                )}
                <div className="min-w-0 flex-1">
                  <div className="truncate text-[12.5px] font-medium text-fg">
                    {model.modelId.split("/").pop()}
                  </div>
                  <div className="mt-0.5 flex items-center gap-1 truncate text-[10.5px] text-fg/45">
                    {model.pipelineTag && <span className="truncate">{model.pipelineTag}</span>}
                    {paramSize && (
                      <>
                        <span className="text-fg/20">·</span>
                        <span className="shrink-0">{paramSize}</span>
                      </>
                    )}
                    {model.lastModified && (
                      <>
                        <span className="text-fg/20">·</span>
                        <span className="shrink-0 truncate">
                          {formatTimeAgo(model.lastModified)}
                        </span>
                      </>
                    )}
                  </div>
                </div>
                <div className="flex shrink-0 items-center gap-2.5 text-[10.5px] tabular-nums text-fg/50">
                  <span className="flex items-center gap-1">
                    <ArrowDownToLine size={10} className="text-fg/35" />
                    {formatNumber(model.downloads)}
                  </span>
                  <span className="flex items-center gap-1">
                    <Heart size={10} className="text-fg/35" />
                    {formatNumber(model.likes)}
                  </span>
                </div>
              </button>
            );
          })}
        </div>
      ) : (
        <div className="flex flex-col items-center gap-2 py-16 text-center">
          <Server size={20} className="text-fg/25" />
          <p className="text-sm text-fg/55">
            {debouncedSearch ? "No models match your search." : "No GGUF models from this author."}
          </p>
        </div>
      )}
    </div>
  );
}
