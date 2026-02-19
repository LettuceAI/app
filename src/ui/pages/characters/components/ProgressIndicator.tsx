import { motion } from "framer-motion";
import { User, BookOpen, FileText, Check } from "lucide-react";
import { typography, radius, cn } from "../../../design-tokens";

interface ProgressIndicatorProps {
  currentStep: number;
  stepLabel: string;
}

const steps = [
  { number: 1, label: "Identity", icon: User },
  { number: 2, label: "Scenes", icon: BookOpen },
  { number: 3, label: "Details", icon: FileText },
];

export function ProgressIndicator({ currentStep, stepLabel }: ProgressIndicatorProps) {
  const progressPercentage = ((currentStep - 1) / (steps.length - 1)) * 100;

  return (
    <div className="border-b border-fg/5 bg-surface px-4 pb-5 pt-5">
      {/* Header with step info */}
      <div className="mb-4 flex items-center justify-between">
        <div className="flex items-center gap-2">
          <span
            className={cn(
              typography.overline.size,
              typography.overline.weight,
              typography.overline.tracking,
              "uppercase text-fg/40"
            )}
          >
            Step {currentStep} of {steps.length}
          </span>
          <div className={cn("h-1 w-1", radius.full, "bg-fg/20")} />
          <span className={cn(typography.bodySmall.size, typography.h3.weight, "text-fg/70")}>
            {stepLabel}
          </span>
        </div>
        <span className={cn(typography.caption.size, "text-accent/70")}>
          {Math.round(progressPercentage)}%
        </span>
      </div>

      {/* Modern segmented progress bar */}
      <div className="flex gap-2">
        {steps.map((step) => {
          const isCompleted = currentStep > step.number;
          const isCurrent = currentStep === step.number;
          const Icon = step.icon;

          return (
            <div key={step.number} className="flex-1">
              <motion.div
                initial={false}
                animate={{
                  backgroundColor: isCompleted || isCurrent 
                    ? "rgba(52, 211, 153, 0.25)" 
                    : "rgba(255, 255, 255, 0.05)",
                }}
                transition={{ duration: 0.3, ease: "easeOut" }}
                className={cn(
                  "relative overflow-hidden",
                  radius.full,
                  "h-1.5 border",
                  isCompleted || isCurrent
                    ? "border-accent/40"
                    : "border-fg/10"
                )}
              >
                {/* Animated fill */}
                {isCurrent && (
                  <motion.div
                    className="absolute inset-0 bg-gradient-to-r from-accent/60 to-accent/80"
                    initial={{ x: "-100%" }}
                    animate={{ x: "0%" }}
                    transition={{
                      duration: 0.6,
                      ease: "easeOut",
                    }}
                  />
                )}
                {isCompleted && (
                  <div className="h-full w-full bg-accent/60" />
                )}
              </motion.div>

              {/* Step icon and label below */}
              <div className="mt-2 flex flex-col items-center gap-1">
                <motion.div
                  initial={false}
                  animate={{
                    scale: isCurrent ? 1 : 0.9,
                    opacity: isCompleted || isCurrent ? 1 : 0.4,
                  }}
                  transition={{ duration: 0.2 }}
                  className={cn(
                    "flex items-center justify-center",
                    radius.md,
                    "h-7 w-7 border",
                    isCompleted
                      ? "border-accent/40 bg-accent/20"
                      : isCurrent
                      ? "border-accent/30 bg-accent/15"
                      : "border-fg/10 bg-fg/5"
                  )}
                >
                  {isCompleted ? (
                    <motion.div
                      initial={{ scale: 0, rotate: -180 }}
                      animate={{ scale: 1, rotate: 0 }}
                      transition={{ duration: 0.3, ease: [0.34, 1.56, 0.64, 1] }}
                    >
                      <Check className="h-3.5 w-3.5 text-accent/80" strokeWidth={3} />
                    </motion.div>
                  ) : (
                    <Icon
                      className={cn(
                        "h-3.5 w-3.5 transition-colors",
                        isCurrent ? "text-accent/80" : "text-fg/40"
                      )}
                    />
                  )}
                </motion.div>

                {/* Step label */}
                <motion.span
                  initial={false}
                  animate={{
                    opacity: isCompleted || isCurrent ? 1 : 0.3,
                  }}
                  className={cn(
                    typography.caption.size,
                    typography.caption.weight,
                    isCurrent
                      ? "text-accent/80"
                      : isCompleted
                      ? "text-accent/60"
                      : "text-fg/40"
                  )}
                >
                  {step.label}
                </motion.span>
              </div>
            </div>
          );
        })}
      </div>
    </div>
  );
}
