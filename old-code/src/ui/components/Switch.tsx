import { cn } from "../design-tokens";

export interface SwitchProps {
  id?: string;
  checked: boolean;
  onChange: (checked: boolean) => void;
  disabled?: boolean;
  className?: string;
  "aria-label"?: string;
  "aria-labelledby"?: string;
}

export function Switch({
  id,
  checked,
  onChange,
  disabled,
  className,
  "aria-label": ariaLabel,
  "aria-labelledby": ariaLabelledBy,
}: SwitchProps) {
  return (
    <button
      type="button"
      role="switch"
      id={id}
      aria-checked={checked}
      disabled={disabled}
      onClick={() => onChange(!checked)}
      className={cn("inline-flex items-center", className)}
      aria-label={ariaLabel}
      aria-labelledby={ariaLabelledBy}
    >
      <span
        aria-label={ariaLabel}
        aria-hidden="true"
        className={cn(
          "relative inline-flex h-6 w-11 shrink-0 rounded-full border-2 border-transparent transition-all duration-200 ease-in-out",
          checked ? "bg-accent" : "bg-fg/20",
          disabled ? "cursor-not-allowed opacity-50" : "cursor-pointer",
        )}
      >
        <span
          className={cn(
            "inline-block h-5 w-5 transform rounded-full bg-fg transition duration-200 ease-in-out",
            checked ? "translate-x-5" : "translate-x-0",
          )}
        />
      </span>
    </button>
  );
}
