import { useId, useState, type ReactNode } from "react";
import { useTranslation } from "react-i18next";
import { Check, ChevronDown, Globe } from "lucide-react";
import { cn, icon, ListGroup, ListRowButton, SelectMenu, Sheet, Text, useCompactLayout, useFieldContext } from "@lettuceai/crisp";
import { ProviderMark } from "./ProviderMark";

export interface ProviderOption {
  id: string;
  name: string;
  kind: string;
  group?: string;
  meta?: string;
  disabled?: boolean;
}

export interface ProviderSelectProps {
  providers: readonly ProviderOption[];
  value: string | null;
  onChange: (id: string | null) => void;
  placeholder?: string;
  footer?: ReactNode;
  disabled?: boolean;
  /** Force the bottom sheet on native mobile, including wide displays. */
  compact?: boolean;
  id?: string;
  className?: string;
  "aria-label"?: string;
}

/** Provider counterpart of Crisp's ModelSelect: desktop menu, mobile sheet. */
export function ProviderSelect({ providers, value, onChange, placeholder, footer, disabled, compact: compactOverride, id, className, "aria-label": label }: ProviderSelectProps) {
  const { t } = useTranslation();
  const field = useFieldContext();
  const generatedId = useId();
  const controlId = id ?? field?.id ?? generatedId;
  const compactLayout = useCompactLayout();
  const compact = compactOverride ?? compactLayout;
  const [open, setOpen] = useState(false);
  const chosen = providers.find((provider) => provider.id === value);
  const prompt = placeholder ?? t("providerSelect.placeholder");
  const mark = (provider: ProviderOption) => <span aria-hidden="true"><ProviderMark kind={provider.kind} name={provider.name} /></span>;

  if (!compact) {
    return <>
      {!field && <label htmlFor={controlId} className="sr-only">{label ?? t("providerSelect.title")}</label>}
      <SelectMenu
      id={controlId}
      options={providers.map((provider) => ({
        value: provider.id,
        label: provider.name,
        icon: mark(provider),
        ...(provider.meta !== undefined ? { description: provider.meta } : {}),
        ...(provider.group !== undefined ? { group: provider.group } : {}),
        ...(provider.disabled !== undefined ? { disabled: provider.disabled } : {}),
      }))}
      value={value}
      onChange={onChange}
      placeholder={prompt}
      disabled={disabled ?? false}
      {...(className !== undefined ? { className } : {})}
      aria-label={label}
    /></>;
  }

  const groups = [...new Set(providers.map((provider) => provider.group ?? ""))];
  return <>
    <button
      type="button"
      id={controlId}
      disabled={disabled}
      aria-haspopup="dialog"
      aria-expanded={open && !disabled}
      aria-label={label}
      aria-describedby={field?.describedBy}
      aria-invalid={field?.invalid || undefined}
      onClick={() => setOpen(true)}
      className={cn("control-md flex w-full items-center gap-2.5 rounded-xl border border-line bg-fill px-3.5 text-left text-base text-fg disabled:opacity-40", className)}
    >
      <span className="shrink-0 text-fg-3">{chosen ? mark(chosen) : <Globe size={icon.md} />}</span>
      <span className={cn("min-w-0 flex-1 truncate", !chosen && "text-fg-4")}>{chosen?.name ?? prompt}</span>
      <ChevronDown size={icon.md} className="shrink-0 text-fg-3" />
    </button>
    <Sheet open={open && !disabled} onClose={() => setOpen(false)} title={label ?? t("providerSelect.title")} detents={[0.6, 0.92]}>
      <div className="space-y-5">
        {groups.map((group) => <ListGroup key={group} label={group || undefined}>
          {providers.filter((provider) => (provider.group ?? "") === group).map((provider) => <ListRowButton
            key={provider.id}
            icon={mark(provider)}
            title={provider.name}
            description={provider.meta}
            disabled={provider.disabled}
            aria-pressed={provider.id === value}
            chevron={false}
            trailing={provider.id === value ? <Check size={icon.md} className="text-accent" /> : undefined}
            onClick={() => { onChange(provider.id); setOpen(false); }}
          />)}
        </ListGroup>)}
        {providers.length === 0 && <Text variant="hint">{t("providerSelect.empty")}</Text>}
        {footer}
      </div>
    </Sheet>
  </>;
}
