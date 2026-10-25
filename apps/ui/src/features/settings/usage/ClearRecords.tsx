import { useEffect, useRef, useState } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useTranslation } from "react-i18next";
import { Button, Segmented, Sheet, Text, toast } from "@lettuceai/crisp";
import type { UsageClearBeforeRequest } from "@/api/generated/bindings";
import { queryKeys } from "@/api/query-keys";
import { ActionError } from "@/shared/ui/ActionError";
import { useSettingsApi } from "../data";
import { timestamp } from "./format";

export function ClearRecords({ phone, onClose }: { phone: boolean; onClose: () => void }) {
  const { t, i18n } = useTranslation("usage");
  const api = useSettingsApi();
  const client = useQueryClient();
  const [days, setDays] = useState<"30" | "90" | "365">("90");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<unknown>(null);
  const [cutoff, setCutoff] = useState(() => Date.now() - 90 * 86400000);
  const request = useRef<UsageClearBeforeRequest | null>(null);
  const alive = useRef(true);
  useEffect(() => {
    alive.current = true;
    return () => {
      alive.current = false;
    };
  }, []);
  const clear = async () => {
    if (busy) return;
    setBusy(true);
    setError(null);
    request.current ??= { before: cutoff, client_operation_id: crypto.randomUUID() };
    try {
      const result = await api.call("usageClearBefore", request.current);
      await client.invalidateQueries({ queryKey: queryKeys.usage.all });
      if (alive.current) {
        toast.success(t("clear.done", { count: result.removed }));
        onClose();
      }
    } catch (failure) {
      if (alive.current) setError(failure);
    } finally {
      if (alive.current) setBusy(false);
    }
  };
  return (
    <Sheet
      open
      onClose={() => {
        if (!busy) {
          alive.current = false;
          onClose();
        }
      }}
      dismissible={!busy}
      placement={phone ? "bottom" : "side"}
      title={t("clear.title")}
      description={t("clear.body")}
      footer={
        <div className="flex justify-end gap-2">
          <Button variant="ghost" disabled={busy} onClick={onClose}>
            {t("cancel")}
          </Button>
          <Button variant="danger" loading={busy} onClick={() => void clear()}>
            {t("clear.action")}
          </Button>
        </div>
      }
    >
      <div className="space-y-4 pb-2">
        <Segmented
          block
          aria-label={t("clear.olderThan")}
          value={days}
          onChange={(value) => {
            if (busy) return;
            setDays(value);
            setCutoff(Date.now() - Number(value) * 86400000);
            request.current = null;
            setError(null);
          }}
          options={[
            { value: "30", label: t("clear.days", { count: 30 }) },
            { value: "90", label: t("clear.days", { count: 90 }) },
            { value: "365", label: t("clear.year") },
          ]}
        />
        <Text variant="hint">{t("clear.before", { value: timestamp(cutoff, i18n.language) })}</Text>
        <ActionError error={error} busy={busy} onRetry={() => void clear()} />
      </div>
    </Sheet>
  );
}
