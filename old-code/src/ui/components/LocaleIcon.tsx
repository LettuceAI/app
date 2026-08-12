import { Globe } from "lucide-react";

import { type Locale, getLocaleIconSrc } from "../../core/i18n";
import { cn } from "../design-tokens";

interface LocaleIconProps {
  locale: Locale;
  className?: string;
  imageClassName?: string;
  fallbackClassName?: string;
}

export function LocaleIcon({
  locale,
  className,
  imageClassName,
  fallbackClassName,
}: LocaleIconProps) {
  const src = getLocaleIconSrc(locale);

  return (
    <span
      className={cn("flex shrink-0 items-center justify-center overflow-hidden rounded-sm", className)}
    >
      {src ? (
        <img
          src={src}
          alt=""
          aria-hidden="true"
          className={cn("h-full w-full object-contain", imageClassName)}
        />
      ) : (
        <Globe aria-hidden="true" className={cn("h-4 w-4 text-fg/60", fallbackClassName)} />
      )}
    </span>
  );
}
