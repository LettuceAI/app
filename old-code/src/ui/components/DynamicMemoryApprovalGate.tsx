import { useEffect, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { storageBridge } from "../../core/storage/files";
import { DynamicMemoryApprovalMenu } from "./DynamicMemoryApprovalMenu";

interface DynamicMemoryApprovalGateProps {
  sessionId: string | null;
  variant: "chat" | "group";
}

export function DynamicMemoryApprovalGate({ sessionId, variant }: DynamicMemoryApprovalGateProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [pendingCount, setPendingCount] = useState(0);

  const eventName =
    variant === "group" ? "group-dynamic-memory:approval-needed" : "dynamic-memory:approval-needed";

  useEffect(() => {
    if (!sessionId) {
      setIsOpen(false);
      return;
    }
    let cancelled = false;
    const query =
      variant === "group"
        ? storageBridge.groupChatDynamicMemoryPendingApproval
        : storageBridge.dynamicMemoryPendingApproval;
    void query(sessionId).then((count) => {
      if (cancelled || typeof count !== "number") return;
      setPendingCount(count);
      setIsOpen(true);
    });
    return () => {
      cancelled = true;
    };
  }, [sessionId, variant]);

  useEffect(() => {
    if (!sessionId) return;
    let unlisten: (() => void) | undefined;
    let disposed = false;
    void listen<{ sessionId: string; pendingCount: number }>(eventName, (event) => {
      if (event.payload?.sessionId !== sessionId) return;
      setPendingCount(event.payload.pendingCount ?? 0);
      setIsOpen(true);
    }).then((fn) => {
      if (disposed) fn();
      else unlisten = fn;
    });
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, [sessionId, eventName]);

  return (
    <DynamicMemoryApprovalMenu
      isOpen={isOpen}
      onClose={() => setIsOpen(false)}
      sessionId={sessionId}
      pendingCount={pendingCount}
      variant={variant}
    />
  );
}
