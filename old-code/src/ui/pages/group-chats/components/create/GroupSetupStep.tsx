import { motion } from "framer-motion";
import {
  MessageSquare,
  Theater,
  Image as ImageIcon,
  X,
  Brain,
  BarChart3,
  RefreshCw,
  BookOpen,
  Clapperboard,
} from "lucide-react";
import { useI18n } from "../../../../../core/i18n/context";
import type { Character } from "../../../../../core/storage/schemas";
import { typography, radius, spacing, interactive, cn } from "../../../../design-tokens";
import { processBackgroundImage } from "../../../../../core/utils/image";
import { AvatarImage } from "../../../../components/AvatarImage";
import { useAvatar } from "../../../../hooks/useAvatar";
import { FieldLabel, OptionRow } from "../OptionRow";

type SpeakerSelectionMethod = "llm" | "heuristic" | "round_robin" | "director" | "director_action";

interface GroupSetupStepProps {
  selectedCharacters: Character[];
  chatType: "conversation" | "roleplay";
  onChatTypeChange: (value: "conversation" | "roleplay") => void;
  memoryType: "manual" | "dynamic";
  onMemoryTypeChange: (value: "manual" | "dynamic") => void;
  speakerSelectionMethod: SpeakerSelectionMethod;
  onSpeakerSelectionMethodChange: (value: SpeakerSelectionMethod) => void;
  groupName: string;
  onGroupNameChange: (value: string) => void;
  backgroundImagePath: string;
  onBackgroundImageChange: (value: string) => void;
  namePlaceholder: string;
}

function CastAvatar({ character }: { character: Character }) {
  const avatarUrl = useAvatar("character", character.id, character.avatarPath, "round");
  return (
    <div className="h-9 w-9 shrink-0 overflow-hidden rounded-full bg-linear-to-br from-fg/8 to-fg/4 ring-2 ring-surface">
      {avatarUrl ? (
        <AvatarImage src={avatarUrl} alt={character.name} crop={character.avatarCrop} applyCrop />
      ) : (
        <div className="flex h-full w-full items-center justify-center text-[10px] font-bold text-fg/60">
          {character.name.slice(0, 2).toUpperCase()}
        </div>
      )}
    </div>
  );
}

export function GroupSetupStep({
  selectedCharacters,
  chatType,
  onChatTypeChange,
  memoryType,
  onMemoryTypeChange,
  speakerSelectionMethod,
  onSpeakerSelectionMethodChange,
  groupName,
  onGroupNameChange,
  backgroundImagePath,
  onBackgroundImageChange,
  namePlaceholder,
}: GroupSetupStepProps) {
  const { t } = useI18n();
  const isDirector =
    speakerSelectionMethod === "director" || speakerSelectionMethod === "director_action";
  const visibleCast = selectedCharacters.slice(0, 6);
  const hiddenCastCount = selectedCharacters.length - visibleCast.length;

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
      className="grid items-start gap-6 lg:grid-cols-2 lg:gap-x-10"
    >
      <div className={spacing.section}>
        <div>
          <h1 className={cn(typography.h1.size, typography.h1.weight, "text-fg")}>
            {t("groupChats.create.groupSetup.title")}
          </h1>
          <p className={cn(typography.body.size, "mt-1 text-fg/50")}>
            {t("groupChats.create.groupSetup.subtitle")}
          </p>
        </div>

        <div className={spacing.field}>
          <FieldLabel>
            {t("groupChats.create.groupSetup.groupName")}{" "}
            <span className="text-fg/40">{t("groupChats.create.groupSetup.optional")}</span>
          </FieldLabel>
          <div className={cn("border border-fg/10 bg-surface-el/85", radius.lg)}>
            <div className="flex items-center gap-3 border-b border-fg/10 px-3 py-2.5">
              <div className="flex -space-x-2.5">
                {visibleCast.map((character) => (
                  <CastAvatar key={character.id} character={character} />
                ))}
                {hiddenCastCount > 0 && (
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-full bg-fg/10 text-[10px] font-bold text-fg/60 ring-2 ring-surface">
                    +{hiddenCastCount}
                  </div>
                )}
              </div>
              <p className={cn(typography.caption.size, "text-fg/50")}>
                {t("groupChats.create.groupSetup.castCount", {
                  count: String(selectedCharacters.length),
                })}
              </p>
            </div>
            <input
              value={groupName}
              onChange={(e) => onGroupNameChange(e.target.value)}
              placeholder={namePlaceholder}
              inputMode="text"
              className={cn(
                "w-full bg-transparent px-3 py-3 text-fg placeholder:text-fg/40",
                typography.body.size,
                "focus:outline-none",
              )}
            />
          </div>
          <p className={cn(typography.bodySmall.size, "text-fg/40")}>
            {t("groupChats.create.groupSetup.groupNameAutoGenerate")}
          </p>
        </div>

        <div className={spacing.field}>
          <FieldLabel>{t("groupChats.create.groupSetup.chatType")}</FieldLabel>
          <div className="space-y-2">
            <OptionRow
              selected={chatType === "conversation"}
              onSelect={() => onChatTypeChange("conversation")}
              icon={MessageSquare}
              label={t("groupChats.create.groupSetup.conversation")}
              description={t("groupChats.create.groupSetup.conversationDesc")}
            />
            <OptionRow
              selected={chatType === "roleplay"}
              onSelect={() => onChatTypeChange("roleplay")}
              icon={Theater}
              label={t("groupChats.create.groupSetup.roleplay")}
              description={t("groupChats.create.groupSetup.roleplayDesc")}
            />
          </div>
        </div>

        <div className={spacing.field}>
          <FieldLabel>
            {t("groupChats.create.groupSetup.chatBackground")}{" "}
            <span className="text-fg/40">{t("groupChats.create.groupSetup.optional")}</span>
          </FieldLabel>
          <div
            className={cn(
              "overflow-hidden border border-fg/10",
              radius.lg,
              !backgroundImagePath && "bg-surface-el/85",
            )}
          >
            {backgroundImagePath ? (
              <div className="relative">
                <img
                  src={backgroundImagePath}
                  alt={t("groupChats.create.groupSetup.chatBackground")}
                  className="h-28 w-full object-cover"
                />
                <button
                  onClick={() => onBackgroundImageChange("")}
                  className={cn(
                    "absolute right-2 top-2 flex h-7 w-7 items-center justify-center border border-fg/20 bg-surface/70 text-fg/80 backdrop-blur-sm",
                    radius.full,
                    interactive.transition.fast,
                    "active:scale-95",
                  )}
                  aria-label={t("groupChats.create.groupSetup.removeBackground")}
                >
                  <X className="h-3.5 w-3.5" />
                </button>
              </div>
            ) : (
              <label
                className={cn(
                  "flex cursor-pointer items-center justify-center gap-2 py-6",
                  interactive.transition.default,
                  "hover:bg-fg/5",
                )}
              >
                <ImageIcon className="h-5 w-5 text-fg/40" />
                <span className={cn(typography.body.size, "text-fg/50")}>
                  {t("groupChats.create.groupSetup.uploadBackground")}
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
          <p className={cn(typography.bodySmall.size, "text-fg/40")}>
            {t("groupChats.create.groupSetup.backgroundDesc")}
          </p>
        </div>
      </div>

      <div className={spacing.section}>
        <div className={spacing.field}>
          <FieldLabel>{t("groupChats.create.groupSetup.speakerSelection")}</FieldLabel>
          <div className="space-y-2">
            <OptionRow
              selected={speakerSelectionMethod === "llm"}
              onSelect={() => onSpeakerSelectionMethodChange("llm")}
              icon={Brain}
              label={t("groupChats.create.groupSetup.llm")}
              description={t("groupChats.create.groupSetup.llmDesc")}
            />
            <OptionRow
              selected={speakerSelectionMethod === "heuristic"}
              onSelect={() => onSpeakerSelectionMethodChange("heuristic")}
              icon={BarChart3}
              label={t("groupChats.create.groupSetup.heuristic")}
              description={t("groupChats.create.groupSetup.heuristicDesc")}
            />
            <OptionRow
              selected={speakerSelectionMethod === "round_robin"}
              onSelect={() => onSpeakerSelectionMethodChange("round_robin")}
              icon={RefreshCw}
              label={t("groupChats.create.groupSetup.roundRobin")}
              description={t("groupChats.create.groupSetup.roundRobinDesc")}
            />
            <OptionRow
              selected={isDirector}
              onSelect={() => {
                if (!isDirector) onSpeakerSelectionMethodChange("director");
              }}
              icon={Clapperboard}
              label={t("groupChats.sessionSettings.director")}
              description={t("groupChats.create.groupSetup.directorRowDesc")}
            />
            {isDirector && (
              <div className="grid grid-cols-2 gap-2 pl-4">
                {(
                  [
                    {
                      value: "director" as const,
                      label: t("groupChats.sessionSettings.directorCue"),
                      desc: t("groupChats.sessionSettings.directorCueShort"),
                    },
                    {
                      value: "director_action" as const,
                      label: t("groupChats.sessionSettings.directorAction"),
                      desc: t("groupChats.sessionSettings.directorActionShort"),
                    },
                  ] as const
                ).map((option) => {
                  const selected = speakerSelectionMethod === option.value;
                  return (
                    <button
                      key={option.value}
                      onClick={() => onSpeakerSelectionMethodChange(option.value)}
                      className={cn(
                        "flex flex-col items-center gap-0.5 px-3 py-2",
                        radius.lg,
                        "border text-center",
                        interactive.transition.fast,
                        selected
                          ? "border-accent/40 bg-accent/10"
                          : "border-fg/10 bg-surface-el/85 hover:border-fg/20",
                      )}
                    >
                      <div
                        className={cn(
                          "text-xs font-semibold",
                          selected ? "text-accent" : "text-fg/80",
                        )}
                      >
                        {option.label}
                      </div>
                      <div className="text-[10px] text-fg/40">{option.desc}</div>
                    </button>
                  );
                })}
              </div>
            )}
          </div>
        </div>

        <div className={spacing.field}>
          <FieldLabel>{t("groupChats.groupSettings.memoryMode")}</FieldLabel>
          <div className="space-y-2">
            <OptionRow
              selected={memoryType === "manual"}
              onSelect={() => onMemoryTypeChange("manual")}
              icon={BookOpen}
              label={t("groupChats.groupSettings.manual")}
              description={t("groupChats.groupSettings.memoryManualInfo")}
            />
            <OptionRow
              selected={memoryType === "dynamic"}
              onSelect={() => onMemoryTypeChange("dynamic")}
              icon={Brain}
              label={t("groupChats.groupSettings.dynamic")}
              description={t("groupChats.groupSettings.memoryDynamicInfo")}
            />
          </div>
        </div>
      </div>
    </motion.div>
  );
}
