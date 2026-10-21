import { useState } from "react";
import { createPortal } from "react-dom";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { Check } from "lucide-react";
import { useTranslation } from "react-i18next";
import { Button, Checkbox, icon, Input, ListGroup, ListRow, ListRowButton, LoadingState } from "@lettuceai/crisp";
import type { SdComputePolicy, SdComputePolicyInfo, SdRuntimeRef } from "@/api/generated/bindings";
import { FailureState } from "@/shared/ui/FailureState";
import { useSettingsApi } from "../data";
import { localImageKeys, useByteSize, useImageAction } from "./local-data";

export function ComputePolicyView({ runtime, onDone, onClose, footerTarget }: { runtime: SdRuntimeRef; onDone: () => void; onClose: () => void; footerTarget: HTMLElement | null }) {
  const api = useSettingsApi();
  const policy = useQuery({ queryKey: localImageKeys.policy(runtime.release, runtime.asset), queryFn: () => api.call("sdComputePolicyGet", runtime) });
  if (!policy.data) return policy.isError ? <FailureState error={policy.error} onRetry={() => void policy.refetch()} /> : <LoadingState />;
  return <PolicyForm info={policy.data} onDone={onDone} onClose={onClose} footerTarget={footerTarget} />;
}

function PolicyForm({ info, onDone, onClose, footerTarget }: { info: SdComputePolicyInfo; onDone: () => void; onClose: () => void; footerTarget: HTMLElement | null }) {
  const { t } = useTranslation("settings");
  const { t: common } = useTranslation("common");
  const api = useSettingsApi();
  const client = useQueryClient();
  const action = useImageAction();
  const size = useByteSize();
  const [draft, setDraft] = useState(info.policy);
  const [budgets, setBudgets] = useState<Record<number, string>>(() => Object.fromEntries(info.policy.device_budgets_gib.map((budget) => [budget.device_id, String(budget.gib)])));
  const mode = draft.multi_gpu_enabled ? "multi" : draft.single_gpu_device_id === null ? "auto" : "single";
  const chosen = (id: number) => mode === "auto" || (mode === "single" ? draft.single_gpu_device_id === id : draft.gpu_device_ids.includes(id));
  const budgetError = (id: number) => {
    const text = budgets[id]?.trim() ?? "";
    if (!text) return false;
    const value = Number(text.replace(",", "."));
    return !Number.isFinite(value) || value <= 0 || value * 1024 ** 3 > (info.devices.find((device) => device.id === id)?.total_bytes ?? 0);
  };
  const invalid = info.devices.some((device) => chosen(device.id) && budgetError(device.id)) || (mode === "multi" && draft.gpu_device_ids.length < 2) || (mode === "single" && !info.devices.some((device) => device.id === draft.single_gpu_device_id));
  const chooseMode = (next: "auto" | "single" | "multi") => {
    setDraft((current) => ({ ...current, multi_gpu_enabled: next === "multi", single_gpu_device_id: next === "single" ? current.single_gpu_device_id ?? info.devices[0]?.id ?? null : null, gpu_device_ids: next === "multi" ? current.gpu_device_ids : [], split_mode: info.supports_row_split ? current.split_mode : "layer" }));
  };
  const save = () => action.mutate(async () => {
    const policy: SdComputePolicy = { ...draft, device_budgets_gib: info.devices.filter((device) => chosen(device.id) && budgets[device.id]?.trim()).map((device) => ({ device_id: device.id, gib: Number(budgets[device.id]!.replace(",", ".")) })) };
    const saved = await api.call("sdComputePolicySave", { release: info.runtime_release, asset: info.runtime_asset, policy });
    client.setQueryData(localImageKeys.policy(info.runtime_release, info.runtime_asset), saved);
    onDone();
  });
  const buttons = <div className="flex justify-end gap-2"><Button variant="ghost" onClick={onClose}>{common("actions.cancel")}</Button><Button disabled={invalid} loading={action.isPending} onClick={save}>{t("manage.save")}</Button></div>;
  return (
    <div className="space-y-5">
      {action.isError && <FailureState error={action.error} size="sm" />}
      <ListGroup label={t("image.local.policy.mode")}>
        {(["auto", "single", "multi"] as const).map((item) => <ListRowButton key={item} title={t(`image.local.policy.${item}`)} description={t(item === "multi" && info.devices.length < 2 ? "image.local.policy.twoGpus" : `image.local.policy.${item}Body`)} chevron={false} aria-pressed={mode === item} trailing={mode === item ? <Check size={icon.md} /> : undefined} disabled={action.isPending || (item === "multi" && info.devices.length < 2) || (item === "single" && info.devices.length === 0)} onClick={() => chooseMode(item)} />)}
      </ListGroup>
      <ListGroup label={t("image.local.policy.devices")}>
        {info.devices.length === 0 && <ListRow title={t("image.local.policy.noDevices")} />}
        {info.devices.map((device) => <div key={device.id}>
          {mode === "multi" ? <ListRow title={device.description || device.name} description={t("image.local.policy.memory", { free: size(device.free_bytes), total: size(device.total_bytes) })} trailing={<Checkbox checked={chosen(device.id)} disabled={action.isPending} aria-label={device.name} onChange={(checked) => setDraft((current) => ({ ...current, gpu_device_ids: checked ? [...current.gpu_device_ids, device.id] : current.gpu_device_ids.filter((id) => id !== device.id) }))} />} /> : <ListRowButton title={device.description || device.name} description={t("image.local.policy.memory", { free: size(device.free_bytes), total: size(device.total_bytes) })} chevron={false} disabled={mode === "auto" || action.isPending} trailing={mode === "single" && chosen(device.id) ? <Check size={icon.md} /> : undefined} onClick={() => setDraft((current) => ({ ...current, single_gpu_device_id: device.id }))} />}
          {chosen(device.id) && <ListRow title={t("image.local.policy.budget")} description={t(budgetError(device.id) ? "image.local.policy.invalidBudget" : "image.local.policy.automaticBudget")} trailing={<Input className="w-28" inputMode="decimal" aria-label={t("image.local.policy.budgetFor", { name: device.name })} placeholder={t("image.local.policy.auto")} value={budgets[device.id] ?? ""} disabled={action.isPending} onChange={(event) => setBudgets((current) => ({ ...current, [device.id]: event.target.value }))} />} />}
        </div>)}
      </ListGroup>
      {mode === "multi" && <ListGroup label={t("image.local.policy.split")}>
        {(["layer", "row"] as const).map((split) => <ListRowButton key={split} title={t(`image.local.policy.${split}`)} description={split === "row" && !info.supports_row_split ? t("image.local.policy.rowUnsupported") : undefined} chevron={false} trailing={draft.split_mode === split ? <Check size={icon.md} /> : undefined} disabled={action.isPending || (split === "row" && !info.supports_row_split)} onClick={() => setDraft((current) => ({ ...current, split_mode: split }))} />)}
      </ListGroup>}
      {footerTarget ? createPortal(buttons, footerTarget) : buttons}
    </div>
  );
}
