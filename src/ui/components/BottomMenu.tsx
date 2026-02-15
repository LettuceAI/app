import { motion, AnimatePresence, useDragControls, PanInfo } from "framer-motion";
import { X, ChevronRight, LucideIcon, Loader2 } from "lucide-react";
import { ReactNode, useCallback, useMemo, useEffect, useId, isValidElement } from "react";

const ICON_ACCENT_MAP: Record<string, string> = {
  "from-blue-500 to-blue-600":
    "border-blue-400/40 bg-blue-500/15 text-blue-200 group-hover:border-blue-300/50 group-hover:text-blue-100",
  "from-blue-500 to-cyan-600":
    "border-cyan-400/40 bg-cyan-500/15 text-cyan-200 group-hover:border-cyan-300/50 group-hover:text-cyan-100",
  "from-purple-500 to-purple-600":
    "border-purple-400/35 bg-purple-500/15 text-purple-200 group-hover:border-purple-300/45 group-hover:text-purple-100",
  "from-purple-500 to-pink-600":
    "border-pink-400/40 bg-pink-500/15 text-pink-200 group-hover:border-pink-300/50 group-hover:text-pink-100",
  "from-amber-500 to-orange-600":
    "border-amber-400/40 bg-amber-500/15 text-amber-200 group-hover:border-amber-300/50 group-hover:text-amber-100",
  "from-amber-500 to-amber-600":
    "border-amber-400/40 bg-amber-500/15 text-amber-200 group-hover:border-amber-300/50 group-hover:text-amber-100",
  "from-indigo-500 to-blue-600":
    "border-indigo-400/40 bg-indigo-500/15 text-indigo-200 group-hover:border-indigo-300/50 group-hover:text-indigo-100",
  "from-emerald-500 to-emerald-600":
    "border-emerald-400/40 bg-emerald-500/15 text-emerald-200 group-hover:border-emerald-300/50 group-hover:text-emerald-100",
  "from-rose-500 to-red-600":
    "border-rose-400/40 bg-rose-500/15 text-rose-200 group-hover:border-rose-300/50 group-hover:text-rose-100",
  "from-rose-500 to-rose-600":
    "border-rose-400/40 bg-rose-500/15 text-rose-200 group-hover:border-rose-300/50 group-hover:text-rose-100",
};

export interface MenuButtonProps {
  icon: LucideIcon | ReactNode;
  title: string;
  description?: string;
  color?: string;
  onClick: () => void;
  disabled?: boolean;
  loading?: boolean;
  rightElement?: ReactNode;
}

export interface MenuDividerProps {
  label?: string;
  className?: string;
}

export interface MenuLabelProps {
  children: ReactNode;
  className?: string;
}

export interface BottomMenuProps {
  isOpen: boolean;
  onClose: () => void;
  title?: string;
  includeExitIcon?: boolean;
  location?: "top" | "bottom";
  children: ReactNode;
  className?: string;
  rightAction?: ReactNode;
}

export function BottomMenu({
  isOpen,
  onClose,
  title = "Menu",
  includeExitIcon = false,
  location = "bottom",
  children,
  className = "",
  rightAction,
}: BottomMenuProps) {
  const isBottomMenu = location === "bottom";
  const dragControls = useDragControls();
  const titleId = useId();

  const handleDragEnd = useCallback(
    (_: unknown, info: PanInfo) => {
      if (!isBottomMenu) return;
      const hasPulledFarEnough = info.offset.y > 100;
      const hasQuickSwipe = info.velocity.y > 600 && info.offset.y > 20;
      if (hasPulledFarEnough || hasQuickSwipe) {
        onClose();
        return;
      }

      dragControls.stop();
    },
    [isBottomMenu, onClose, dragControls],
  );

  const handlePointerDown = useCallback(
    (event: React.PointerEvent) => {
      event.preventDefault();
      dragControls.start(event);
    },
    [dragControls],
  );

  useEffect(() => {
    if (!isOpen) return;
    const handleKeyDown = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        onClose();
      }
    };
    window.addEventListener("keydown", handleKeyDown);
    return () => window.removeEventListener("keydown", handleKeyDown);
  }, [isOpen, onClose]);

  const menuVariants = useMemo(
    () => ({
      hidden: {
        y: isBottomMenu ? "100%" : "-100%",
        opacity: 0,
      },
      visible: {
        y: 0,
        opacity: 1,
      },
      exit: {
        y: isBottomMenu ? "100%" : "-100%",
        opacity: 0,
      },
    }),
    [isBottomMenu],
  );

  const menuClasses = isBottomMenu
    ? "fixed bottom-0 left-0 right-0 rounded-t-3xl pb-[calc(env(safe-area-inset-bottom))]"
    : "fixed top-0 left-0 right-0 rounded-b-3xl";

  return (
    <AnimatePresence>
      {isOpen && (
        <>
          <motion.div
            className="fixed inset-0 z-100 bg-black/60 backdrop-blur-sm"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            transition={{ duration: 0.15 }}
            onClick={onClose}
            aria-hidden="true"
          />

          <motion.div
            className={`${menuClasses} z-110 mx-auto max-w-xl border border-white/10 bg-[#0f1014] p-1 ${isBottomMenu ? "max-h-[90vh]" : "max-h-[95vh]"} overflow-hidden flex flex-col ${className}`}
            variants={menuVariants}
            initial="hidden"
            animate="visible"
            exit="exit"
            transition={{
              type: "spring",
              damping: 30,
              stiffness: 300,
              mass: 0.8,
            }}
            role="dialog"
            aria-modal="true"
            aria-labelledby={titleId}
            {...(isBottomMenu
              ? {
                drag: "y" as const,
                dragControls,
                dragListener: false,
                dragConstraints: { top: 0, bottom: 200 }, // Reduced for better feel
                dragElastic: { top: 0, bottom: 0.1 }, // Less elastic for snappier feel
                dragMomentum: false,
                onDragEnd: handleDragEnd,
              }
              : {})}
          >
            {isBottomMenu && (
              <div className="flex justify-center pt-4 pb-2">
                <button
                  type="button"
                  onPointerDown={handlePointerDown}
                  style={{ touchAction: "none" }}
                  className="flex h-8 w-28 items-center justify-center border-0 bg-transparent focus:outline-none"
                  aria-label="Drag to close menu"
                >
                  <span className="h-1 w-20 rounded-full bg-white/40" />
                </button>
              </div>
            )}

            <div
              className={`flex items-center justify-between px-6 ${isBottomMenu ? "pb-4" : "pt-4 pb-4"} shrink-0`}
            >
              <h3 id={titleId} className="text-lg font-semibold text-white">
                {title}
              </h3>
              <div className="flex items-center gap-2">
                {rightAction}
                {includeExitIcon && (
                  <button
                    className="flex items-center justify-center rounded-full border border-white/10 bg-white/5 text-white/70 hover:border-white/20 hover:text-white hover:bg-white/10 active:scale-95"
                    onClick={onClose}
                    aria-label="Close menu"
                  >
                    <X size={14} />
                  </button>
                )}
              </div>
            </div>

            <div className={`px-6 ${isBottomMenu ? "pb-8" : "pt-2 pb-4"} overflow-y-auto flex-1`}>
              {children}
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}

export function MenuButton({
  icon: Icon,
  title,
  description,
  color = "from-purple-500 to-blue-500",
  onClick,
  disabled = false,
  loading = false,
  rightElement,
}: MenuButtonProps) {
  const handleClick = useCallback(() => {
    if (!disabled && !loading) onClick();
  }, [disabled, loading, onClick]);

  const iconAccentClasses = ICON_ACCENT_MAP[color] || "border-white/10 bg-white/5 text-white/60";

  return (
    <motion.button
      className={`group relative w-full rounded-xl border border-white/10 bg-white/4 p-3 text-left text-white ${
        disabled || loading
          ? "cursor-not-allowed opacity-50"
          : "hover:border-white/15 hover:bg-white/[0.07] active:scale-[0.98]"
      } focus:outline-none focus-visible:ring-2 focus-visible:ring-white/25`}
      onClick={handleClick}
      disabled={disabled || loading}
    >
      <div className="flex items-center gap-2">
        <div
          className={`flex h-9 w-9 items-center justify-center rounded-lg border ${iconAccentClasses} ${
            disabled || loading ? "opacity-60" : ""
          }`}
        >
          {loading ? (
            <Loader2 size={18} className="animate-spin" />
          ) : isValidElement(Icon) ? (
            Icon
          ) : (
            // @ts-ignore - Icon is a component
            <Icon size={18} />
          )}
        </div>
        <div className="flex flex-1 items-center gap-2">
          <div className="flex-1">
            <h4 className="text-sm font-medium text-white group-hover:text-white">
              {loading ? "Processing..." : title}
            </h4>
            {description && !loading && (
              <p className="mt-0.5 text-xs text-white/55">{description}</p>
            )}
          </div>
          {!disabled &&
            !loading &&
            (rightElement || (
              <ChevronRight className="h-4 w-4 text-white/30 transition group-hover:text-white/60" />
            ))}
        </div>
      </div>
    </motion.button>
  );
}

export function MenuLabel({ children, className = "" }: MenuLabelProps) {
  return (
    <div className={`py-2 px-1 ${className}`}>
      <h4 className="text-[11px] font-medium uppercase tracking-[0.28em] text-white/40">
        {children}
      </h4>
    </div>
  );
}

export function MenuDivider({ label, className = "" }: MenuDividerProps) {
  return (
    <div className={`flex items-center py-4 ${className}`}>
      <div className="flex-1 border-t border-white/10" />
      {label && (
        <>
          <span className="px-3 text-[11px] font-medium uppercase tracking-[0.28em] text-white/40">
            {label}
          </span>
          <div className="flex-1 border-t border-white/10" />
        </>
      )}
    </div>
  );
}

export function MenuButtonGroup({
  children,
  className = "",
}: {
  children: ReactNode;
  className?: string;
}) {
  return <div className={`space-y-3 ${className}`}>{children}</div>;
}

export function MenuSection({
  label,
  children,
  className = "",
}: {
  label?: string;
  children: ReactNode;
  className?: string;
}) {
  return (
    <div className={`space-y-3 ${className}`}>
      {label && (
        <p className="text-[11px] font-medium uppercase tracking-[0.28em] text-white/40">{label}</p>
      )}
      {children}
    </div>
  );
}
