import { useMemo, useState } from "react";
import { Check, ChevronDown } from "lucide-react";

import { type Locale, SUPPORTED_LOCALES, getLocaleMetadata } from "../../core/i18n";
import { getLocaleTranslationCompletion } from "../../core/i18n/translationStatus";
import { BottomMenu } from "./BottomMenu";
import { LocaleIcon } from "./LocaleIcon";
import { cn, interactive, radius, typography } from "../design-tokens";

interface LocaleSelectorProps {
  value: Locale;
  onChange: (locale: Locale) => void;
  label?: string;
  description?: string;
  title?: string;
  className?: string;
  triggerClassName?: string;
  labelClassName?: string;
  descriptionClassName?: string;
  menuClassName?: string;
}

function LocaleText({ locale, selected = false }: { locale: Locale; selected?: boolean }) {
  const metadata = getLocaleMetadata(locale);
  const showName = metadata.label !== metadata.name;

  return (
    <div className="min-w-0 flex-1">
      <div className={cn("truncate text-sm font-medium", selected ? "text-accent" : "text-fg")}>
        {metadata.label}
      </div>
      {showName && (
        <div className={cn("truncate text-[11px]", selected ? "text-accent/70" : "text-fg/45")}>
          {metadata.name}
        </div>
      )}
    </div>
  );
}

function CompletionBadge({ locale, selected = false }: { locale: Locale; selected?: boolean }) {
  const completion = getLocaleTranslationCompletion(locale);

  return (
    <span
      className={cn(
        "shrink-0 rounded-full border px-1.5 py-0.5 text-[10px] font-semibold tabular-nums",
        selected
          ? "border-accent/35 bg-accent/10 text-accent/85"
          : "border-fg/10 bg-fg/5 text-fg/45",
      )}
    >
      {completion.percent}%
    </span>
  );
}

export function LocaleSelector({
  value,
  onChange,
  label,
  description,
  title,
  className,
  triggerClassName,
  labelClassName,
  descriptionClassName,
  menuClassName,
}: LocaleSelectorProps) {
  const [isOpen, setIsOpen] = useState(false);

  const options = useMemo(() => SUPPORTED_LOCALES.map((locale) => ({ locale })), []);

  const handleSelect = (locale: Locale) => {
    onChange(locale);
    setIsOpen(false);
  };

  return (
    <div className={cn("space-y-2", className)}>
      {label && <label className={cn("text-sm font-medium text-fg", labelClassName)}>{label}</label>}

      <button
        type="button"
        onClick={() => setIsOpen(true)}
        className={cn(
          "flex w-full items-center gap-3 border px-3 py-2.5 text-left",
          radius.lg,
          interactive.transition.default,
          "border-fg/15 bg-fg/6 hover:border-fg/25 hover:bg-fg/8",
          "focus:outline-none focus:ring-1 focus:ring-accent/35 focus:border-accent/50",
          triggerClassName,
        )}
      >
        <LocaleIcon locale={value} className="h-5 w-5" fallbackClassName="h-4 w-4" />
        <LocaleText locale={value} />
        <CompletionBadge locale={value} />
        <ChevronDown className="h-4 w-4 shrink-0 text-fg/45" />
      </button>

      {description && (
        <p className={cn(typography.bodySmall.size, "text-fg/45", descriptionClassName)}>
          {description}
        </p>
      )}

      <BottomMenu
        isOpen={isOpen}
        onClose={() => setIsOpen(false)}
        title={title ?? label}
        className={menuClassName}
      >
        <div className="space-y-2">
          {options.map(({ locale }) => {
            const isSelected = locale === value;
            return (
              <button
                key={locale}
                type="button"
                onClick={() => handleSelect(locale)}
                className={cn(
                  "flex w-full items-center gap-3 border px-3 py-3 text-left",
                  radius.lg,
                  interactive.transition.fast,
                  isSelected
                    ? "border-accent/40 bg-accent/10"
                    : "border-fg/10 bg-fg/4 hover:border-fg/20 hover:bg-fg/7",
                )}
              >
                <LocaleIcon
                  locale={locale}
                  className="h-5 w-5"
                  fallbackClassName={isSelected ? "h-4 w-4 text-accent/80" : "h-4 w-4 text-fg/55"}
                />
                <LocaleText locale={locale} selected={isSelected} />
                <CompletionBadge locale={locale} selected={isSelected} />
                <div
                  className={cn(
                    "flex h-5 w-5 shrink-0 items-center justify-center rounded-full border",
                    isSelected
                      ? "border-accent/60 bg-accent/20 text-accent"
                      : "border-fg/15 bg-fg/5 text-transparent",
                  )}
                >
                  <Check className="h-3 w-3" />
                </div>
              </button>
            );
          })}
        </div>
      </BottomMenu>
    </div>
  );
}
