import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";

import { useDownloadQueue } from "../../../../../core/downloads/DownloadQueueContext";
import { useI18n } from "../../../../../core/i18n/context";
import { toast } from "../../../../components/toast";
import type {
  BundleDraft,
  BundleRecipe,
  BundleRole,
  BundleStatus,
  Estimate,
  QueueResult,
  RuntimeInventory,
  ValidatedAsset,
} from "./types";

function initialDraft(): BundleDraft {
  return { profileId: "", selections: {}, repositories: {}, displayName: "" };
}

export function useImageBundle({
  enabled,
  onOpenModel,
  onDownloadStarted,
}: {
  enabled: boolean;
  onOpenModel: (modelId: string) => void;
  onDownloadStarted?: () => void;
}) {
  const { t } = useI18n();
  const { queue, cancelItem } = useDownloadQueue();
  const [draft, setDraft] = useState<BundleDraft>(initialDraft);
  const [profiles, setProfiles] = useState<BundleRecipe[]>([]);
  const [runtime, setRuntime] = useState<RuntimeInventory["active"]>(null);
  const [roleFiles, setRoleFiles] = useState<ValidatedAsset[]>([]);
  const [roleFilesLoading, setRoleFilesLoading] = useState(false);
  const [roleFilesError, setRoleFilesError] = useState<string | null>(null);
  const [estimate, setEstimate] = useState<Estimate | null>(null);
  const [estimateBusy, setEstimateBusy] = useState(false);
  const [bundleStatus, setBundleStatus] = useState<BundleStatus | null>(null);
  const [busy, setBusy] = useState(false);
  const loadedRef = useRef(false);
  const notifiedBundleRef = useRef<string | null>(null);

  useEffect(() => {
    if (!enabled || loadedRef.current) return;
    loadedRef.current = true;
    void Promise.all([
      invoke<BundleRecipe[]>("hf_image_bundle_profiles"),
      invoke<RuntimeInventory>("sdcpp_runtime_inventory"),
    ]).then(([recipes, inventory]) => {
      setProfiles(recipes);
      setRuntime(inventory.active);
    });
  }, [enabled]);

  const profile = useMemo(
    () => profiles.find((item) => item.id === draft.profileId) ?? null,
    [profiles, draft.profileId],
  );
  const requiredRoles = useMemo(() => profile?.requiredRoles ?? [], [profile]);
  const currentRole = requiredRoles.find((role) => !draft.selections[role]) ?? null;
  const ready = !!profile && requiredRoles.length > 0 && !currentRole;
  const bundleQueue = useMemo(
    () => (draft.bundleId ? queue.filter((item) => item.installId === draft.bundleId) : []),
    [queue, draft.bundleId],
  );
  const downloadsSettled =
    bundleQueue.length > 0 &&
    bundleQueue.every((item) => !["queued", "downloading"].includes(item.status));
  const downloadFailed = bundleQueue.some(
    (item) => item.status === "error" || item.status === "cancelled",
  );
  const downloadsInterrupted =
    !!draft.bundleId &&
    bundleQueue.length === 0 &&
    (!bundleStatus || bundleStatus.registrationState === "downloading");

  const roleLabel = useCallback(
    (role: BundleRole) => {
      if (role === "diffusion_model") return t("hfBrowser.bundle.roles.diffusion");
      if (role === "text_encoder") return t("hfBrowser.bundle.roles.textEncoder");
      if (role === "vision_encoder") return t("hfBrowser.bundle.roles.visionEncoder");
      return t("hfBrowser.bundle.roles.vae");
    },
    [t],
  );

  useEffect(() => {
    if (!enabled || !draft.bundleId) return;
    if (bundleQueue.length > 0 && !downloadsSettled) return;
    let cancelled = false;
    let poll: number | null = null;
    const fetchStatus = () => {
      void invoke<BundleStatus>("hf_image_bundle_status", { bundleId: draft.bundleId })
        .then((status) => {
          if (cancelled) return;
          setBundleStatus(status);
          if (
            status.registrationState === "registered" &&
            status.modelId &&
            notifiedBundleRef.current !== draft.bundleId
          ) {
            notifiedBundleRef.current = draft.bundleId ?? null;
            toast.success(
              t("hfBrowser.bundle.completeTitle"),
              t("hfBrowser.bundle.completeBody", { name: draft.displayName }),
              {
                actionLabel: t("hfBrowser.bundle.openModel"),
                onAction: () => onOpenModel(status.modelId!),
                actionTone: "accent",
              },
            );
          }
          if (status.registrationState !== "downloading" && poll != null) {
            window.clearInterval(poll);
            poll = null;
          }
        })
        .catch(() => {});
    };
    fetchStatus();
    poll = window.setInterval(fetchStatus, 800);
    return () => {
      cancelled = true;
      if (poll != null) window.clearInterval(poll);
    };
  }, [
    enabled,
    draft.bundleId,
    draft.displayName,
    bundleQueue.length,
    downloadsSettled,
    onOpenModel,
    t,
  ]);

  const estimateSizes = useMemo(() => {
    const size = (role: BundleRole) => draft.selections[role]?.size ?? 0;
    return {
      diffusionBytes: size("diffusion_model"),
      textEncoderBytes: size("text_encoder"),
      vaeBytes: size("vae"),
      visionEncoderBytes: size("vision_encoder"),
    };
  }, [draft.selections]);

  useEffect(() => {
    if (!enabled || !ready || !profile || !runtime) {
      setEstimate(null);
      return;
    }
    setEstimateBusy(true);
    void invoke<Estimate>("sdcpp_estimate_remote_bundle", {
      request: {
        profileId: profile.id,
        runtimeRelease: runtime.release,
        runtimeAsset: runtime.asset,
        ...estimateSizes,
      },
    })
      .then(setEstimate)
      .catch((error) => setEstimate({ status: "inconclusive", reason: String(error) }))
      .finally(() => setEstimateBusy(false));
  }, [enabled, ready, profile, runtime, estimateSizes]);

  const chooseProfile = useCallback(
    (next: BundleRecipe) => {
      if (
        draft.profileId &&
        draft.profileId !== next.id &&
        Object.keys(draft.selections).length > 0 &&
        !window.confirm(t("hfBrowser.bundle.changeProfileConfirm"))
      ) {
        return;
      }
      setDraft({
        profileId: next.id,
        selections: {},
        repositories: {},
        displayName: next.displayName,
      });
      setEstimate(null);
      setBundleStatus(null);
    },
    [draft.profileId, draft.selections, t],
  );

  const resetProfile = useCallback(() => {
    if (
      Object.keys(draft.selections).length > 0 &&
      !window.confirm(t("hfBrowser.bundle.changeProfileConfirm"))
    ) {
      return;
    }
    setDraft({ profileId: "", selections: {}, repositories: {}, displayName: "" });
    setEstimate(null);
    setBundleStatus(null);
  }, [draft.selections, t]);

  const clearRole = useCallback((role: BundleRole) => {
    setDraft((current) => {
      const selections = { ...current.selections };
      delete selections[role];
      return { ...current, selections, bundleId: undefined };
    });
    setBundleStatus(null);
  }, []);

  const selectAsset = useCallback((asset: ValidatedAsset) => {
    setDraft((current) => ({
      ...current,
      selections: { ...current.selections, [asset.role]: asset },
      repositories: { ...current.repositories, [asset.role]: asset.modelId },
    }));
    setRoleFiles([]);
    setRoleFilesError(null);
  }, []);

  const setDisplayName = useCallback((displayName: string) => {
    setDraft((current) => ({ ...current, displayName }));
  }, []);

  const loadRoleFiles = useCallback(
    async (modelId: string) => {
      if (!profile || !currentRole) return;
      setRoleFilesLoading(true);
      setRoleFiles([]);
      setRoleFilesError(null);
      try {
        const result = await invoke<ValidatedAsset[]>("hf_image_bundle_files", {
          profileId: profile.id,
          modelId,
          role: currentRole,
        });
        setRoleFiles(result);
      } catch (error) {
        setRoleFilesError(String(error));
      } finally {
        setRoleFilesLoading(false);
      }
    },
    [profile, currentRole],
  );

  const queueBundle = useCallback(
    async (selectionIds: string[]) => {
      if (!profile || !runtime) throw new Error("missing runtime");
      return invoke<QueueResult>("hf_queue_image_bundle", {
        request: {
          profileId: profile.id,
          displayName: draft.displayName,
          runtimeRelease: runtime.release,
          runtimeAsset: runtime.asset,
          selectionIds,
        },
      });
    },
    [profile, runtime, draft.displayName],
  );

  const revalidateSelections = useCallback(async (): Promise<Record<
    string,
    ValidatedAsset
  > | null> => {
    if (!profile) return null;
    const revalidated: Record<string, ValidatedAsset> = {};
    for (const role of requiredRoles) {
      const asset = draft.selections[role];
      if (!asset) return null;
      let fresh: ValidatedAsset[] = [];
      try {
        fresh = await invoke<ValidatedAsset[]>("hf_image_bundle_files", {
          profileId: profile.id,
          modelId: asset.modelId,
          role,
        });
      } catch {
        fresh = [];
      }
      const match = fresh.find(
        (candidate) =>
          candidate.sha256 === asset.sha256 &&
          candidate.relativePath === asset.relativePath &&
          candidate.revision === asset.revision,
      );
      if (!match) {
        clearRole(role);
        toast.error(
          t("hfBrowser.bundle.queueFailed"),
          t("hfBrowser.bundle.selectionInvalid", { role: roleLabel(role) }),
        );
        return null;
      }
      revalidated[role] = match;
    }
    return revalidated;
  }, [profile, requiredRoles, draft.selections, clearRole, roleLabel, t]);

  const startDownload = useCallback(async () => {
    if (!profile || !runtime || !estimate) return;
    setBusy(true);
    try {
      const selectionIds = requiredRoles.map((role) => draft.selections[role]!.selectionId);
      let result: QueueResult;
      try {
        result = await queueBundle(selectionIds);
      } catch (error) {
        const message = String(error);
        if (!/expired|no longer/i.test(message)) throw error;
        const revalidated = await revalidateSelections();
        if (!revalidated) return;
        setDraft((current) => ({
          ...current,
          selections: { ...current.selections, ...revalidated },
        }));
        result = await queueBundle(
          requiredRoles.map((role) => revalidated[role].selectionId),
        );
      }
      setDraft((current) => ({ ...current, bundleId: result.bundleId }));
      setBundleStatus({ registrationState: "downloading", modelId: null, setupError: null });
      onDownloadStarted?.();
    } catch (error) {
      toast.error(t("hfBrowser.bundle.queueFailed"), String(error));
    } finally {
      setBusy(false);
    }
  }, [profile, runtime, estimate, requiredRoles, draft.selections, queueBundle, revalidateSelections, onDownloadStarted, t]);

  const retryRegistration = useCallback(async () => {
    if (!draft.bundleId) return;
    setBusy(true);
    try {
      const modelId = await invoke<string>("hf_retry_image_bundle_registration", {
        bundleId: draft.bundleId,
      });
      setBundleStatus({ registrationState: "registered", modelId, setupError: null });
    } catch (error) {
      toast.error(t("hfBrowser.bundle.registrationFailed"), String(error));
    } finally {
      setBusy(false);
    }
  }, [draft.bundleId, t]);

  const retryDownloads = useCallback(async () => {
    if (!draft.bundleId) return;
    setBusy(true);
    try {
      await invoke("hf_retry_image_bundle_downloads", { bundleId: draft.bundleId });
      setBundleStatus({ registrationState: "downloading", modelId: null, setupError: null });
    } catch (error) {
      toast.error(t("hfBrowser.bundle.queueFailed"), String(error));
    } finally {
      setBusy(false);
    }
  }, [draft.bundleId, t]);

  return {
    draft,
    profiles,
    profile,
    runtime,
    requiredRoles,
    currentRole,
    ready,
    roleFiles,
    roleFilesLoading,
    roleFilesError,
    estimate,
    estimateBusy,
    bundleStatus,
    bundleQueue,
    downloadsSettled,
    downloadFailed,
    downloadsInterrupted,
    busy,
    roleLabel,
    chooseProfile,
    resetProfile,
    clearRole,
    selectAsset,
    setDisplayName,
    loadRoleFiles,
    startDownload,
    retryRegistration,
    retryDownloads,
    cancelItem,
  };
}

export type ImageBundleController = ReturnType<typeof useImageBundle>;
