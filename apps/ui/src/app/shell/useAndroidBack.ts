import { dismissTopLayer } from "@lettuceai/crisp";
import { useRouter } from "@tanstack/react-router";
import { useEffect, useLayoutEffect, useRef } from "react";
import type { AppPlatform } from "@/api/generated/bindings";
import { createBackButton } from "@/api/back-button";

export function useAndroidBack(platform: AppPlatform, atRoot: boolean) {
  const router = useRouter();
  const root = useRef(atRoot);
  useLayoutEffect(() => {
    root.current = atRoot;
  }, [atRoot]);
  useEffect(() => {
    let disposed = false;
    let stop: (() => void) | undefined;
    void createBackButton(platform).then(async (button) => {
      if (!button || disposed) return;
      const unlisten = await button.listen((canGoBack) => {
        if (dismissTopLayer()) return;
        if (canGoBack) router.history.back();
        else if (root.current) void button.exit();
        else void router.navigate({ to: "/", replace: true });
      });
      if (disposed) unlisten();
      else stop = unlisten;
    }).catch(() => undefined);
    return () => { disposed = true; stop?.(); };
  }, [platform, router]);
}
