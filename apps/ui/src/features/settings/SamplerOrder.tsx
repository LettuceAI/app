import { Reorder, useDragControls } from "framer-motion";
import { ArrowDown, ArrowUp, GripVertical, Plus, X } from "lucide-react";
import { Button, IconButton, icon } from "@lettuceai/crisp";
import { useTranslation } from "react-i18next";

export function SamplerOrder({ value, options, onChange }: { value: string[]; options: readonly string[]; onChange: (value: string[]) => void }) {
  const { t } = useTranslation("settings");
  const move = (from: number, to: number) => {
    if (to < 0 || to >= value.length) return;
    const next = [...value];
    const item = next.splice(from, 1)[0];
    if (item) { next.splice(to, 0, item); onChange(next); }
  };
  const available = options.filter((option) => !value.includes(option));
  return <div className="space-y-3">
    {value.length > 0 ? <Reorder.Group axis="y" values={value} onReorder={onChange} className="divide-y divide-line rounded-xl border border-line" aria-label={t("modelEditor.fields.samplerOrder")}>
      {value.map((stage, index) => <SamplerStage key={stage} stage={stage} index={index} count={value.length} onMove={(offset) => move(index, index + offset)} onRemove={() => onChange(value.filter((item) => item !== stage))} />)}
    </Reorder.Group> : <p className="rounded-xl border border-line px-4 py-5 text-center text-sm text-fg-3">{t("modelEditor.emptyOrder")}</p>}
    {available.length > 0 && <div className="space-y-2">
      <p className="text-xs text-fg-3">{t("modelEditor.addStage")}</p>
      <div className="flex flex-wrap gap-2">{available.map((stage) => <Button key={stage} variant="secondary" size="sm" leading={<Plus size={icon.sm} />} onClick={() => onChange([...value, stage])}>{t(`modelEditor.options.samplerOrder.${stage}` as never)}</Button>)}</div>
    </div>}
  </div>;
}

function SamplerStage({ stage, index, count, onMove, onRemove }: { stage: string; index: number; count: number; onMove: (offset: number) => void; onRemove: () => void }) {
  const { t } = useTranslation("settings");
  const controls = useDragControls();
  const name = t(`modelEditor.options.samplerOrder.${stage}` as never);
  return <Reorder.Item value={stage} dragListener={false} dragControls={controls} dragMomentum={false} dragElastic={0} layout="position" whileDrag={{ zIndex: 1 }} transition={{ layout: { duration: 0.16 } }} className="relative flex items-center gap-2 bg-surface-1 px-2 py-1.5 first:rounded-t-xl last:rounded-b-xl">
    <IconButton size="sm" label={t("modelEditor.dragStage", { name })} className="touch-none cursor-grab active:cursor-grabbing" onPointerDown={(event) => controls.start(event)} onKeyDown={(event) => {
      if (event.key === "ArrowUp" || event.key === "ArrowDown") { event.preventDefault(); onMove(event.key === "ArrowUp" ? -1 : 1); }
      if (event.key === "Delete" || event.key === "Backspace") { event.preventDefault(); onRemove(); }
    }}><GripVertical size={icon.sm} /></IconButton>
    <span className="w-5 shrink-0 text-center text-xs tabular-nums text-fg-4">{index + 1}</span>
    <span className="min-w-0 flex-1 text-sm">{name}</span>
    <div className="flex shrink-0 items-center">
      <IconButton size="sm" disabled={index === 0} label={t("modelEditor.moveUp", { name })} onClick={() => onMove(-1)}><ArrowUp size={icon.sm} /></IconButton>
      <IconButton size="sm" disabled={index === count - 1} label={t("modelEditor.moveDown", { name })} onClick={() => onMove(1)}><ArrowDown size={icon.sm} /></IconButton>
      <IconButton size="sm" label={t("modelEditor.removeStage", { name })} onClick={onRemove}><X size={icon.sm} /></IconButton>
    </div>
  </Reorder.Item>;
}
