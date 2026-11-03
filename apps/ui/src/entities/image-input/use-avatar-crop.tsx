import { useEffect, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { Alert, AvatarCropSheet, CrispStringsProvider, type AvatarCropRegion, type AvatarCropView } from "@lettuceai/crisp";
import { ApiFailure, type ApiClient } from "@/api/client";
import type { AssetRef, LibraryCrop } from "@/api/generated/bindings";
import { useAvatarPanelRoute } from "./use-avatar-panel-route";

export interface BannerCropSelection {
  asset: AssetRef;
  source: AssetRef;
  crop: LibraryCrop;
}

interface PendingCrop {
  asset: AssetRef;
  resolve: (selection: BannerCropSelection | null) => void;
  result: BannerCropSelection | null;
  opened: boolean;
  saving: boolean;
}

export function useAvatarCrop(api: ApiClient) {
  const { t } = useTranslation("image-input");
  const { t: common } = useTranslation("common");
  const panel = useAvatarPanelRoute("avatarCrop");
  const pending = useRef<PendingCrop | null>(null);
  const mounted = useRef(true);
  const [asset, setAsset] = useState<AssetRef | null>(null);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<ApiFailure | null>(null);
  const [mode, setMode] = useState<"avatar" | "banner">("avatar");
  const [initialView, setInitialView] = useState<AvatarCropView | undefined>();
  useEffect(() => {
    const request = pending.current;
    if (!request) return;
    if (panel.open) request.opened = true;
    else if (request.opened) {
      pending.current = null;
      setAsset(null);
      setSaving(false);
      request.resolve(request.result);
    }
  }, [panel.open]);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
      pending.current?.resolve(null);
      pending.current = null;
    };
  }, []);
  const start = (image: AssetRef, target: "avatar" | "banner", crop?: LibraryCrop | null): Promise<BannerCropSelection | null> => {
    if (!mounted.current) return Promise.resolve(null);
    pending.current?.resolve(null);
    setAsset(image);
    setMode(target);
    setInitialView(crop && crop.x !== null && crop.y !== null && crop.scale !== null ? { x: crop.x, y: crop.y, zoom: crop.scale } : undefined);
    setError(null);
    setSaving(false);
    const result = new Promise<BannerCropSelection | null>((resolve) => {
      pending.current = { asset: image, resolve, result: null, opened: panel.open, saving: false };
    });
    panel.show();
    return result;
  };
  const choose = async (image: AssetRef): Promise<AssetRef | null> => (await start(image, "avatar"))?.asset ?? null;
  const chooseBanner = (image: AssetRef, crop?: LibraryCrop | null) => start(image, "banner", crop);
  const confirm = async (region: AvatarCropRegion, view: AvatarCropView) => {
    const request = pending.current;
    if (!request || request.saving) return;
    request.saving = true;
    setSaving(true);
    setError(null);
    try {
      const cropped = await api.call("assetsCropAvatar", { asset_id: request.asset.asset_id, ...region });
      if (pending.current !== request) return;
      request.result = { asset: cropped, source: request.asset, crop: { x: view.x, y: view.y, scale: view.zoom } };
      panel.hide();
    } catch (failure) {
      if (pending.current === request) setError(ApiFailure.fromRejection(failure));
    } finally {
      if (pending.current === request) {
        request.saving = false;
        setSaving(false);
      }
    }
  };
  const editor = <CrispStringsProvider strings={{
    avatarPickerTitle: t(mode === "banner" ? "bannerCrop.title" : "crop.title"),
    avatarPickerDescription: t(mode === "banner" ? "bannerCrop.description" : "crop.description"),
    avatarPickerZoom: t("crop.zoom"),
    avatarPickerZoomIn: t("crop.zoomIn"),
    avatarPickerZoomOut: t("crop.zoomOut"),
    avatarPickerCenter: t("crop.center"),
    avatarPickerArea: t("crop.area"),
    avatarPickerKeyboardHint: t("crop.keyboard"),
    avatarPickerReset: t("crop.reset"),
    avatarPickerCancel: t("actions.cancel"),
    avatarPickerUse: t("crop.apply"),
    avatarPickerRetry: t("actions.retry"),
    avatarPickerLoadError: t("crop.loadFailed"),
  }}>
    <AvatarCropSheet src={panel.open ? asset?.url ?? null : null} shape={mode === "banner" ? "banner" : "circle"} {...(initialView ? { initialView } : {})} onClose={panel.hide} busy={saving} onConfirm={(region, _image, view) => void confirm(region, view)}
      error={error && <Alert tone="danger" title={t(mode === "banner" ? "bannerCrop.saveFailed" : "crop.saveFailed")} description={common(`errors.code.${error.code}`)} />} />
  </CrispStringsProvider>;
  return { choose, chooseBanner, editor };
}
