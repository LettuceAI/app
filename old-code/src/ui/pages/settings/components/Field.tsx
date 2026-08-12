import type { ReactNode } from "react";
import { Switch } from "../../../components/Switch";

const labelClass = "mb-1 block text-[11px] font-medium text-fg/70";
const inputClass =
  "w-full rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2 text-sm text-fg placeholder-fg/40 focus:border-fg/30 focus:outline-none";

type TextFieldProps = {
  label: string;
  value: string;
  onChange: (value: string) => void;
  placeholder?: string;
  type?: "text" | "url" | "password";
};

export function TextField({
  label,
  value,
  onChange,
  placeholder,
  type = "text",
}: TextFieldProps) {
  return (
    <div>
      <label className={labelClass}>{label}</label>
      <input
        type={type}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        placeholder={placeholder}
        className={inputClass}
      />
    </div>
  );
}

type SelectFieldProps = {
  label: string;
  value: string;
  onChange: (value: string) => void;
  children: ReactNode;
};

export function SelectField({ label, value, onChange, children }: SelectFieldProps) {
  return (
    <div>
      <label className={labelClass}>{label}</label>
      <select
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className={inputClass}
      >
        {children}
      </select>
    </div>
  );
}

type ToggleRowProps = {
  id: string;
  title: string;
  description?: string;
  checked: boolean;
  onChange: (value: boolean) => void;
  variant?: "default" | "warning";
};

export function ToggleRow({
  id,
  title,
  description,
  checked,
  onChange,
  variant = "default",
}: ToggleRowProps) {
  const containerClass =
    variant === "warning"
      ? "rounded-lg border border-warning/20 bg-warning/5 px-3 py-2"
      : "rounded-lg border border-fg/10 bg-surface-el/20 px-3 py-2";
  return (
    <div className={containerClass}>
      <div className="flex items-center justify-between gap-3">
        <div className="min-w-0">
          <p className="text-sm font-medium text-fg/80">{title}</p>
          {description && <p className="text-[11px] text-fg/45">{description}</p>}
        </div>
        <Switch id={id} checked={checked} onChange={onChange} />
      </div>
    </div>
  );
}
