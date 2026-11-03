import { useEffect, useRef } from "react";
import { useNavigate } from "@tanstack/react-router";
import { toast } from "@lettuceai/crisp";
import type { ApiClient } from "@/api/client";
import { usePromptText } from "./copy";

/** Tells once per launch that edited prompts have built-in changes waiting for a decision. */
export function BuiltInUpdateNotice({ api }: { api: ApiClient }) {
  const { t } = usePromptText();
  const navigate = useNavigate();
  const asked = useRef(false);
  useEffect(() => {
    if (asked.current) return;
    asked.current = true;
    api.call("promptsBuiltinUpdatesPending").then((pending) => {
      if (pending.count === 0) return;
      toast.info(t("Edited prompts updated count", { count: pending.count }), undefined, {
        action: { label: t("Open prompts"), onClick: () => void navigate({ to: "/settings/$section", params: { section: "prompts" } }) },
      });
    }).catch(() => undefined);
  }, [api, navigate, t]);
  return null;
}
