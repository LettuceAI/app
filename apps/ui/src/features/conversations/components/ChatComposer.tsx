import { useEffect, useImperativeHandle, useRef, useState, type ReactNode, type Ref } from "react";
import { useNavigate } from "@tanstack/react-router";
import { useTranslation } from "react-i18next";
import {
  Banner,
  Button,
  Composer,
  toast,
  useCompactLayout,
  type ComposerAttachment,
} from "@lettuceai/crisp";
import { useChatLook } from "@/entities/preferences";
import type { ApiClient, ApiFailure } from "@/api/client";
import { dictationErrorKey, useDictation } from "../data/use-dictation";
import type { MemoryBlockedReason, RequiredModel } from "@/api/generated/bindings";

export interface Blockers {
  missingModels: readonly RequiredModel[];
  modelRequired: boolean;
  memoryBlocked: MemoryBlockedReason | null;
}

export function ChatBanners({
  blockers,
  swap,
  onEndSwap,
  onRetryMemory,
}: {
  blockers: Blockers;
  swap: boolean;
  onEndSwap: () => void;
  onRetryMemory: () => void;
}) {
  const { t } = useTranslation("conversations");
  const navigate = useNavigate();
  const models = (
    <Button
      variant="secondary"
      size="sm"
      onClick={() => void navigate({ to: "/settings/$section", params: { section: "models" } })}
    >
      {t("banner.openModels")}
    </Button>
  );
  const named = blockers.missingModels.map((model) => t(`requiredModel.${model}`));
  return (
    <div className="mb-2 space-y-2 empty:hidden">
      {named.length > 0 ? (
        <Banner tone="warning" action={models}>
          {named.length === 1
            ? t("banner.modelNamed", { model: named[0] })
            : t("banner.modelsMissing", { models: named.join(", ") })}
        </Banner>
      ) : (
        blockers.modelRequired && (
          <Banner tone="warning" action={models}>
            {t("banner.modelRequired")}
          </Banner>
        )
      )}
      {swap && (
        <Banner
          tone="info"
          action={
            <Button variant="secondary" size="sm" onClick={onEndSwap}>
              {t("banner.endSwap")}
            </Button>
          }
        >
          {t("banner.swapActive")}
        </Banner>
      )}
      {blockers.memoryBlocked && (
        <Banner
          tone="warning"
          action={
            <Button variant="secondary" size="sm" onClick={onRetryMemory}>
              {t("banner.memoryRetry")}
            </Button>
          }
        >
          {t("banner.memoryBlocked")}
        </Banner>
      )}
    </div>
  );
}

export interface ComposerHandle {
  read: () => string;
  replace: (text: string) => void;
  append: (text: string) => void;
  attach: (attachment: ComposerAttachment) => void;
}

export interface ChatComposerProps {
  api: ApiClient;
  conversationId: string;
  handle: Ref<ComposerHandle>;
  name: string;
  sending: boolean;
  disabled: boolean;
  canContinue: boolean;
  onSend: (text: string, attachments: ComposerAttachment[]) => Promise<boolean>;
  onContinue: () => void;
  onStop: () => void;
  onAttach: () => void;
  topSlot?: ReactNode;
}

export function ChatComposer({
  api,
  conversationId,
  handle,
  name,
  sending,
  disabled,
  canContinue,
  onSend,
  onContinue,
  onStop,
  onAttach,
  topSlot,
}: ChatComposerProps) {
  const look = useChatLook();
  const { t } = useTranslation("conversations");
  const { t: common } = useTranslation("common");
  const phone = useCompactLayout();
  const [draft, setDraft] = useState("");
  const [attachments, setAttachments] = useState<ComposerAttachment[]>([]);
  const draftRef = useRef(draft);
  useEffect(() => {
    draftRef.current = draft;
  });

  useImperativeHandle(
    handle,
    () => ({
      read: () => draftRef.current,
      replace: (text) => setDraft(text),
      attach: (attachment) => setAttachments((current) => [...current, attachment]),
      append: (text) =>
        setDraft((current) => (current.trimEnd() ? `${current.trimEnd()} ${text}` : text)),
    }),
    [],
  );

  const dictation = useDictation(api, conversationId, {
    onText: (text) => setDraft(text.trim()),
    onError: (error: ApiFailure) => {
      const key = dictationErrorKey(error);
      toast.error(key ? t(`mic.error.${key}`) : common(`errors.code.${error.code}`));
    },
    onJobError: (key) => toast.error(t(`mic.error.${key}`)),
  });

  const send = () => {
    const text = draft.trim();
    if (!text && attachments.length === 0) return;
    const sent = attachments;
    setDraft("");
    setAttachments([]);
    void onSend(text, sent).then((accepted) => {
      if (accepted) return;
      setDraft((current) => (current.length === 0 ? text : current));
      setAttachments((current) => (current.length === 0 ? sent : current));
    });
  };

  return (
    <Composer
      fontSize={look.composerFontSize}
      {...(look.footerInputColorHex ? { color: look.footerInputColorHex } : {})}
      topSlot={topSlot}
      value={draft}
      onChange={setDraft}
      onSend={send}
      onStop={onStop}
      onAttach={onAttach}
      attachments={attachments}
      onRemoveAttachment={(id) =>
        setAttachments((current) => current.filter((attachment) => attachment.id !== id))
      }
      {...(canContinue ? { onContinue } : {})}
      mic={{
        active: dictation.phase !== "idle",
        transcribing: dictation.phase === "transcribing",
        elapsedMs: dictation.elapsedMs,
        analyser: null,
        level: dictation.level,
        onStart: () => void dictation.start(),
        onStop: () => void dictation.stop(),
        onCancel: dictation.cancel,
        disabled: sending || disabled,
      }}
      sending={sending}
      disabled={disabled}
      enterSends={!phone}
      placeholder={t("composer.placeholder", { name })}
    />
  );
}
