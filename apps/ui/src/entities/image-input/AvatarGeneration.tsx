import { useEffect, useRef } from "react";
import { useNavigate } from "@tanstack/react-router";
import { Sparkles } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, icon } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import type { AssetRef } from "@/api/generated/bindings";
import type { AvatarContext } from "./avatar-session";
import { AvatarGenerationSheet } from "./AvatarGenerationSheet";
import { useAvatarOwner } from "./avatar-owner-context";
import { useAvatarPanelRoute } from "./use-avatar-panel-route";

export interface AvatarGenerationProps {
  api: ApiClient;
  disabled?: boolean;
  readContext: () => AvatarContext;
  onUse: (asset: AssetRef) => void;
}

/** The Generate avatar action and its panel. The panel's open state lives in the owner's route, so back closes it before the form. */
export function AvatarGeneration({ api, disabled = false, readContext, onUse }: AvatarGenerationProps) {
  const { t } = useTranslation("image-input");
  const navigate = useNavigate();
  const owner = useAvatarOwner();
  const panel = useAvatarPanelRoute();
  const trigger = useRef<HTMLButtonElement>(null);
  const wasOpen = useRef(panel.open);
  useEffect(() => {
    if (wasOpen.current && !panel.open) trigger.current?.focus({ preventScroll: true });
    wasOpen.current = panel.open;
  }, [panel.open]);
  const configure = () => {
    owner?.keepForSettings(panel.routeKey);
    void navigate({ to: "/settings/$section", params: { section: "imageGeneration" }, search: { imagePicker: "avatar" } });
  };
  return <>
    <Button ref={trigger} size="sm" variant="ghost" disabled={disabled} leading={<Sparkles size={icon.sm} />} onClick={panel.show}>{t("generate")}</Button>
    <AvatarGenerationSheet api={api} open={panel.open} onClose={panel.hide} readContext={readContext} onUse={onUse} onConfigure={configure} />
  </>;
}
