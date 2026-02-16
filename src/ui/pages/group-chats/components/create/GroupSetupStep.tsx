import { motion } from "framer-motion";
import {
  MessageSquare,
  Sparkles,
  Theater,
  Image as ImageIcon,
  X,
  Brain,
  BarChart3,
  RefreshCw,
} from "lucide-react";
import { typography, radius, spacing, interactive, shadows, cn } from "../../../../design-tokens";
import { processBackgroundImage } from "../../../../../core/utils/image";

interface GroupSetupStepProps {
  chatType: "conversation" | "roleplay";
  onChatTypeChange: (value: "conversation" | "roleplay") => void;
  speakerSelectionMethod: "llm" | "heuristic" | "round_robin";
  onSpeakerSelectionMethodChange: (value: "llm" | "heuristic" | "round_robin") => void;
  groupName: string;
  onGroupNameChange: (value: string) => void;
  backgroundImagePath: string;
  onBackgroundImageChange: (value: string) => void;
  namePlaceholder: string;
  onContinue: () => void;
  canContinue: boolean;
}

export function GroupSetupStep({
  chatType,
  onChatTypeChange,
  speakerSelectionMethod,
  onSpeakerSelectionMethodChange,
  groupName,
  onGroupNameChange,
  backgroundImagePath,
  onBackgroundImageChange,
  namePlaceholder,
  onContinue,
  canContinue,
}: GroupSetupStepProps) {
  const handleBackgroundImageUpload = (event: React.ChangeEvent<HTMLInputElement>) => {
    const file = event.target.files?.[0];
    if (!file) return;

    const input = event.target;
    void processBackgroundImage(file)
      .then((dataUrl: string) => {
        onBackgroundImageChange(dataUrl);
      })
      .catch((error: unknown) => {
        console.warn("GroupSetup: failed to process background image", error);
      })
      .finally(() => {
        input.value = "";
      });
  };

  return (
    <motion.div
      initial={{ opacity: 0, y: 16 }}
      animate={{ opacity: 1, y: 0 }}
      exit={{ opacity: 0, y: -16 }}
      transition={{ duration: 0.2, ease: "easeOut" }}
      className={spacing.section}
    >
      {/* Title */}
      <div className={spacing.tight}>
        <div className="flex items-center gap-2">
          <div className="rounded-lg border border-purple-400/30 bg-purple-400/10 p-1.5">
            <MessageSquare className="h-4 w-4 text-purple-400" />
          </div>
          <h2 className={cn(typography.h1.size, typography.h1.weight, "text-white")}>
            Group Setup
          </h2>
        </div>
        <p className={cn(typography.body.size, "mt-2 text-white/50")}>
          Configure your group chat settings
        </p>
      </div>

      {/* Chat Type Selection */}
      <div className={spacing.field}>
        <label
          className={cn(
            typography.label.size,
            typography.label.weight,
            typography.label.tracking,
            "uppercase text-white/70",
          )}
        >
          Chat Type
        </label>

        <div className="grid grid-cols-2 gap-3">
          {/* Conversation Option */}
          <button
            onClick={() => onChatTypeChange("conversation")}
            className={cn(
              "relative flex flex-col items-center gap-2 p-4",
              radius.lg,
              "border text-center",
              interactive.transition.fast,
              chatType === "conversation"
                ? "border-emerald-400/40 bg-emerald-400/10"
                : "border-white/10 bg-white/5 hover:border-white/20",
            )}
          >
            <div
              className={cn(
                "flex h-12 w-12 items-center justify-center",
                radius.lg,
                chatType === "conversation"
                  ? "border border-emerald-400/30 bg-emerald-400/20"
                  : "border border-white/10 bg-white/5",
              )}
            >
              <MessageSquare
                className={cn(
                  "h-6 w-6",
                  chatType === "conversation" ? "text-emerald-300" : "text-white/50",
                )}
              />
            </div>
            <div>
              <div
                className={cn(
                  "text-sm font-semibold",
                  chatType === "conversation" ? "text-emerald-100" : "text-white/80",
                )}
              >
                Conversation
              </div>
              <div className="mt-0.5 text-xs text-white/40">Casual chat</div>
            </div>
            {chatType === "conversation" && (
              <motion.div
                layoutId="chatTypeIndicator"
                className="absolute -top-1 -right-1 flex h-5 w-5 items-center justify-center rounded-full bg-emerald-400"
              >
                <Sparkles className="h-3 w-3 text-black" />
              </motion.div>
            )}
          </button>

          {/* Roleplay Option */}
          <button
            onClick={() => onChatTypeChange("roleplay")}
            className={cn(
              "relative flex flex-col items-center gap-2 p-4",
              radius.lg,
              "border text-center",
              interactive.transition.fast,
              chatType === "roleplay"
                ? "border-emerald-400/40 bg-emerald-400/10"
                : "border-white/10 bg-white/5 hover:border-white/20",
            )}
          >
            <div
              className={cn(
                "flex h-12 w-12 items-center justify-center",
                radius.lg,
                chatType === "roleplay"
                  ? "border border-emerald-400/30 bg-emerald-400/20"
                  : "border border-white/10 bg-white/5",
              )}
            >
              <Theater
                className={cn(
                  "h-6 w-6",
                  chatType === "roleplay" ? "text-emerald-300" : "text-white/50",
                )}
              />
            </div>
            <div>
              <div
                className={cn(
                  "text-sm font-semibold",
                  chatType === "roleplay" ? "text-emerald-100" : "text-white/80",
                )}
              >
                Roleplay
              </div>
              <div className="mt-0.5 text-xs text-white/40">With scenes</div>
            </div>
            {chatType === "roleplay" && (
              <motion.div
                layoutId="chatTypeIndicator"
                className="absolute -top-1 -right-1 flex h-5 w-5 items-center justify-center rounded-full bg-emerald-400"
              >
                <Sparkles className="h-3 w-3 text-black" />
              </motion.div>
            )}
          </button>
        </div>

        <p className={cn(typography.bodySmall.size, "mt-2 text-white/40")}>
          {chatType === "conversation"
            ? "Casual group conversation without starting scenes"
            : "Roleplay scenario with starting scene and immersive prompts"}
        </p>
      </div>

      {/* Speaker Selection Method */}
      <div className={spacing.field}>
        <label
          className={cn(
            typography.label.size,
            typography.label.weight,
            typography.label.tracking,
            "uppercase text-white/70",
          )}
        >
          Speaker Selection
        </label>

        <div className="grid grid-cols-3 gap-2">
          {(
            [
              {
                value: "llm" as const,
                label: "LLM",
                desc: "AI picks",
                icon: Brain,
              },
              {
                value: "heuristic" as const,
                label: "Heuristic",
                desc: "Score-based",
                icon: BarChart3,
              },
              {
                value: "round_robin" as const,
                label: "Round Robin",
                desc: "Take turns",
                icon: RefreshCw,
              },
            ] as const
          ).map((option) => (
            <button
              key={option.value}
              onClick={() => onSpeakerSelectionMethodChange(option.value)}
              className={cn(
                "relative flex flex-col items-center gap-1.5 p-3",
                radius.lg,
                "border text-center",
                interactive.transition.fast,
                speakerSelectionMethod === option.value
                  ? "border-emerald-400/40 bg-emerald-400/10"
                  : "border-white/10 bg-white/5 hover:border-white/20",
              )}
            >
              <option.icon
                className={cn(
                  "h-5 w-5",
                  speakerSelectionMethod === option.value ? "text-emerald-300" : "text-white/50",
                )}
              />
              <div
                className={cn(
                  "text-xs font-semibold",
                  speakerSelectionMethod === option.value ? "text-emerald-100" : "text-white/80",
                )}
              >
                {option.label}
              </div>
              <div className="text-[10px] text-white/40">{option.desc}</div>
              {speakerSelectionMethod === option.value && (
                <motion.div
                  layoutId="selectionMethodIndicator"
                  className="absolute -top-1 -right-1 flex h-4 w-4 items-center justify-center rounded-full bg-emerald-400"
                >
                  <Sparkles className="h-2.5 w-2.5 text-black" />
                </motion.div>
              )}
            </button>
          ))}
        </div>

        <p className={cn(typography.bodySmall.size, "mt-2 text-white/40")}>
          {speakerSelectionMethod === "llm"
            ? "Uses your default model to choose who speaks (costs tokens)"
            : speakerSelectionMethod === "heuristic"
              ? "Uses participation balance and context clues (free)"
              : "Characters take turns in order (free)"}
        </p>
      </div>

      {/* Background Image */}
      <div className={spacing.field}>
        <label
          className={cn(
            typography.label.size,
            typography.label.weight,
            typography.label.tracking,
            "uppercase text-white/70",
          )}
        >
          Chat Background <span className="text-white/40">(Optional)</span>
        </label>

        <div
          className={cn(
            "overflow-hidden border",
            radius.md,
            backgroundImagePath
              ? "border-purple-400/30 bg-purple-400/5"
              : "border-white/10 bg-black/20",
          )}
        >
          {backgroundImagePath ? (
            <div className="relative">
              <img
                src={backgroundImagePath}
                alt="Background preview"
                className="h-24 w-full object-cover"
              />
              <div className="absolute inset-0 bg-gradient-to-t from-black/60 to-transparent" />
              <button
                onClick={() => onBackgroundImageChange("")}
                className={cn(
                  "absolute top-2 right-2 flex h-6 w-6 items-center justify-center border border-white/20 bg-black/50 text-white/70",
                  radius.full,
                  interactive.transition.fast,
                  "active:scale-95 active:bg-black/70",
                )}
                aria-label="Remove background image"
              >
                <X className="h-3.5 w-3.5" />
              </button>
            </div>
          ) : (
            <label
              className={cn(
                "flex cursor-pointer items-center justify-center gap-2 py-6",
                interactive.transition.default,
                "hover:bg-white/5",
              )}
            >
              <ImageIcon className="h-5 w-5 text-white/40" />
              <span className={cn(typography.body.size, "text-white/50")}>
                Upload background image
              </span>
              <input
                type="file"
                accept="image/*"
                onChange={handleBackgroundImageUpload}
                className="hidden"
              />
            </label>
          )}
        </div>

        <p className={cn(typography.bodySmall.size, "text-white/40")}>
          Set a background image for this group chat
        </p>
      </div>

      {/* Group Name Input */}
      <div className={spacing.field}>
        <label
          className={cn(
            typography.label.size,
            typography.label.weight,
            typography.label.tracking,
            "uppercase text-white/70",
          )}
        >
          Group Name <span className="text-white/40">(Optional)</span>
        </label>
        <div className="relative">
          <input
            value={groupName}
            onChange={(e) => onGroupNameChange(e.target.value)}
            placeholder={namePlaceholder}
            inputMode="text"
            className={cn(
              "w-full border bg-black/20 px-4 py-3.5 text-white placeholder-white/40 backdrop-blur-xl",
              radius.md,
              typography.body.size,
              interactive.transition.default,
              "focus:border-white/30 focus:bg-black/30 focus:outline-none",
              groupName.trim() ? "border-emerald-400/30 bg-emerald-400/5" : "border-white/10",
            )}
          />
          {groupName.trim() && (
            <motion.div
              initial={{ scale: 0, opacity: 0 }}
              animate={{ scale: 1, opacity: 1 }}
              className="pointer-events-none absolute right-3 top-1/2 -translate-y-1/2"
            >
              <div
                className={cn(
                  "flex h-6 w-6 items-center justify-center",
                  radius.full,
                  "bg-emerald-400/20",
                )}
              >
                <Sparkles className="h-3 w-3 text-emerald-300" />
              </div>
            </motion.div>
          )}
        </div>
        <p className={cn(typography.bodySmall.size, "text-white/40")}>
          Leave empty to auto-generate from character names
        </p>
      </div>

      {/* Continue Button */}
      <div className="pt-2">
        <motion.button
          disabled={!canContinue}
          onClick={onContinue}
          whileTap={{ scale: canContinue ? 0.97 : 1 }}
          className={cn(
            "w-full py-4 text-base font-semibold",
            radius.md,
            interactive.transition.fast,
            canContinue
              ? cn(
                  "border border-emerald-400/40 bg-emerald-400/20 text-emerald-100",
                  shadows.glow,
                  "active:border-emerald-400/60 active:bg-emerald-400/30",
                )
              : "cursor-not-allowed border border-white/5 bg-white/5 text-white/30",
          )}
        >
          {chatType === "roleplay" ? "Continue to Starting Scene" : "Create Group Chat"}
        </motion.button>
      </div>
    </motion.div>
  );
}
