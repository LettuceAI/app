import { useState, useEffect } from "react";
import { motion, AnimatePresence } from "framer-motion";
import { ArrowUp } from "lucide-react";
import { hasSeenTooltip, setTooltipSeen } from "../../core/storage/appState";
import { useI18n } from "../../core/i18n/context";

interface TooltipProps {
  isVisible: boolean;
  message: string;
  onClose: () => void;
  position?: "top" | "bottom";
  className?: string;
}

export function Tooltip({ 
  isVisible, 
  message, 
  onClose, 
  position = "top",
  className = "" 
}: TooltipProps) {
  const { t } = useI18n();
  const [shouldShow, setShouldShow] = useState(false);

  useEffect(() => {
    if (isVisible) {
      const timer = setTimeout(() => setShouldShow(true), 1000);
      return () => clearTimeout(timer);
    } else {
      setShouldShow(false);
    }
  }, [isVisible]);

  return (
    <AnimatePresence>
      {shouldShow && (
        <>
          {/* Backdrop */}
          <motion.div
            className="fixed inset-0 bg-black/20 z-40"
            initial={{ opacity: 0 }}
            animate={{ opacity: 1 }}
            exit={{ opacity: 0 }}
            onClick={onClose}
          />
          
          {/* Tooltip */}
          <motion.div
            className={`fixed z-50 ${getPositionClasses(position)} ${className}`}
            initial={{ opacity: 0, scale: 0.8, y: position === "top" ? 20 : -20 }}
            animate={{ opacity: 1, scale: 1, y: 0 }}
            exit={{ opacity: 0, scale: 0.8, y: position === "top" ? 20 : -20 }}
            transition={{ type: "spring", damping: 25, stiffness: 300 }}
          >
            <div className="bg-indigo-600 text-white p-3 rounded-xl shadow-2xl max-w-sm mx-auto relative">
              {/* Arrow */}
                <div className={`absolute ${getArrowClasses(position)}`}>
                <ArrowUp 
                  size={16} 
                  className={`text-indigo-600 ${position === "bottom" ? "rotate-180" : ""}`} 
                />
                </div>
              
              {/* Content */}
              <div className="flex items-center space-x-2">
                <div className="flex-1">
                  <p className="text-sm leading-relaxed">{message}</p>
                </div>
              </div>
              
              {/* Dismiss hint */}
              <div className="mt-3 pt-3 border-t border-white/20">
                <p className="text-xs text-indigo-100">{t("components.tooltip.dismissHint")}</p>
              </div>
            </div>
          </motion.div>
        </>
      )}
    </AnimatePresence>
  );
}

function getPositionClasses(position: "top" | "bottom"): string {
  if (position === "top") {
    return "top-24 left-1/2 transform -translate-x-1/2";
  } else {
    return "bottom-24 left-1/2 transform -translate-x-1/2";
  }
}

function getArrowClasses(position: "top" | "bottom"): string {
  if (position === "top") {
    return "bottom-full left-1/2 transform -translate-x-1/2 mb-1";
  } else {
    return "top-full left-1/2 transform -translate-x-1/2 mt-1";
  }
}

export function useFirstTimeTooltip(key: string) {
  const [isVisible, setIsVisible] = useState(false);

  useEffect(() => {
    let cancelled = false;

    (async () => {
      const seen = await hasSeenTooltip(key);
      if (!cancelled && !seen) {
        setIsVisible(true);
      }
    })();

    return () => {
      cancelled = true;
    };
  }, [key]);

  const dismissTooltip = () => {
    setIsVisible(false);
    void setTooltipSeen(key, true);
  };

  return {
    isVisible,
    dismissTooltip
  };
}
