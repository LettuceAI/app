import { motion } from "framer-motion";
import { Link } from "react-router-dom";

export function TabItem({
  to,
  icon: Icon,
  label,
  active,
  className = "",
  dataTourId,
  layoutId = "activeTab",
  showLabel = false,
  rounded = "rounded-2xl",
  iconSize,
}: {
  to: string;
  icon: any;
  label: string;
  active: boolean;
  className?: string;
  dataTourId?: string;
  layoutId?: string;
  showLabel?: boolean;
  rounded?: string;
  iconSize?: number;
}) {
  return (
    <Link
      to={to}
      aria-label={label}
      aria-current={active ? "page" : undefined}
      data-tour-id={dataTourId}
      className={`relative block ${className}`}
    >
      <motion.div
        className={`relative flex h-full w-full items-center justify-center ${rounded} font-medium transition ${
          showLabel ? "flex-col gap-1" : ""
        } ${active ? "text-fg" : "text-fg/40 hover:text-fg"}`}
        whileTap={{ scale: 0.95 }}
      >
        {active && (
          <motion.div
            className={`absolute inset-0 ${rounded} border border-fg/15 bg-fg/10`}
            layoutId={layoutId}
            transition={{ type: "spring", stiffness: 320, damping: 28 }}
          />
        )}
        <Icon size={iconSize ?? (showLabel ? 20 : 22)} className="relative z-10" />
        {showLabel ? (
          <span className="relative z-10 text-[10px] leading-none">{label}</span>
        ) : (
          <span className="sr-only">{label}</span>
        )}
      </motion.div>
    </Link>
  );
}
