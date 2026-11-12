import { useRef } from "react";
import { useMutation } from "@tanstack/react-query";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import { UserPlus, UserRound, Wand2 } from "lucide-react";
import { icon, ListRowButton, toast } from "@lettuceai/crisp";
import { isApiFailure, type ApiClient } from "@/api/client";
import type { CreatorKind } from "@/api/generated/bindings";

/** Starts a creator session for a new item of `kind`, or for improving the item `targetId`, and opens it. */
export function CreatorStartRow({ api, kind, targetId, onStarted }: { api: ApiClient; kind: CreatorKind; targetId?: string; onStarted?: () => void }) {
  const { t } = useTranslation("creator");
  const { t: common } = useTranslation();
  const navigate = useNavigate();
  const key = useRef(crypto.randomUUID());
  const start = useMutation({
    mutationFn: () => api.call("creatorStart", { kind, target_id: targetId ?? null, draft: null, client_operation_id: key.current }),
    onSuccess: (work) => {
      key.current = crypto.randomUUID();
      onStarted?.();
      void navigate({ to: "/creator/$workflowId", params: { workflowId: work.summary.id } });
    },
    onError: (error) => toast.error(common(`errors.code.${isApiFailure(error) ? error.code : "internal"}`)),
  });
  const glyph = targetId ? <Wand2 size={icon.lg} /> : kind === "character" ? <UserPlus size={icon.lg} /> : <UserRound size={icon.lg} />;
  return (
    <ListRowButton
      icon={glyph}
      title={t(targetId ? "editWithAi" : `new.${kind}`)}
      description={t(targetId ? "editWithAiBody" : `new.${kind}Body`)}
      disabled={start.isPending}
      onClick={() => start.mutate()}
    />
  );
}
