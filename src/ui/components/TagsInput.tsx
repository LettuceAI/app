import { motion } from "framer-motion";
import { Tag } from "lucide-react";
import { typography, radius, spacing, interactive, cn } from "../design-tokens";

type TagsInputProps = {
  value: string;
  onChange: (value: string) => void;
  label: string;
  placeholder: string;
  hint?: string;
};

export function TagsInput({ value, onChange, label, placeholder, hint }: TagsInputProps) {
  return (
    <div className={spacing.field}>
      <label
        className={cn(
          typography.label.size,
          typography.label.weight,
          typography.label.tracking,
          "uppercase text-fg/70",
        )}
      >
        {label}
      </label>
      <div className="relative">
        <input
          value={value}
          onChange={(e) => onChange(e.target.value)}
          placeholder={placeholder}
          className={cn(
            "w-full border bg-surface-el/20 px-4 py-3.5 text-fg placeholder-fg/40 backdrop-blur-xl",
            radius.md,
            typography.body.size,
            interactive.transition.default,
            "focus:bg-surface-el/30 focus:outline-none",
            value.trim()
              ? "border-fg/20 focus:border-fg/40"
              : "border-fg/10 focus:border-fg/30",
          )}
        />
        {value.trim() && (
          <motion.div
            initial={{ scale: 0, opacity: 0 }}
            animate={{ scale: 1, opacity: 1 }}
            className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2"
          >
            <Tag className="h-3.5 w-3.5 text-fg/30" />
          </motion.div>
        )}
      </div>
      {hint && <p className={cn(typography.bodySmall.size, "text-fg/40")}>{hint}</p>}
    </div>
  );
}
