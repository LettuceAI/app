import { motion } from "framer-motion";
import { Link } from "react-router-dom";

export function TabItem({
  to,
  icon: Icon,
  label,
  active,
  className = "",
}: {
  to: string;
  icon: any;
  label: string;
  active: boolean;
  className?: string;
}) {
  return (
    <Link
      to={to}
      aria-label={label}
      aria-current={active ? "page" : undefined}
      className={`relative block ${className}`}
    >
      <motion.div
        className={`relative flex h-full w-full items-center justify-center rounded-2xl font-medium transition ${
          active ? "text-fg" : "text-fg/40 hover:text-fg"
        }`}
        whileTap={{ scale: 0.95 }}
      >
        {active && (
          <motion.div
            className="absolute inset-0 rounded-2xl border border-fg/15 bg-fg/10"
            layoutId="activeTab"
            transition={{ type: "spring", stiffness: 320, damping: 28 }}
          />
        )}
        <Icon size={22} className="relative z-10" />
        <span className="sr-only">{label}</span>
      </motion.div>
    </Link>
  );
}
