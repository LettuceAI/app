import { useI18n } from "../../core/i18n/context";
import { storageBridge } from "../../core/storage/files";
import { cn, radius } from "../design-tokens";
import { BottomMenu } from "./BottomMenu";

interface DynamicMemoryApprovalMenuProps {
  isOpen: boolean;
  onClose: () => void;
  sessionId: string | null;
  pendingCount: number;
  variant: "chat" | "group";
}

export function DynamicMemoryApprovalMenu({
  isOpen,
  onClose,
  sessionId,
  pendingCount,
  variant,
}: DynamicMemoryApprovalMenuProps) {
  const { t } = useI18n();

  const handleStart = async () => {
    if (!sessionId) return;
    try {
      if (variant === "group") {
        await storageBridge.groupChatRetryDynamicMemory(sessionId);
      } else {
        await storageBridge.triggerDynamicMemory(sessionId);
      }
    } catch (err) {
      console.error("Failed to start dynamic memory cycle:", err);
    } finally {
      onClose();
    }
  };

  const handleSkip = async () => {
    if (!sessionId) return;
    try {
      if (variant === "group") {
        await storageBridge.groupChatSkipDynamicMemory(sessionId);
      } else {
        await storageBridge.skipDynamicMemoryCycle(sessionId);
      }
    } catch (err) {
      console.error("Failed to skip dynamic memory cycle:", err);
    } finally {
      onClose();
    }
  };

  return (
    <BottomMenu isOpen={isOpen} onClose={onClose} title={t("dynamicMemory.approval.title")}>
      <div className="space-y-4">
        <p className="text-sm text-fg/70">
          {t("dynamicMemory.approval.description", { count: pendingCount })}
        </p>
        <div className="flex gap-3">
          <button
            type="button"
            onClick={() => void handleSkip()}
            className={cn(
              "flex-1 py-3 text-sm font-medium",
              radius.lg,
              "border border-fg/10 bg-fg/5 text-fg",
              "transition hover:border-fg/20 hover:bg-fg/10",
            )}
          >
            {t("dynamicMemory.approval.skip")}
          </button>
          <button
            type="button"
            onClick={() => void handleStart()}
            className={cn(
              "flex-1 py-3 text-sm font-medium",
              radius.lg,
              "border border-emerald-500/30 bg-emerald-500/20 text-emerald-100",
              "transition hover:bg-emerald-500/30",
            )}
          >
            {t("dynamicMemory.approval.start")}
          </button>
        </div>
      </div>
    </BottomMenu>
  );
}
