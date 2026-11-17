import { useEffect, useRef } from "react";
import type { ApiClient } from "@/api/client";
import type { AssetRef } from "@/api/generated/bindings";
import { AvatarLibrarySheet } from "./AvatarLibrary";
import { useAvatarPanelRoute } from "./use-avatar-panel-route";

type AvatarLibraryTarget = "avatar" | "banner";

export function useAvatarLibrary(api: ApiClient, onPick: (asset: AssetRef, target: AvatarLibraryTarget) => void) {
  const panel = useAvatarPanelRoute("avatarLibrary");
  const target = panel.value === "banner" ? "banner" : "avatar";
  const selected = useRef<{ asset: AssetRef; target: AvatarLibraryTarget } | null>(null);

  // Open the crop after the library's history entry has closed.
  useEffect(() => {
    if (panel.open || !selected.current) return;
    const selection = selected.current;
    selected.current = null;
    onPick(selection.asset, selection.target);
  }, [panel.open, onPick]);

  return {
    show: (target: AvatarLibraryTarget = "avatar") => panel.show(target === "banner" ? "banner" : true),
    sheet: <AvatarLibrarySheet api={api} open={panel.open} banner={target === "banner"} onClose={panel.hide} onPick={(asset) => {
      if (selected.current) return;
      selected.current = { asset, target };
      panel.hide();
    }} />,
  };
}
