export type BundleRole = "diffusion_model" | "text_encoder" | "vae" | "vision_encoder";

export interface BundleRecipe {
  id: string;
  displayName: string;
  family: string;
  description: string;
  minimumRuntimeBuild: number | null;
  requiredRoles: BundleRole[];
  diffusionMarkers: string[];
  encoderMarkers: string[];
  encoderParameterBillions: number;
  recommendedRepositories: Partial<Record<BundleRole, string>>;
  supportsTextToImage: boolean;
  supportsImageEdit: boolean;
  maxReferenceImages: number | null;
  requiresReferenceImage: boolean;
  recommendedForScenes: boolean;
  defaultWidth: number;
  defaultHeight: number;
  defaultSteps: number;
  defaultCfg: number;
}

export interface ValidatedAsset {
  selectionId: string;
  profileId: string;
  role: BundleRole;
  modelId: string;
  revision: string;
  relativePath: string;
  format: string;
  quantization: string | null;
  size: number;
  sha256: string;
  architecture: string | null;
  gated: boolean;
}

export interface RuntimeInventory {
  active: { release: string; asset: string } | null;
}

export interface Estimate {
  status: "estimatedRunnable" | "cpuFallback" | "inconclusive" | "notInstalled" | "incompatibleRuntime";
  reason: string;
  estimate?: {
    modelBytes: number;
    availableRamBytes: number | null;
    planMode: string;
    deviceSource: string;
    devices: Array<{
      id: number;
      name: string;
      description: string;
      totalBytes: number;
      freeBytes: number;
      budgetBytes: number;
    }>;
    placements: Array<{
      component: string;
      paramsBytes: number;
      computeReserveBytes: number;
      targets: string[];
      cpu: boolean;
      split: boolean;
    }>;
  } | null;
}

export interface AuthStatus {
  saved: boolean;
  valid: boolean;
  username: string | null;
  errorKind: string | null;
}

export interface QueueResult {
  bundleId: string;
  queueIds: string[];
}

export interface BundleStatus {
  registrationState: "downloading" | "registered" | "setupFailed";
  modelId: string | null;
  setupError: string | null;
}

export interface BundleDraft {
  profileId: string;
  selections: Partial<Record<BundleRole, ValidatedAsset>>;
  repositories: Partial<Record<BundleRole, string>>;
  displayName: string;
  bundleId?: string;
}

export function formatBundleBytes(value: number): string {
  if (!value) return "0 B";
  const units = ["B", "KB", "MB", "GB", "TB"];
  const index = Math.min(Math.floor(Math.log(value) / Math.log(1024)), units.length - 1);
  return `${(value / 1024 ** index).toFixed(index >= 3 ? 1 : 0)} ${units[index]}`;
}
